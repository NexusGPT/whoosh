#if os(macOS)
import AppKit
import Foundation
import Testing
@testable import WhooshUI

// fix-screens: a pasted or dropped image is decoded and written as a PNG on a utility queue, never on the main thread.
// MUTANTS (red first, one edit each):
//   aPastedImageIsWrittenAsAPngOffTheMainThread : AttachmentIntake.takeImage runs `writeImage(data)` inline instead of inside DispatchQueue.global
//   anImageThatIsNoPictureWritesNothing         : writeImage returns a file for any data

private func tiffData() -> Data {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 32, pixelsHigh: 32, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                               colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    return rep.tiffRepresentation!
}

@Test func aPastedImageIsWrittenAsAPngOffTheMainThread() async throws {
    let data = tiffData()
    let (url, onMain) = await Task.detached { () -> (URL?, Bool) in (AttachmentIntake.writeImage(data), pthread_main_np() != 0) }.value   // Thread.isMainThread is not available in an async context
    let file = try #require(url, "a tiff on the pasteboard gives a png file")
    defer { try? FileManager.default.removeItem(at: file) }
    #expect(onMain == false)
    #expect(try Data(contentsOf: file).starts(with: [0x89, 0x50, 0x4E, 0x47]), "the file is a PNG")
    #expect(file.pathExtension == "png")
}

@Test func takeImageReturnsBeforeTheFileIsWritten() async throws {
    // the call itself does no decode: with a rig-less run folder it only queues the work
    let started = Date()
    AttachmentIntake.takeImage(tiffData())
    #expect(Date().timeIntervalSince(started) < 0.05, "takeImage must queue, not decode (\(Date().timeIntervalSince(started)) s)")
}

@Test func anImageThatIsNoPictureWritesNothing() {
    #expect(AttachmentIntake.writeImage(Data("not an image".utf8)) == nil)
}
#endif
