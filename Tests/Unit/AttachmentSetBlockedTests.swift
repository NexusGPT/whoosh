import Foundation
import Testing
@testable import WhooshUI

// Owner, 2026-11-10 22:28, on a real mail: "this attachment is weird" (a file named whoosh-blocked.png, "Not on this Mac
// yet", on a mail that carried one blocked tracker and no file). The core replaces a blocked remote picture with
// cid:whoosh-blocked; that placeholder names no part of the message, so it is never a file of the mail.

@Test func aBlockedRemotePictureIsNotAFile() {
    let html = "<p>Bonsoir Assem,</p><img src=\"cid:whoosh-blocked\" width=\"1\" height=\"1\">"
    let set = AttachmentSet.make(messageId: "m", names: [], html: html, count: 0)
    #expect(set.isEmpty)                    // mutant: drop the filter in inlineSources -> one file "whoosh-blocked.png"
    #expect(set.inline.isEmpty)
}

@Test func aRealInlinePictureStaysListedBesideABlockedOne() {
    let html = "<img src=\"cid:whoosh-blocked\"><img src=\"cid:logo@mail\"><img src='CID:whoosh-blocked'>"
    let set = AttachmentSet.make(messageId: "m", names: [], html: html, count: 0)
    #expect(set.media.map(\.name) == ["logo@mail.png"])   // mutant: filter every cid: -> empty
}

@Test func theBlockedPlaceholderIsTheCoresOwn() throws {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let rust = try String(contentsOf: root.appendingPathComponent("core/crates/parse/src/sanitise.rs"), encoding: .utf8)
    #expect(rust.contains("BLOCKED_IMAGE_SRC: &str = \"\(AttachmentSet.blockedImageSource)\""))   // mutant: rename one side
}
