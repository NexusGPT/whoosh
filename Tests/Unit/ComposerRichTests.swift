import Foundation
import Testing
@testable import WhooshUI

// gap-composer-rich (item 05d1d9c3 parts 1 and 2): a signature with formatting and a picture inside the text, on the fixture core.
// The core's own builder is proven by core/crates/send/tests/rich_body.rs; here the composer model and the fixture's record of the send.
// BROKEN COPIES (one per test, the single edit that must turn it red; listed in checks/proofs/g2/05d1d9c3.json):
//   aRichSignatureIsShownAndGoesOutWithItsBold     : ComposerRichMail.swift `case .bold: a.inlinePresentationIntent = .stronglyEmphasized` -> `case .bold: break`
//   aPlainSignatureChangesNothing                  : ComposerRichMail.swift isRich `contains { $0.mark != .plain }` -> `contains { _ in true }`
//   aPictureGoesIntoTheTextAndOutAsOneInlinePart   : ComposerModel+Send.swift `let body = ComposerRichMail.outgoingText(session.document)` -> `let body = bodyText`
//   aPictureIsNotAlsoATile                         : ComposerModel+Files.swift `attachments = fileTiles.filter { !inText.contains($0.id) } +` -> `attachments = fileTiles +`
//   aSavedDraftKeepsThePictureWhenReopened         : ComposerModel+Pictures.swift insertPicture `placePicture(id)` -> `_ = id`

@MainActor private func model() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}

@MainActor private func newMessage(_ m: AppModel, to: String, text: String) throws -> ComposerModel {
    let c = ComposerModel.shared
    c.discardDraft()
    try #require(c.openNew(m), "a composer opens on the fixture")
    c.to = [to]
    c.session.setPlain(text)
    return c
}

/// A real 1 x 1 PNG in this test's own temporary folder.
private func picture(_ name: String) throws -> URL {
    let png: [UInt8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
                        0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
                        0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82]
    let dir = FileManager.default.temporaryDirectory.appendingPathComponent("whoosh-composer-rich-tests", isDirectory: true)
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    let url = dir.appendingPathComponent(name)
    try Data(png).write(to: url)
    return url
}

/// A string path of the fixture's record, decoded from its JSON (nil when the core has no value).
@MainActor private func text(_ m: AppModel, _ path: String) -> String? {
    guard let raw = m.core.state(path: path), let data = raw.data(using: .utf8) else { return nil }
    return (try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])) as? String
}

private let richSignature = "Assem\n**Whoosh** studio, *Paris*\n[whoosh.example](https://whoosh.example)"

/// Sets the From account's signature for one test and gives back the one it had.
@MainActor private func withSignature(_ m: AppModel, _ c: ComposerModel, _ text: String, _ body: () throws -> Void) throws {
    let before = m.core.signature(address: c.from?.address ?? "")?.text ?? ""
    _ = try m.core.run(command: .setSignature(accountId: c.fromId, text: text))
    defer { _ = try? m.core.run(command: .setSignature(accountId: c.fromId, text: before)) }
    try body()
}

@MainActor @Test func aRichSignatureIsShownAndGoesOutWithItsBold() throws {
    let m = try model()
    let c = try newMessage(m, to: "nobody-rich-sig@example.com", text: "Thanks for the files.")
    defer { c.discardDraft(); _ = c.close(m) }
    try withSignature(m, c, richSignature) {
        #expect(ComposerRichMail.isRich(c.signature), "the signature has marks: \(c.signature)")
        let shown = ComposerRichMail.shown(c.signature)
        #expect(String(shown.characters) == "Assem\nWhoosh studio, Paris\nwhoosh.example", "the marks are not shown: \(String(shown.characters))")
        let bold = shown.runs.filter { $0.inlinePresentationIntent == .stronglyEmphasized }.map { String(shown[$0.range].characters) }
        #expect(bold == ["Whoosh"], "the bold is drawn: \(bold)")
        #expect(shown.runs.contains { $0.link == URL(string: "https://whoosh.example") }, "the link is a link")
        #expect(c.send(m)); #expect(c.isCounting, "\(c.line)")
        let html = try #require(text(m, "sent/last/html"), "the sent mail has an HTML part")
        #expect(html.contains("<b>Whoosh</b>"), "the HTML holds the signature's bold: \(html)")
        let plain = text(m, "sent/last/plain") ?? ""
        #expect(plain.contains("Thanks for the files.") && plain.contains("Whoosh studio, Paris") && !plain.contains("**"), "the plain part: \(plain)")
        _ = c.undo(m)
    }
}

