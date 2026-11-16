import Foundation
import Testing
@testable import WhooshUI

// Written from docs/JOURNEYS.md, docs/CONSTANTS.md and the fixture, never from a built screen. A test of a feature
// that is still a stub FAILS on purpose: the failure is the honest state until the lane lands.
// Run with WHOOSH_FIXTURE_CORE=1 so the shared AppModel serves the design fixture.
// MUTANTS: each test names, in its trailing comment, the mutation of the target that must turn it red.

@MainActor private func model() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()   // launch state once per test (Tests/Support/SharedAppFresh.swift)
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}
@MainActor private func rows(_ ax: String) throws -> [ControlRow] {
    let src = try #require(Registrations.rowSource(ax), "no rows are registered at \(ax) yet")
    return try src(try model())
}
private func fixture() throws -> [String: Any] {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let data = try Data(contentsOf: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"))
    return try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
}
private func list(_ key: String) throws -> [[String: Any]] { try #require(try fixture()[key] as? [[String: Any]]) }

@MainActor @Test func theQuickReplyFieldIsFromTheReceivingAddressWithItsSignature() throws {
    let q = try #require((try fixture()["open_thread"] as? [String: Any])?["quick_reply"] as? [String: Any])
    // swift-tests lane: the field row is empty until a quick reply is open (Sources/Features/QuickReply/QuickReplyFeature.swift:20 `guard model.isOpen else { return [] }`;
    // QuickReplyModel.swift:57 "R: open the focused thread and the field under it"). The app no longer starts with one open, so the test opens it with the same key.
    let m = try model()
    try #require(m.press("R"), "R opens the quick reply on the focused thread")
    defer { _ = m.press("Escape") }
    let f = try rows("mac-main.quickreply.field")
    #expect(f.first?["from"] as? String == q["from"] as? String)                    // mutant: the default account
    // two-line signature since f1ee4e4b ("Best,\n" + the one line the fixture file's quick_reply block still holds); same reason as ComposerMacTests.fromAddressAndSignatureFollowTheReceivingAccount
    let shown = try #require(f.first?["signature"] as? String)
    let oneLine = try #require(q["signature"] as? String)
    #expect(shown == oneLine || shown.hasSuffix("\n" + oneLine), "quick reply signature \(shown.debugDescription) is not the account's signature \(oneLine.debugDescription)")
}
@Test func theUndoSendWindowStaysInsideItsBounds() {
    #expect((WhooshConstants.UNDO_SEND_MIN...WhooshConstants.UNDO_SEND_MAX).contains(WhooshConstants.UNDO_SEND_DEFAULT_S))   // mutant: default 31
    #expect(WhooshConstants.QR_EXPIRY_S == 180)
}
@MainActor @Test func theUndoBarCountsFromTheDefaultWindow() throws {
    // swift-tests lane: the bar is the quick reply's own countdown (QuickReplyFeature.swift:48-50 `guard case .counting = model.phase`), shown only after a send from it; a fresh model has
    // none, so send one the way a person does: R opens the field, type, click Send. (A core `quickSend` queues an outbox entry but does not start this countdown.)
    // The countdown reads the real clock (`secondsLeft(at: Date())`), so a read a few milliseconds after the send is the default or one second under it.
    let m = try model()
    try #require(m.press("R"), "R opens the quick reply on the focused thread")
    Registrations.type("Thanks.", m)
    let send = try #require(Registrations.click(QuickReplyIds.send), "the quick reply's Send is not registered")
    try #require(send(m), "the quick reply did not send")
    let bar = try rows("mac-main.undo.bar")
    let seconds = try #require(bar.first?["seconds"] as? Int, "no undo bar after the send")
    #expect((WhooshConstants.UNDO_SEND_DEFAULT_S - 1...WhooshConstants.UNDO_SEND_DEFAULT_S).contains(seconds), "counts from the default window, got \(seconds)")   // mutant: counts from 30
}
