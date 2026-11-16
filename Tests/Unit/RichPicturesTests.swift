import Foundation
#if canImport(AppKit)
import AppKit
#elseif canImport(UIKit)
import UIKit
#endif
import ImageIO
import Testing
import UniformTypeIdentifiers
@testable import WhooshUI

// fix-screens: a picture run in the text is drawn as the picture (a thumbnail decoded off the main thread), not as the "photo" symbol.
// MUTANTS (red first, one edit each):
//   aThumbnailIsNeverLargerThanItsLimitAndKeepsTheShape : RichPictures.thumbnail passes `kCGImageSourceThumbnailMaxPixelSize: maxSide` as `100000`
//   aFileThatIsNoPictureGivesNoThumbnail                : thumbnail returns a blank image instead of nil
//   aRegisteredPictureReplacesThePlaceholderInTheText   : RichAttributed.make drops the `RichPictures.shared.cached(cid)` branch
//   theThumbnailIsMadeOffTheMainThread                  : RichPictures.load runs `Self.thumbnail` on the calling thread
//   aLargePictureIsDrawnShrunkAndASmallOneIsNotEnlarged : drawnSize returns the size unchanged
//   theCacheKeepsAtMostItsLimit                         : init drops `cache.countLimit`

private func writePNG(width: Int, height: Int, name: String) throws -> URL {
    let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(),
                        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    ctx.setFillColor(CGColor(red: 0.2, green: 0.5, blue: 0.8, alpha: 1)); ctx.fill(CGRect(x: 0, y: 0, width: width, height: height))
    let url = FileManager.default.temporaryDirectory.appendingPathComponent("rich-pictures-\(UUID().uuidString)-\(name).png")
    let dest = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)!
    CGImageDestinationAddImage(dest, ctx.makeImage()!, nil)
    #expect(CGImageDestinationFinalize(dest))
    return url
}

@Test func aThumbnailIsNeverLargerThanItsLimitAndKeepsTheShape() throws {
    let url = try writePNG(width: 2400, height: 1200, name: "big")
    defer { try? FileManager.default.removeItem(at: url) }
    let image = try #require(RichPictures.thumbnail(of: url, maxSide: RichPictures.maxSide))
    #expect(Int(image.size.width) == RichPictures.maxSide, "the longer side is the limit: \(image.size)")
    #expect(Int(image.size.height) == RichPictures.maxSide / 2, "the shape (2:1) is kept: \(image.size)")
}

@Test func aFileThatIsNoPictureGivesNoThumbnail() throws {
    let url = FileManager.default.temporaryDirectory.appendingPathComponent("rich-pictures-\(UUID().uuidString).txt")
    try Data("not a picture".utf8).write(to: url)
    defer { try? FileManager.default.removeItem(at: url) }
    #expect(RichPictures.thumbnail(of: url, maxSide: 600) == nil)
    #expect(RichPictures.thumbnail(of: url.appendingPathExtension("missing"), maxSide: 600) == nil)
}

@Test func aLargePictureIsDrawnShrunkAndASmallOneIsNotEnlarged() {
    let big = RichPictures.drawnSize(width: 600, height: 300)
    #expect(big.width == RichPictures.drawnMaxSide && big.height == (RichPictures.drawnMaxSide / 2).rounded(), "\(big)")
    let small = RichPictures.drawnSize(width: 40, height: 30)
    #expect(small == CGSize(width: 40, height: 30), "\(small)")
}

@MainActor @Test func aRegisteredPictureReplacesThePlaceholderInTheText() async throws {
    let url = try writePNG(width: 800, height: 600, name: "run")
    defer { try? FileManager.default.removeItem(at: url) }
    let cid = "test-cid-\(UUID().uuidString)"
    RichPictures.shared.register(cid: cid, url: url)
    let doc = RichDocument(paragraphs: [RichParagraph(runs: [RichRun("", cid: cid)])])
    func attachmentImage() -> PImage? {
        let s = RichAttributed.make(doc)
        var found: PImage?
        s.enumerateAttribute(.attachment, in: NSRange(location: 0, length: s.length)) { v, _, _ in found = (v as? NSTextAttachment)?.image }
        return found
    }
    let placeholder = try #require(attachmentImage())
    #expect(RichPictures.shared.cached(cid) == nil || placeholder.size.width > 20, "first drawn as the placeholder or already loaded")
    for _ in 0..<100 where RichPictures.shared.cached(cid) == nil { try await Task.sleep(nanoseconds: 20_000_000) }
    let thumb = try #require(RichPictures.shared.cached(cid), "the thumbnail never finished")
    #expect(Int(thumb.size.width) == 600, "\(thumb.size)")
    let drawn = try #require(attachmentImage())
    #expect(drawn.size.width == thumb.size.width, "the text now draws the picture (\(drawn.size)), not the symbol (\(placeholder.size))")
}

@Test func aContentIdWithNoFileKeepsThePlaceholderAndDecodesNothing() {
    let cid = "no-file-\(UUID().uuidString)"
    RichPictures.shared.load(cid)
    #expect(RichPictures.shared.cached(cid) == nil)
    #expect(RichPictures.shared.file(for: cid) == nil)
}

@MainActor @Test func theThumbnailIsMadeOffTheMainThread() async throws {
    let url = try writePNG(width: 3000, height: 3000, name: "slow")
    defer { try? FileManager.default.removeItem(at: url) }
    let cid = "off-main-\(UUID().uuidString)"
    RichPictures.shared.register(cid: cid, url: url)
    RichPictures.shared.load(cid)
    // load returns before the picture exists: the decode runs on its own queue
    #expect(RichPictures.shared.cached(cid) == nil, "load decoded on the calling thread")
    for _ in 0..<150 where RichPictures.shared.cached(cid) == nil { try await Task.sleep(nanoseconds: 20_000_000) }
    #expect(RichPictures.shared.cached(cid) != nil)
}

@Test func theCacheKeepsAtMostItsLimit() throws {
    let url = try writePNG(width: 64, height: 64, name: "many")
    defer { try? FileManager.default.removeItem(at: url) }
    let store = RichPictures()
    #expect(RichPictures.cacheLimit == 12)
    for n in 0..<40 {
        let cid = "c\(n)"
        store.register(cid: cid, url: url)
        store.load(cid)
    }
    Thread.sleep(forTimeInterval: 1.0)
    let kept = (0..<40).filter { store.cached("c\($0)") != nil }.count
    #expect(kept <= RichPictures.cacheLimit, "kept \(kept) thumbnails, the limit is \(RichPictures.cacheLimit)")
}
