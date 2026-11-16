import Foundation
import Testing
@testable import WhooshUI

// Written from docs/JOURNEYS.md J3, docs/CONSTANTS.md and the design fixture, never from a built screen.
// A test of a feature that is still a stub FAILS on purpose: the failure is the honest state until the composermac lane lands.
// MUTANTS (one per test, the mutation that must turn it red):
//   everyRegionJ3ReadsIsRegistered      : the lane drops one region from rowSources()
//   everyKeyJ3PressesIsBound            : the lane leaves one chord unbound
//   everyControlJ3ClicksIsRegistered    : the lane leaves one control unregistered
//   fromAddressAndSignatureFollowTheReceivingAccount : the header shows another account's address or signature
//   theUndoWindowTheCodeUsesIsTheLedgerNumber        : UNDO_SEND_DEFAULT_S in Constants.swift drifts from docs/CONSTANTS.md
//   aNewMessageSendsADraftTheCoreHolds               : ComposerModel+Send.swift submit: `.send(draftId: try saveNewDraft(m))` becomes `.send(draftId: "draft-new")`
//   aSendTheCoreLetGoReadsSentNotCancelled           : ComposerModel+Send.swift reconcile: `if wentOut(m) { expire(m) } else { phase = .editing }` becomes `phase = .editing`
//   aSentMessageCountsOnceForItsRecipient            : FixtureCoreComposer.swift stateComposerServed: `guard case .sent = e.state` becomes `guard true`
//   anAIDraftNotesItsJobAndTheKeysReturnToTheText    : ComposerModel+AI.swift runDraft: the `noteAIJob` line or the `focus(.body)` line removed

@MainActor private func model() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()   // launch state once per test (Tests/Support/SharedAppFresh.swift)
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}
private func repo() -> URL { URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent() }
private func json(_ rel: String) throws -> [String: Any] {
    try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: repo().appendingPathComponent(rel))) as? [String: Any])
}

@MainActor @Test(arguments: ["mac-composer.body", "mac-composer.header", "mac-composer.undo.bar", "mac-composer.ai-marks",
                             "mac-composer.rewrite.choices", "mac-composer.send-later.options", "mac-composer.runs"])
func everyRegionJ3ReadsIsRegistered(ax: String) throws {
    #expect(Registrations.rowSource(ax) != nil, "no rows are registered at \(ax) yet (J3 reads it)")
}

@MainActor @Test(arguments: ["⌘Enter", "Z", "⌘Z", "⌘⇧L", "Fn:down", "Fn:up"])
func everyKeyJ3PressesIsBound(chord: String) throws {
    #expect(Registrations.key(chord) != nil, "\(chord) is bound to nothing (J3 presses it)")
}

@MainActor @Test(arguments: ["mac-main.reader.open-full-composer", "mac-composer.ai-draft", "mac-composer.rewrite",
                             "mac-composer.rewrite.shorter", "mac-composer.send-later.tomorrow-0800"])
func everyControlJ3ClicksIsRegistered(ax: String) throws {
    #expect(Registrations.click(ax) != nil, "nothing is registered at \(ax) (J3 clicks it)")
}

@MainActor @Test func fromAddressAndSignatureFollowTheReceivingAccount() throws {
    let m = try model()
    let thread = try #require(try json("docs/design/fixtures/mac-main-sample.json")["open_thread"] as? [String: Any])
    let qr = try #require(thread["quick_reply"] as? [String: Any])
    // swift-tests lane: the full composer opens from the OPEN thread; the app no longer starts with one open, so open the focused thread first (mac-main.reader.open).
    let openThread = try #require(Registrations.click("mac-main.reader.open"), "mac-main.reader.open is not registered")
    _ = openThread(m)
    let open = try #require(Registrations.click("mac-main.reader.open-full-composer"), "the full-composer control is not registered")
    _ = open(m)
    let src = try #require(Registrations.rowSource("mac-composer.header"), "no rows at mac-composer.header")
    let row = try #require(try src(m).first)
    #expect(row["from"] as? String == qr["from"] as? String)
    // swift-tests lane: each fixture account now carries a TWO-line signature, "Best,\n<the old one line>" (commit f1ee4e4b, contracts delta 15: AccountSummary.signature); the quick_reply
    // block of the fixture file still holds the one-line form, which is the last line of what the composer shows (main76-style run: "Best,\nAssem Chammah · Nordlicht" vs "Assem Chammah · Nordlicht").
    let shown = try #require(row["signature"] as? String)
    let oneLine = try #require(qr["signature"] as? String)
    #expect(shown == oneLine || shown.hasSuffix("\n" + oneLine), "composer signature \(shown.debugDescription) is not the account's signature \(oneLine.debugDescription)")
}

/// The undo window is the ledger's number; the generated Swift constant must equal the markdown row (every number lives once).
@Test func theUndoWindowTheCodeUsesIsTheLedgerNumber() throws {
    let md = try String(contentsOf: repo().appendingPathComponent("docs/CONSTANTS.md"), encoding: .utf8)
    let row = try #require(md.split(separator: "\n").first { $0.hasPrefix("| UNDO_SEND_DEFAULT_S") }.map(String.init))
    let cells = row.split(separator: "|").map { $0.trimmingCharacters(in: .whitespaces) }
    let nums = cells[1].split(separator: "·").compactMap { Int($0.trimmingCharacters(in: .whitespaces)) }
    #expect(nums == [WhooshConstants.UNDO_SEND_DEFAULT_S, WhooshConstants.UNDO_SEND_MIN, WhooshConstants.UNDO_SEND_MAX])
}