@MainActor @Test func aPlainSignatureChangesNothing() throws {
    let m = try model()
    let c = try newMessage(m, to: "nobody-plain-sig@example.com", text: "Plain as before.")
    defer { c.discardDraft(); _ = c.close(m) }
    try withSignature(m, c, "Assem\nWhoosh studio") {
        #expect(!ComposerRichMail.isRich(c.signature), "no mark: plain")
        #expect(c.outgoing.hasPrefix("Plain as before.\n\nAssem\nWhoosh studio"), "the text that goes out is the text and the signature, as before: \(c.outgoing)")
        #expect(c.send(m)); #expect(c.isCounting, "\(c.line)")
        #expect(m.core.state(path: "sent/last/html") == nil, "a plain signature adds no HTML part")
        _ = c.undo(m)
    }
}

@MainActor @Test func aPictureGoesIntoTheTextAndOutAsOneInlinePart() throws {
    let m = try model()
    let c = try newMessage(m, to: "nobody-picture@example.com", text: "Here is the chart:")
    defer { c.discardDraft(); _ = c.close(m) }
    #expect(c.insertPicture(try picture("chart.png")), "a picture goes into the text: \(c.line)")
    let ids = ComposerRichMail.pictureIds(c.session.document)
    #expect(ids.count == 1, "one picture run in the text: \(ids)")
    let id = try #require(ids.first)
    #expect(c.outgoing.contains(ComposerRichMail.marker(id)), "the text that goes out names the picture: \(c.outgoing)")
    #expect(c.send(m)); #expect(c.isCounting, "\(c.line)")
    #expect(m.core.state(path: "sent/last/inline_parts") == "1", "one inline part")
    #expect(m.core.state(path: "sent/last/attachments") == "0", "the picture is not also an attachment")
    #expect((text(m, "sent/last/html") ?? "").contains("<img src=\"cid:"), "the HTML references the picture")
    _ = c.undo(m)
}

@MainActor @Test func aPictureIsNotAlsoATile() throws {
    let m = try model()
    let c = try newMessage(m, to: "nobody-tile@example.com", text: "Words.")
    defer { c.discardDraft(); _ = c.close(m) }
    #expect(c.insertPicture(try picture("inline.png")))
    #expect(c.attachments.isEmpty, "a picture in the text draws no file tile: \(c.attachments.map(\.name))")
    let big = FileManager.default.temporaryDirectory.appendingPathComponent("whoosh-composer-rich-tests/notes.txt")
    try "a file, not a picture".write(to: big, atomically: true, encoding: .utf8)
    #expect(!c.insertPicture(big), "a file that is not a picture is not put in the text")
    c.settleFiles()
    #expect(c.attachments.map(\.name) == ["notes.txt"], "it is attached as a file, as a drop outside the text: \(c.attachments.map(\.name))")
}

@MainActor @Test func aSavedDraftKeepsThePictureWhenReopened() async throws {
    let m = try model()
    let c = try newMessage(m, to: "nobody-reopen@example.com", text: "Before the picture.")
    defer { c.discardDraft(); _ = c.close(m) }
    #expect(c.insertPicture(try picture("kept.png")))
    let id = try #require(ComposerRichMail.pictureIds(c.session.document).first)
    #expect(c.close(m), "Escape closes and keeps the draft")
    await c.autosave(refresh: false)
    let d = try #require(c.coreDraftId, "the core holds the draft")
    #expect((m.core.state(path: "draft/\(d)/attachments") ?? "").contains(id), "the picture's file is on the core's draft")
    #expect(c.openNew(m), "the same key reopens the draft")
    #expect(ComposerRichMail.pictureIds(c.session.document) == [id], "the picture is in the text again")
    #expect(c.send(m)); #expect(c.isCounting, "\(c.line)")
    #expect(m.core.state(path: "sent/last/inline_parts") == "1", "the reopened draft sends its picture inline")
    _ = c.undo(m)
}
