import Foundation
import Testing
@testable import WhooshUI

// Lane fix-signature-order (after fix-rest-app point 5 and docs/lanes/fix-core.md finding G): the text the composer hands the core is the text, then the
// signature, then the footer line (when on); the signature is always written by the app, so the core finds it and exactly one copy leaves, and the
// footer stays last. Four cases on `ComposerModel.outgoing`: signature, signature and footer, footer only, neither. The core's own rule is proven by
// core/crates/send/tests/rich_body.rs (`a_plain_signature_the_app_already_appended_is_sent_once...`, `a_signature_followed_by_the_footer_...`).
// Each test names its broken copy.

@MainActor private func model() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}

/// A composer with one line of text, the From account's signature set to `signature` ("" = none) and the footer on or off; everything put back after.
@MainActor private func outgoing(_ m: AppModel, signature: String, footer: Bool) throws -> String {
    let c = ComposerModel.shared
    c.discardDraft()
    try #require(c.openNew(m), "a composer opens on the fixture")
    defer { c.discardDraft(); _ = c.close(m) }
    c.to = ["nobody-signature-once@example.com"]
    c.session.setPlain("Thanks for the files.")
    let before = m.core.signature(address: c.from?.address ?? "")?.text ?? ""
    _ = try m.core.run(command: .setSignature(accountId: c.fromId, text: signature))
    let footerBefore = c.library.footerOn
    c.library.footerOn = footer
    defer { _ = try? m.core.run(command: .setSignature(accountId: c.fromId, text: before)); c.library.footerOn = footerBefore }
    return c.outgoing
}

private let sig = "Assem\nWhoosh studio"
@MainActor private var footerLine: String { ComposerLibrary.footerLine }   // the library is main-actor state

private func occurrences(of part: String, in text: String) -> Int { text.components(separatedBy: part).count - 1 }

@MainActor @Test func withASignatureAndNoFooterTheTextEndsWithTheSignatureOnce() throws {
    let m = try model()
    let t = try outgoing(m, signature: sig, footer: false)
    #expect(t == "Thanks for the files.\n\n" + sig, "\(t.debugDescription)")                       // mutant: assemble() drops the signature
    #expect(t.hasSuffix(sig) && occurrences(of: sig, in: t) == 1)
}

@MainActor @Test func withASignatureAndTheFooterTheTextIsTheBodyThenTheSignatureThenTheFooter() throws {
    let m = try model()
    let t = try outgoing(m, signature: sig, footer: true)
    #expect(t == "Thanks for the files.\n\n" + sig + "\n\n" + footerLine, "\(t.debugDescription)")   // mutant: the footer written above the signature again
    #expect(t.hasSuffix(footerLine), "the footer is last: \(t.debugDescription)")
    #expect(occurrences(of: sig, in: t) == 1 && occurrences(of: footerLine, in: t) == 1)
}

@MainActor @Test func withTheFooterAndNoSignatureTheFooterEndsTheTextAndNoSignatureIsMade() throws {
    let m = try model()
    let t = try outgoing(m, signature: "", footer: true)
    #expect(t == "Thanks for the files.\n\n" + footerLine, "\(t.debugDescription)")                // mutant: assemble() adds "\n\n" + signature even when it is empty
    #expect(!t.contains("Assem"))
}

@MainActor @Test func withNeitherTheTextIsTheBodyAlone() throws {
    let m = try model()
    let t = try outgoing(m, signature: "", footer: false)
    #expect(t == "Thanks for the files.", "\(t.debugDescription)")                                 // mutant: assemble() always appends "\n\n"
}

@MainActor @Test func theAssemblyIsTextThenSignatureThenFooterWhateverIsOn() {
    #expect(ComposerModel.assemble(body: "Body", signature: "A\nB", footer: nil) == "Body\n\nA\nB")
    #expect(ComposerModel.assemble(body: "Body", signature: "A\nB", footer: "Sent with Whoosh") == "Body\n\nA\nB\n\nSent with Whoosh")   // mutant: the footer above the signature
    #expect(ComposerModel.assemble(body: "Body", signature: "", footer: "Sent with Whoosh") == "Body\n\nSent with Whoosh")
    #expect(ComposerModel.assemble(body: "", signature: "A", footer: nil) == "\n\nA", "an empty body (a picture alone) keeps the old shape")
}