// ---- the gap-composer lane: the new-message send, the window's end, the per-recipient count, the AI draft's job note

@MainActor private func newMessage(_ m: AppModel, to: String, text: String) throws -> ComposerModel {
    let c = ComposerModel.shared
    c.discardDraft()
    try #require(c.openNew(m), "a composer opens on the fixture")
    c.to = [to]
    c.session.setPlain(text)
    return c
}

/// J6.7 sends a message composed from scratch: it has no core draft yet, so the send saves one and sends that id (never the stand-in "draft-new").
@MainActor @Test func aNewMessageSendsADraftTheCoreHolds() throws {
    let m = try model()
    let c = try newMessage(m, to: "nobody-new-send@example.com", text: "A new message from scratch.")
    defer { c.discardDraft(); _ = c.close(m) }
    let before = m.core.outbox().count
    #expect(c.send(m))
    #expect(c.isCounting, "the send opened the undo window: \(c.line)")
    let entries = m.core.outbox()
    #expect(entries.count == before + 1, "one entry was queued")
    let entry = try #require(entries.first { $0.id == c.outboxId })
    #expect(entry.draft.id != "draft-new", "the entry holds a draft id the core made")
    #expect(entry.draft.body.contains("A new message from scratch."), "the core sent the text it was given")
    #expect(entry.draft.to == ["nobody-new-send@example.com"])
    _ = c.undo(m)
}

/// J6.7: the core's window ends before the bar's own; the entry leaves the open list as SENT. The composer must read that as a send that went out
/// (the bar says Sent, the composer closes), never as a cancel from elsewhere (which reopened it for editing, main83).
@MainActor @Test func aSendTheCoreLetGoReadsSentNotCancelled() throws {
    let m = try model()
    let c = try newMessage(m, to: "nobody-window@example.com", text: "Goes out when the window ends.")
    defer { c.discardDraft(); _ = c.close(m) }
    #expect(c.send(m)); #expect(c.isCounting)
    #expect(m.host.clock.advance(seconds: Double(WhooshConstants.UNDO_SEND_MAX) + 5), "the core's clock moved past the window")
    c.reconcile(m)
    #expect(c.phase == .sent, "the entry left (sent): the composer says Sent, got \(c.phase)")
}

/// The other half: an entry that is gone and never went out (cancelled elsewhere) still reopens the composer for editing.
@MainActor @Test func aSendCancelledElsewhereReopensTheComposer() throws {
    let m = try model()
    let c = try newMessage(m, to: "nobody-cancel@example.com", text: "Taken back by another window.")
    defer { c.discardDraft(); _ = c.close(m) }
    #expect(c.send(m)); let id = try #require(c.outboxId)
    _ = try m.core.run(command: .cancelSend(outboxId: id))
    c.reconcile(m)
    #expect(c.phase == .editing, "a cancelled entry hands the text back, got \(c.phase)")
}

/// `server/<address>/inbox/count`: a message counts once, when its entry is sent; never for a cancelled or still-waiting one.
@MainActor @Test func aSentMessageCountsOnceForItsRecipient() throws {
    let m = try model()
    let who = "count-me-\(UUID().uuidString.prefix(6).lowercased())@example.com"
    let path = "server/\(who)/inbox/count"
    let c = try newMessage(m, to: who, text: "Counted once.")
    defer { c.discardDraft(); _ = c.close(m) }
    #expect(m.core.state(path: path) == "0", "nothing sent yet")
    #expect(c.send(m)); let id = try #require(c.outboxId)
    #expect(m.core.state(path: path) == "0", "inside the window nothing is delivered")
    _ = try m.core.run(command: .cancelSend(outboxId: id))
    #expect(m.core.state(path: path) == "0", "a cancelled entry never counts")
    c.discardDraft()
    let d = try newMessage(m, to: who, text: "Counted once, for real.")
    #expect(d.send(m))
    #expect(m.host.clock.advance(seconds: Double(WhooshConstants.UNDO_SEND_MAX) + 5))
    #expect(m.core.state(path: path) == "1", "the sent entry counts once")
}

/// J3.3: the AI draft is the Writer agent's, written for the sending address; and the cursor returns to the text, so the next key edits the draft.
@MainActor @Test func anAIDraftNotesItsJobAndTheKeysReturnToTheText() async throws {
    let m = try model()
    let c = try newMessage(m, to: "nobody-ai@example.com", text: "")
    defer { c.discardDraft(); _ = c.close(m) }
    c.useFixtureWriter()
    _ = c.openAIField(m)
    c.aiInput = "Confirm March."
    let serial = c.focusRequest?.serial ?? 0
    #expect(c.runDraft(m))
    try await Task.sleep(for: .milliseconds(400))
    #expect(!c.aiFieldOpen, "the instruction field closed")
    #expect(c.aiMarkActive, "the draft is marked as AI text")
    #expect(m.core.state(path: "ai_job/last/agent") == "\"built-in Writer\"")
    #expect(m.core.state(path: "ai_job/last/voice_profile_address") == "\"\(c.from?.address ?? "?")\"")
    #expect(c.focusRequest?.field == .body && (c.focusRequest?.serial ?? 0) > serial, "the cursor was asked back into the text")
    c.insertAtCaret(".")
    #expect(!c.aiMarkActive, "an edit clears the AI mark")
}
