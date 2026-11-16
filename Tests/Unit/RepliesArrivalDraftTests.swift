import Foundation
import Testing
@testable import WhooshUI

// Item ebde5eae (journey J10, step J10.8 "a draft written alone on arrival"): the fixture core's arrival door. `fixture deliver` reaches
// FixtureCore.deliverArrival; with the auto use on, an arrival that passes the three steps gets a live draft (made_by auto, state proposed, no
// instruction) served at `reply/draft/<thread>`, and NOTHING is sent: the outbox holds no entry (only the owner sends). Run with WHOOSH_FIXTURE_CORE=1.
// MUTANTS (one per test, the one-line broken copy that must turn it red):
//   anArrivalWithTheAutoUseOnGetsALiveDraftAndNothingIsSent : FixtureFiling.swift deliverArrival: the line `writeArrivalDraftLocked(id)` removed
//   anArrivalWithTheAutoUseOffGetsNoDraft                   : FixtureCoreArrivalDraft.swift: `guard b.uses[.auto]?.enabled == true,` -> `guard true,`
//   aMessageIsDraftedOnceAndTheDraftIsNeverSent             : FixtureCoreArrivalDraft.swift: the `never twice for one message` guard removed (a second arrival call of the same id is refused by the core anyway:
//                                                             the test hands the same message to the door directly)

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
/// A fixture core of its own over the design fixture (no shared app, no socket).
private func model() throws -> (Void, FixtureCore) {
    ((), try FixtureCore(url: repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock()))
}

private struct DraftRead: Decodable { let madeBy: String; let state: String; let instruction: String?; let threadId: String; let messageId: String }
private func draftRead(_ core: FixtureCore, thread: String) throws -> DraftRead? {
    guard let raw = core.state(path: "reply/draft/\(thread)") else { return nil }
    let d = JSONDecoder(); d.keyDecodingStrategy = .convertFromSnakeCase
    return try d.decode(DraftRead.self, from: Data(raw.utf8))
}
/// The replies are looked at (the fixture builds its reply book on first use), every message is in the auto use's scope, the auto use is `on`.
private func setUp(_ core: FixtureCore, autoOn on: Bool) throws {
    _ = try core.run(command: .setReplyFilterScope(scope: .all))
    _ = try core.run(command: .setReplyUseEnabled(kind: .auto, enabled: on))
}
private func arrive(_ core: FixtureCore, _ id: String) throws {
    let acc = try #require(core.accounts().first?.id, "the fixture has an account")
    _ = try core.deliverArrival(id: id, account: acc, from: "marta@vogel-example.de", sender: "Marta Vogel", subject: "Can you confirm the delivery date?",
                                body: "Hi, can you confirm that the delivery still lands on Thursday?")
}

@Test func anArrivalWithTheAutoUseOnGetsALiveDraftAndNothingIsSent() throws {
    let (_, core) = try model()
    try setUp(core, autoOn: true)
    let outbox0 = core.outbox().count
    try arrive(core, "msg-arrival-on")
    let d = try #require(try draftRead(core, thread: "msg-arrival-on"), "no draft is served at reply/draft/msg-arrival-on")
    #expect(d.madeBy == "auto")
    #expect(d.state == "proposed")
    #expect(d.instruction == nil)
    #expect(d.threadId == "msg-arrival-on" && d.messageId == "msg-arrival-on")
    #expect(core.outbox().count == outbox0, "nothing is sent alone: the outbox has \(core.outbox().count) entries, it had \(outbox0)")
}

@Test func anArrivalWithTheAutoUseOffGetsNoDraft() throws {
    let (_, core) = try model()
    try setUp(core, autoOn: false)
    try arrive(core, "msg-arrival-off")
    #expect(try draftRead(core, thread: "msg-arrival-off") == nil, "the auto use is off: no draft")
}

@Test func aMessageIsDraftedOnceAndTheDraftIsNeverSent() throws {
    let (_, core) = try model()
    try setUp(core, autoOn: true)
    let outbox0 = core.outbox().count
    try arrive(core, "msg-arrival-once")
    let first = try #require(try draftRead(core, thread: "msg-arrival-once"))
    core.lock.withLock { core.writeArrivalDraftLocked("msg-arrival-once") }   // the same message handed again
    let n = core.lock.withLock { core.replyBookLocked().drafts.filter { $0.messageId == "msg-arrival-once" }.count }
    #expect(n == 1, "\(n) drafts for one message")
    #expect(try draftRead(core, thread: "msg-arrival-once")?.messageId == first.messageId)
    #expect(core.outbox().count == outbox0)
}
