import Foundation
import Testing
#if canImport(AppKit) && !canImport(UIKit)
import AppKit
#endif
@testable import WhooshUI

// Written from docs/JOURNEYS.md, docs/CAPABILITY-CONTRACT.md, docs/CONSTANTS.md and the fixture, never from a built screen.
// A test of a surface whose lane has not landed FAILS on purpose: the failure is the honest state until the lane registers
// its rows and clicks. Each test names, in its trailing comment, the mutation of the target that must turn it red.
// Run the registered-row tests with WHOOSH_FIXTURE_CORE=1 so the shared AppModel serves the design fixture.

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func fixtureCore() throws -> (FixtureCore, FixtureClock) {
    let clock = FixtureClock()
    return (try FixtureCore(url: repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock), clock)
}
private func fixtureJSON() throws -> [String: Any] {
    let data = try Data(contentsOf: repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"))
    return try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
}
private func list(_ key: String) throws -> [[String: Any]] { try #require(try fixtureJSON()[key] as? [[String: Any]]) }
@MainActor private func model() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    setenv("WHOOSH_FIXTURE_PATH", repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()   // launch state once per test (Tests/Support/SharedAppFresh.swift)
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}
@MainActor private func rows(_ ax: String) throws -> [ControlRow] {
    let src = try #require(Registrations.rowSource(ax), "no rows are registered at \(ax) yet")
    return try src(try model())
}
private func primary(_ c: FixtureCore) -> Int { Int(c.counts().tabs.first { $0.tabId == "primary" }?.unread ?? 0) }
private func primaryRows(_ c: FixtureCore) -> [InboxRow] { c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)) }

private func unreadRows(_ c: FixtureCore) -> Int { primaryRows(c).filter(\.unread).count }
@Test func openingAThreadMarksItReadAndNeverArchivesIt() throws {
    let (c, _) = try fixtureCore()
    let unread = try #require(primaryRows(c).first { $0.unread && $0.accountId != c.failingAccountId }).messageId
    let before = unreadRows(c)
    _ = try c.run(command: .openThread(threadId: unread))
    #expect(c.state(path: "message/\(unread)/read") == "true")                               // mutant: opening leaves it unread
    #expect(c.state(path: "message/\(unread)/archived") == "false")                          // mutant: the code pill (copy) archives
    #expect(unreadRows(c) == before - 1)                                                      // read once = one unread row fewer
}
@Test func openingTheSameThreadTwiceCountsOnce() throws {
    let (c, _) = try fixtureCore()
    let unread = try #require(primaryRows(c).first { $0.unread }).messageId
    let before = unreadRows(c)
    _ = try c.run(command: .openThread(threadId: unread))
    _ = try c.run(command: .openThread(threadId: unread))
    #expect(unreadRows(c) == before - 1)                                                         // mutant: every open decrements again
}
@Test func markingReadThenUnreadRestoresTheCount() throws {
    let (c, _) = try fixtureCore()
    let id = try #require(primaryRows(c).first { $0.unread }).messageId
    let before = unreadRows(c)
    _ = try c.run(command: .markRead(messageIds: [id], read: true))
    #expect(unreadRows(c) == before - 1)
    _ = try c.run(command: .markRead(messageIds: [id], read: false))
    #expect(unreadRows(c) == before)                                                             // mutant: unread does not return the count
}
@Test func theFixtureThreadHasTheParticipantsTheScreenMustShow() throws {
    let (c, _) = try fixtureCore()
    let t = try #require(c.thread(threadId: "thr-01"))
    let want = try #require(try fixtureJSON()["open_thread"] as? [String: Any])
    #expect(t.subject == want["subject"] as? String)
    #expect(t.participants == want["participants"] as? [String])                              // mutant: participants reordered
}
@MainActor @Test func theThreadScreenRegistersTheCodePillAndItsRows() throws {
    #expect(Registrations.click("iphone-thread.code-pill") != nil)                           // mutant: no code pill control
    #expect(Registrations.click("iphone-thread.back") != nil)
    #expect(Registrations.click("iphone-thread.reply") != nil)
    // The subject row is the thread on screen: this test opens the Linea quote's thread itself (msg-01) and closes it again, so it never
    // reads whatever row another test left focused (the thread screen reads `focusedMessageId` when nothing is opened).
    let m = try model()
    defer { ThreadPhoneState.shared.reset(); m.overlay = nil }
    let i = try #require(InboxPhoneState.shared.rows(m).firstIndex { $0.messageId == "msg-01" }, "the Linea quote is in the list")
    try #require(ThreadPhoneState.shared.open(row: i, in: m))
    let r = try rows("iphone-thread.subject")
    #expect(r.first?["subject"] as? String == "Quote for the Q1 print run: 12,000 units")     // mutant: another subject
}

// ---- the screen's own pill and body row, driven through the registered click and read from the rows
/// A private pasteboard for the run, so the test never writes over the owner's clipboard.
@MainActor private func privateClipboard() {
    #if canImport(AppKit) && !canImport(UIKit)
    ThreadClipboard.board = NSPasteboard(name: NSPasteboard.Name("whoosh.test.clipboard.\(UUID().uuidString)"))
    #endif
}
@MainActor private func openTheCodeMail(_ m: AppModel) throws -> String {
    let code = "482913"                                                                         // the fixture's msg-09 (GitHub) carries login_code
    let state = ThreadPhoneState.shared
    state.reset()
    let i = try #require(InboxPhoneState.shared.rows(m).firstIndex { $0.messageId == "msg-09" }, "the code mail is in the list")
    #expect(state.open(row: i, in: m))
    return code
}

