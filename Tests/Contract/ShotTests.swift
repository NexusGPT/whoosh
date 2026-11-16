import Foundation
import ImageIO
import Testing

/// Decodes a PNG file: pixel size and the number of distinct colours among a sparse sample.
func pngFacts(_ path: String) -> (w: Int, h: Int, distinct: Int)? {
    guard let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, nil),
          let img = CGImageSourceCreateImageAtIndex(src, 0, nil) else { return nil }
    let w = img.width, h = img.height
    var px = [UInt8](repeating: 0, count: w * h * 4)
    guard let ctx = CGContext(data: &px, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                              space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
    ctx.draw(img, in: CGRect(x: 0, y: 0, width: w, height: h))
    var seen = Set<UInt32>()
    let step = max(1, (w * h) / 4000)
    var i = 0
    while i < w * h {
        seen.insert(UInt32(px[i*4]) << 24 | UInt32(px[i*4+1]) << 16 | UInt32(px[i*4+2]) << 8 | UInt32(px[i*4+3]))
        i += step
    }
    return (w, h, seen.count)
}

extension Contract {
    /// `shot <name>`: PNG of the front window at $WHOOSH_SOCKET_DIR/shots/<name>.png (docs/CONTROL-SOCKET.md).
    @Suite("shot") struct ShotVerb {
        @Test func aShotIsAPngOfTheWindowAtTheNamedPath() throws {
            let dir = try #require(Ctl.appDir)
            let r = try Ctl.ok("shot", ["contract-shot"])
            #expect(r.keys == ["ok", "path"])
            let path = try #require(r.fields["path"] as? String)
            #expect(path == dir + "/shots/contract-shot.png")
            let head = try #require(FileManager.default.contents(atPath: path)?.prefix(8))
            #expect(Array(head) == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A], "PNG signature")
            let facts = try #require(pngFacts(path), "the file decodes as an image")
            // the picture is of the front window: its width is the window frame's width times a whole backing scale,
            // and its height (title bar excluded) is more than half of and no more than the frame's
            let frames = try #require(try Ctl.ok("winframes").fields["frames"] as? [[String: Any]])
            let main = try #require(frames.max { ($0["w"] as? Double ?? 0) * ($0["h"] as? Double ?? 0) < ($1["w"] as? Double ?? 0) * ($1["h"] as? Double ?? 0) })
            let fw = try #require(main["w"] as? Double), fh = try #require(main["h"] as? Double)
            let scale = Double(facts.w) / fw
            #expect(scale >= 1 && scale == scale.rounded(), "image width \(facts.w) is the window width \(fw) times a whole scale")
            #expect(Double(facts.h) / scale <= fh && Double(facts.h) / scale > fh / 2, "image height \(facts.h)/\(scale) fits the window height \(fh)")
            #expect(facts.distinct >= 8, "the picture is the window's content, not a blank: \(facts.distinct) distinct colours in the sample")
        }

        @Test func aShotFollowsTheScreenNotAStaleCache() throws {
            let a = try #require(try Ctl.ok("shot", ["contract-before"]).fields["path"] as? String)
            try Ctl.ok("key", ["E"])
            defer { _ = try? Ctl.ask("key", ["Z"]) }
            let b = try #require(try Ctl.ok("shot", ["contract-after"]).fields["path"] as? String)
            let da = try #require(FileManager.default.contents(atPath: a)), db = try #require(FileManager.default.contents(atPath: b))
            #expect(da != db, "archiving a row changes the pixels of the next shot")
        }

        @Test func noNameIsRefusedInWords() throws {
            let r = try Ctl.ask("shot")
            #expect(!r.ok)
            #expect(r.error == "shot needs a name")
        }

        @Test func namesOutsideTheAllowedAlphabetAreRefusedAndWriteNothing() throws {
            let dir = try #require(Ctl.appDir)
            let escape = "contract-escape"
            for bad in ["a/b", "../" + escape, "a b", "", "x;y", "é", "x\u{0}y"] {
                let r = try Ctl.ask("shot", [bad])
                #expect(!r.ok, "shot \(bad.debugDescription) must be refused")
                #expect(r.error == "a shot name may hold only A-Z a-z 0-9 . _ -", "\(bad.debugDescription): \(r.error ?? "")")
            }
            #expect(!FileManager.default.fileExists(atPath: dir + "/shots/../" + escape + ".png"))
            #expect(!FileManager.default.fileExists(atPath: dir + "/" + escape + ".png"))
            // positive control: the same probe path finds a file a valid shot wrote
            let ok = try #require(try Ctl.ok("shot", ["contract-control"]).fields["path"] as? String)
            #expect(FileManager.default.fileExists(atPath: ok), "the probe path can see a real shot")
        }
    }

    /// `capture shot <name>`: the same PNG under the long name (delta 6).
    @Suite("capture") struct CaptureVerb {
        @Test func captureShotRepliesWithThePathOfTheSamePng() throws {
            let dir = try #require(Ctl.appDir)
            let r = try Ctl.ok("capture", ["shot", "contract-capture"])
            #expect(r.keys == ["ok", "path"])
            let path = try #require(r.fields["path"] as? String)
            #expect(path == dir + "/shots/contract-capture.png")
            #expect(pngFacts(path) != nil)
        }

        @Test func anythingButCaptureShotNameIsUsageNotCoercion() throws {
            for args in [[], ["shot"], ["video", "x"], ["shot", "a", "b"]] as [[Any]] {
                let r = try Ctl.ask("capture", args)
                #expect(!r.ok, "capture \(args)")
                #expect(r.error == "usage: capture shot <name>", "capture \(args): \(r.error ?? "")")
            }
        }
    }
}
