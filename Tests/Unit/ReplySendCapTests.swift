import Foundation
import Testing
@testable import WhooshUI

// Spec A145 (owner 2026-11-11, "btw cmd+enter to send"): every reply control shows ⌘↵ as its send key, as the composer does;
// ⌃↵ and plain Enter keep sending, unshown. Run with WHOOSH_FIXTURE_CORE=1 (the fixture core serves the open thread).
// MUTANTS: each test names, in its trailing comment, the one-line broken copy that must turn it red.

@MainActor private func model() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()   // launch state once per test (Tests/Support/SharedAppFresh.swift)
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}
@MainActor private func undoBarRows(_ m: AppModel) throws -> Int {
    let src = try #require(Registrations.rowSource("mac-main.undo.bar"), "no rows are registered at mac-main.undo.bar")
    return try src(m).count
}
/// Opens the reply on the focused thread, types a word, presses `chord` and says whether the undo bar (the send's countdown) opened.
@MainActor private func sends(_ chord: String) throws -> Bool {
    let m = try model()
    try #require(m.press("R"), "R opens the quick reply on the focused thread")
    defer { _ = m.press("Escape") }
    Registrations.type("Thanks.", m)
    try #require(try undoBarRows(m) == 0, "the undo bar is already open before the key")
    _ = m.press(chord)
    return try undoBarRows(m) == 1
}

@MainActor @Test func theReplySendCapIsTheComposersCommandReturn() throws {
    let composer = try #require(ComposerControlKeys.entries.first { $0.id == ComposerIds.send }, "the composer lists no Send control").key
    #expect(QuickReplyCopy.sendCap == "⌘↵")                           // mutant: QuickReplyCopy.sendCap = "⌃↵"
    #expect(QuickReplyCopy.sendCap == composer)                      // mutant: QuickReplyCopy.sendCap = "⌘⏎" (a different glyph than the composer's)
    #expect(ThreadCardCopy.sendCap == QuickReplyCopy.sendCap)        // mutant: ThreadCardCopy.sendCap = "⌃↵" (the draft and failed cards keep their own constant)
}

@MainActor @Test func theNotchCardSendCapIsTheComposersCommandReturn() throws {
    #expect(NotchReply.sendCap == "⌘↵", "the notch card shows \(NotchReply.sendCap)")      // mutant: NotchReply.sendCap = "⌃↵"
    #expect(NotchReply.sendCap == QuickReplyCopy.sendCap)                                  // mutant: NotchReply.sendCap = "⌘⏎"
    let m = try model()
    try #require(m.press("R"), "R opens the quick reply on the focused thread")
    defer { _ = m.press("Escape") }
    // the row the control serves is empty while no mail is open in the notch: the cap is read on the notch by par-notch (a finding), here only that the row source exists
    let src = try #require(Registrations.rowSource(NotchIds.send), "no rows are registered at \(NotchIds.send)")
    #expect(try src(m).isEmpty)                                                            // mutant: the guard on NotchModel.shared.openItem removed (the row then answers on the slab)
}

@MainActor @Test func theReplyHelpNamesCommandReturnAndNotControlReturn() {
    let help = QuickReplyCopy.sendHelp
    #expect(help.contains("⌘↵"), "help reads \(help.debugDescription)")   // mutant: sendHelp = Words.text("Send (⌃)")
    #expect(!help.contains("⌃"), "help still names ⌃: \(help.debugDescription)")   // mutant: sendHelp = Words.text("Send (⌃↵ or ⌘↵)")
}

@Test func everyLanguageOfTheSendHelpNamesOnlyCommandReturn() throws {
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let data = try Data(contentsOf: repo.appendingPathComponent("Sources/Resources/Localizable.xcstrings"))
    let root = try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
    let strings = try #require(root["strings"] as? [String: Any])
    #expect(strings["Send (⌃↵ or ⌘↵)"] == nil, "the old two-key entry is still in the catalogue")   // mutant: the old key restored in the catalogue
    let loc = try #require(((strings["Send (⌘↵)"] as? [String: Any])?["localizations"]) as? [String: Any], "no catalogue entry \"Send (⌘↵)\"")
    #expect(loc.count == 14, "languages: \(loc.keys.sorted())")                                     // mutant: one language entry deleted
    for (lang, v) in loc {
        let value = (((v as? [String: Any])?["stringUnit"]) as? [String: Any])?["value"] as? String ?? ""
        #expect(value.contains("⌘↵") && !value.contains("⌃"), "\(lang): \(value.debugDescription)")   // mutant: one language keeps "(⌃↵ eller ⌘↵)"
    }
}

@MainActor @Test func controlReturnStillSendsFromTheReplyField() throws {
    #expect(try sends("⌃Enter"), "⌃Enter no longer sends the reply")   // mutant: the "⌃Enter" entry removed from QuickReplyFeature.keyBindings
}
@MainActor @Test func plainEnterNeverSendsFromTheReplyField() throws {
    // Owner, 13 Nov 07:27: "hitting enter actually sends ... not cmd+enter". Plain Enter adds a line; only ⌘↵, ⌃↵ and the Send button send.
    #expect(try !sends("Enter"), "plain Enter sends the reply")        // mutant: the "Enter" entry of QuickReplyFeature.keyBindings sends again
}
@MainActor @Test func commandReturnSendsFromTheReplyField() throws {
    #expect(try sends("⌘Enter"), "⌘Enter does not send the reply")     // mutant: the "⌘Enter" entry removed from QuickReplyFeature.keyBindings
}

@MainActor @Test func commandReturnAndControlReturnAreBothBoundKeys() {
    // The draft and failed cards show ⌘↵ and are reached by the same closure as ⌃↵ (no field open: the shown draft sends, else the newest failed retries).
    // That card path needs a shown draft or failed card, which the fixture does not hold at start: this test pins that both keys are bound; the card path is the lead's run of par-quickreply.
    let b = QuickReplyFeature.keyBindings()
    #expect(b["⌘Enter"] != nil, "⌘Enter is not bound")   // mutant: the "⌘Enter" entry removed
    #expect(b["⌃Enter"] != nil, "⌃Enter is not bound")   // mutant: the "⌃Enter" entry removed
}