@MainActor @Test func tappingTheCodePillCopiesTheCodeMarksTheMailReadAndLeavesItInTheInbox() throws {
    let m = try model()
    privateClipboard()
    defer { ThreadPhoneState.shared.reset(); m.overlay = nil }
    let code = try openTheCodeMail(m)
    let before = try #require(try rows("iphone-thread.code-pill").first, "the code mail shows its pill")
    #expect(before["code"] as? String == code)
    #expect(before["read"] as? Bool == false, "the mail is unread before the tap")
    #expect(before["copied"] as? Bool == false)
    #expect(before["text"] as? String == Words.text("Copy"))
    #expect(before["clipboard"] is NSNull, "nothing of this screen is on the clipboard before the tap")
    let tap = try #require(Registrations.click("iphone-thread.code-pill"), "no click is registered at iphone-thread.code-pill")
    #expect(tap(m))
    let after = try #require(try rows("iphone-thread.code-pill").first)
    #expect(after["copied"] as? Bool == true)
    #expect(after["text"] as? String == Words.text("Copied"))
    #expect(after["clipboard"] as? String == code)                                              // mutant: ThreadClipboard.put(code) -> _ = code
    #expect(after["read"] as? Bool == true)                                                     // mutant: m.perform(.markRead(...)) -> _ = 0
    #expect(after["archived"] as? Bool == false)
    #expect(after["in_inbox"] as? Bool == true)
    #expect(m.host.fixture?.state(path: "message/msg-09/read") == "true")
    #expect(m.host.fixture?.state(path: "message/msg-09/archived") == "false")
    #expect(m.rows.contains { $0.messageId == "msg-09" }, "the code mail is still listed")
}

@MainActor @Test func aMailWithoutACodeShowsNoPillAndTheCodePillClickIsRefused() throws {    // control of the pill test: the pill is the code mail's only
    let m = try model()
    privateClipboard()
    defer { ThreadPhoneState.shared.reset(); m.overlay = nil }
    ThreadPhoneState.shared.reset()
    let i = try #require(InboxPhoneState.shared.rows(m).firstIndex { $0.messageId == "msg-05" })
    #expect(ThreadPhoneState.shared.open(row: i, in: m))
    #expect(try rows("iphone-thread.code-pill").isEmpty)
    let tap = try #require(Registrations.click("iphone-thread.code-pill"))
    #expect(!tap(m))
}

@MainActor @Test func aBodyTheCoreDoesNotHoldSaysNotDownloadedYetAndAHeldBodyShows() throws {
    let none = ThreadPhoneProbe.bodyRow(message: "msg-x", content: nil)
    #expect(none["state"] as? String == "not-downloaded")
    #expect(none["text"] as? String == Words.text("Not downloaded yet"))                       // mutant: Words.text("") in ThreadPhoneWords.notDownloaded
    #expect(!((none["text"] as? String) ?? "").isEmpty)
    let m = try model()
    defer { ThreadPhoneState.shared.reset(); m.overlay = nil }
    _ = try openTheCodeMail(m)
    let held = try #require(try rows("iphone-thread.body").first)
    #expect(held["state"] as? String == "shown", "the fixture core holds this body")
    #expect(!(((held["text"] as? String) ?? "").isEmpty))
}

// ---- `fixture offline on` withholds a body (Sources/CoreBridge/FixtureCore.swift `body(messageId:)`), so the screen is proved on a core that holds none
// MUTANT (both tests): FixtureCore.swift `if lock.withLock({ outboxHold.offline }) { return nil }` -> `if false { return nil }`
@Test func aCoreOfflineWithholdsABodyUntilItIsOnlineAgain() throws {
    let (c, _) = try fixtureCore()
    #expect(c.body(messageId: "msg-09") != nil, "online: the fixture holds the body")
    c.fixtureOutbox(offline: true)
    #expect(c.body(messageId: "msg-09") == nil, "offline: the core answers no body")
    c.fixtureOutbox(offline: false)
    #expect(c.body(messageId: "msg-09") != nil, "back online: the body is there again")
}

@MainActor @Test func theThreadSaysNotDownloadedYetWhileTheCoreIsOfflineAndShowsTheBodyOnceOnline() throws {
    let m = try model()
    let core = try #require(m.host.fixture)
    privateClipboard()
    defer { core.fixtureOutbox(offline: false); ThreadPhoneState.shared.reset(); m.overlay = nil }
    _ = try openTheCodeMail(m)
    let reload = try #require(Registrations.click("iphone-thread.reload"))
    #expect(try #require(try rows("iphone-thread.body").first)["state"] as? String == "shown", "online: the body shows")
    core.fixtureOutbox(offline: true)
    #expect(reload(m))
    let off = try #require(try rows("iphone-thread.body").first)
    #expect(off["state"] as? String == "not-downloaded", "offline: the core holds no body")
    #expect(off["text"] as? String == Words.text("Not downloaded yet"))
    core.fixtureOutbox(offline: false)
    #expect(reload(m))
    #expect(try #require(try rows("iphone-thread.body").first)["state"] as? String == "shown", "online again: the body shows")
}
