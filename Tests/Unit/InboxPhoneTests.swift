import Foundation
import Testing
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

// ---- the invariants the swipes lean on, proven below the UI (C-J7.3: "same use cases as the Mac")
@Test func swipeArchiveThenSnoozeRemovesExactlyThoseTwoRowsAndWritesTwoUndoEntries() throws {
    let (c, _) = try fixtureCore()
    let before = primaryRows(c).map(\.messageId)
    let healthy = primaryRows(c).filter { $0.accountId != c.failingAccountId }.map(\.messageId)
    let (a, s) = (healthy[0], healthy[1])
    _ = try c.run(command: .archive(messageIds: [a]))
    _ = try c.run(command: .snooze(messageIds: [s], when: .tomorrowMorning))
    #expect(primaryRows(c).map(\.messageId) == before.filter { $0 != a && $0 != s })        // mutant: a swipe drops a neighbour
    #expect(c.state(path: "undo_entry/count") == "2")                                         // mutant: a swipe writes no undo entry
    #expect(c.state(path: "message/\(a)/archived") == "true")
    #expect(c.state(path: "message/\(s)/snoozed") == "true")
}
@Test func undoReversesTheLastSwipeFirstAndBringsTheRowBackAtItsPlace() throws {
    let (c, _) = try fixtureCore()
    let before = primaryRows(c).map(\.messageId)
    let healthy = primaryRows(c).filter { $0.accountId != c.failingAccountId }.map(\.messageId)
    _ = try c.run(command: .archive(messageIds: [healthy[0]]))
    _ = try c.run(command: .snooze(messageIds: [healthy[1]], when: .nextWeek))
    _ = try c.run(command: .undo)
    #expect(c.state(path: "message/\(healthy[1])/snoozed") == "false")                       // mutant: undo reverses the first swipe
    #expect(c.state(path: "message/\(healthy[0])/archived") == "true")
    _ = try c.run(command: .undo)
    #expect(primaryRows(c).map(\.messageId) == before)                                       // mutant: the row returns at the bottom
}
@Test func aRefusedSwipeLeavesTheRowAndNamesTheAccountInWords() throws {
    let (c, _) = try fixtureCore()
    let before = primaryRows(c).map(\.messageId)
    let refused = try #require(primaryRows(c).first { $0.accountId == c.failingAccountId }).messageId
    do { _ = try c.run(command: .archive(messageIds: [refused])); Issue.record("the failing account accepted an archive") }
    catch let e as CoreError {
        guard case .Action(let f) = e else { Issue.record("expected an ActionFailed, got \(e)"); return }
        #expect(f.accountName == "Provanta RFP")                                              // mutant: the reason names no account
        #expect(!f.reasonWords.isEmpty)
    }
    #expect(primaryRows(c).map(\.messageId) == before)                                       // mutant: the row is dropped before the server answers
    #expect(c.state(path: "undo_entry/count") == "0")                                         // a refused swipe writes no undo entry
}
@Test func snoozingIntoThePastIsRefused() throws {
    let (c, clock) = try fixtureCore()
    let id = try #require(primaryRows(c).first { $0.accountId != c.failingAccountId }).messageId
    clock.advance(seconds: 3600)
    do { _ = try c.run(command: .snooze(messageIds: [id], when: .at(atMs: 1))); Issue.record("a past time was accepted") }
    catch let e as CoreError { guard case .Invalid = e else { Issue.record("expected Invalid, got \(e)"); return } }   // mutant: the past time is accepted
    #expect(c.state(path: "message/\(id)/snoozed") == "false")
}

// ---- the iPhone surface (fails until the inboxphone lane registers it)
@MainActor @Test func theIphoneListShowsTheFixtureMailPinnedFirstWithAccountMarks() throws {
    let msgs = try list("messages")
    let accts = Dictionary(uniqueKeysWithValues: try list("accounts").map { ($0["id"] as! String, $0["colour"] as! String) })
    let all = try rows("iphone-inbox.list")
    // swift-tests lane: the fixture core also files the `flow_example` rows since 87517150 (see InboxListTests.theListShowsTheFixtureMailPinnedFirst); the fixture's messages stay in order.
    let flowIds = Set(try (fixtureJSON()["flow_example"] as? [[String: Any]] ?? []).compactMap { $0["id"] as? String })
    let r = all.filter { !flowIds.contains($0["id"] as? String ?? "") }
    #expect(r.map { $0["id"] as? String } == msgs.map { $0["id"] as? String })               // mutant: list sorted by time, pinned not on top
    #expect(r.map { ($0["mark_colour"] as? String)?.lowercased() } == msgs.map { accts[$0["account"] as! String]!.lowercased() })   // mutant: one colour on every row
    // mob-account-mark-iphone: the mark says the account's name to a screen reader and carries the account's icon id (the Mac's rule: the picked id, else the provider default)
    let names = Dictionary(uniqueKeysWithValues: try list("accounts").map { ($0["id"] as! String, $0["name"] as! String) })
    let icons = Dictionary(uniqueKeysWithValues: try list("accounts").map { ($0["id"] as! String, $0["icon_id"] as? String ?? "provider-default") })
    #expect(r.map { $0["mark_label"] as? String } == msgs.map { names[$0["account"] as! String] })   // mutant: "mark_label": ""
    #expect(r.map { $0["mark_icon"] as? String } == msgs.map { icons[$0["account"] as! String] })    // mutant: PhoneAccounts passes iconId nil
    #expect(Set(r.compactMap { $0["mark_icon"] as? String }).count > 1, "the rows carry more than one icon, not one default on every row")
}
@MainActor @Test func everyRowHasASwipeArchiveAndASwipeSnoozeAndTheUndoHasAButton() throws {
    #expect(Registrations.click("iphone-inbox.list.row.0.swipe-archive") != nil)              // mutant: no archive swipe
    #expect(Registrations.click("iphone-inbox.list.row.0.swipe-snooze") != nil)               // mutant: no snooze swipe
    #expect(Registrations.click("iphone-inbox.undo.button") != nil)                           // mutant: no undo control
}

// ---- the screen's own swipes, pick and refresh, driven through the registered clicks and read from the rows (the doors the socket uses)
@MainActor private func listIds() throws -> [String] { try rows("iphone-inbox.list").compactMap { $0["id"] as? String } }
/// The first unpinned row of the list on an account that accepts an archive: its index on screen and its message id.
@MainActor private func swipeTarget(_ m: AppModel) throws -> (index: Int, id: String) {
    let failing = m.host.fixture?.failingAccountId
    let r = InboxPhoneState.shared.rows(m)
    let i = try #require(r.firstIndex { $0.accountId != failing && !$0.pinned })
    return (i, r[i].messageId)
}
@MainActor private func swipe(_ ax: String, _ m: AppModel) throws {
    let c = try #require(Registrations.click(ax), "no click is registered at \(ax)")
    #expect(c(m), "the click \(ax) was refused")
}
@MainActor private func coreState(_ m: AppModel, _ path: String) throws -> String? { try #require(m.host.fixture).state(path: path) }
/// The swipe picks as a fresh install has them, read by the real loader from an empty defaults suite.
@MainActor private func freshSwipes() throws -> SwipeSettings {
    let name = "whoosh.test.swipes.\(UUID().uuidString)"
    let d = try #require(UserDefaults(suiteName: name))
    defer { d.removePersistentDomain(forName: name) }
    return SwipeSettings.load(d)
}

@MainActor @Test func aSwipeArchivesTheRowThenASwipeSnoozesAnotherAndTheUndoButtonBringsTheSnoozedRowBack() throws {
    let m = try model()
    let ui = InboxPhoneState.shared
    ui.swipes = try freshSwipes()
    defer { ui.swipes = .standard }
    ui.expireUndo(); ui.clearFailure()
    #expect(try rows("iphone-inbox.undo").isEmpty, "no undo control before a swipe")
    let first = try swipeTarget(m)
    let before = try listIds()
    try swipe("iphone-inbox.list.row.\(first.index).swipe-archive", m)
    #expect(try listIds() == before.filter { $0 != first.id })                                  // mutant: swipe-archive runs the snooze
    #expect(try coreState(m, "message/\(first.id)/archived") == "true")
    let undo = try #require(try rows("iphone-inbox.undo").first, "the undo control shows after a swipe")
    #expect(undo["button"] as? String == "Undo")
    #expect((undo["text"] as? String)?.hasPrefix("Archived") == true, "the capsule says what the swipe did, got \(String(describing: undo["text"]))")
    let second = try swipeTarget(m)
    #expect(second.id != first.id)
    let afterFirst = try listIds()
    try swipe("iphone-inbox.list.row.\(second.index).swipe-snooze", m)
    #expect(try listIds() == afterFirst.filter { $0 != second.id })
    #expect(try coreState(m, "message/\(second.id)/snoozed") == "true")
    #expect((try rows("iphone-inbox.undo").first?["text"] as? String)?.hasPrefix("Snoozed") == true)
    #expect(try coreState(m, "undo_entry/count") == "2")
    try swipe("iphone-inbox.undo.button", m)
    #expect(try listIds().contains(second.id), "undo brings the snoozed row back")
    #expect(!(try listIds().contains(first.id)), "undo reverses the last swipe only: the archived row stays archived")
    #expect(try coreState(m, "message/\(second.id)/snoozed") == "false")
    #expect(try coreState(m, "message/\(first.id)/archived") == "true")
    #expect(try rows("iphone-inbox.undo").isEmpty, "the control goes once Undo is used")
}

@MainActor @Test func aSwipeOnARowOfTheFailingAccountKeepsTheRowAndNamesTheAccountOnTheScreen() throws {
    let m = try model()
    let ui = InboxPhoneState.shared
    ui.swipes = try freshSwipes()
    defer { ui.swipes = .standard }
    ui.expireUndo(); ui.clearFailure()
    let failing = try #require(m.host.fixture?.failingAccountId)
    let i = try #require(ui.rows(m).firstIndex { $0.accountId == failing })
    let before = try listIds()
    try swipe("iphone-inbox.list.row.\(i).swipe-archive", m)
    #expect(try listIds() == before)                                                            // mutant: the row is dropped before the server answers
    let line = try #require(try rows("iphone-inbox.failure").first?["text"] as? String, "a refusal shows its line")
    #expect(line.contains("Provanta RFP"), "the refusal names the account, got \(line)")
    #expect(try rows("iphone-inbox.undo").isEmpty, "a refused swipe writes no undo control")
}

@MainActor @Test func theSwipePicksStartAsArchiveAndSnoozeAndAPickedPinSwipePinsTheRowAndKeepsIt() throws {
    let m = try model()
    let ui = InboxPhoneState.shared
    ui.swipes = try freshSwipes()
    defer { ui.swipes = .standard }
    ui.expireUndo(); ui.clearFailure()
    let defaults = try #require(try rows("iphone-inbox.swipe-settings").first)
    #expect(defaults["leading"] as? String == "archive")                                       // mutant: SwipeSettings.standard leads with another action
    #expect(defaults["trailing"] as? String == "snooze")
    try swipe("iphone-inbox.swipe-settings.leading.pin", m)
    let picked = try #require(try rows("iphone-inbox.swipe-settings").first)
    #expect(picked["leading"] as? String == "pin" && picked["trailing"] as? String == "snooze")
    #expect(try rows("iphone-inbox.list").allSatisfy { $0["swipe_leading"] as? String == "pin" && $0["swipe_trailing"] as? String == "snooze" },
            "every row serves the pick")
    #expect(SwipeSettings.load().leading == .pin, "the pick is kept on this device")
    let t = try swipeTarget(m)
    let before = try listIds()
    try swipe("iphone-inbox.list.row.\(t.index).swipe-leading", m)
    #expect(try coreState(m, "pin/\(t.id)") == "true")                                          // mutant: the leading swipe ignores the pick and archives
    #expect(Set(try listIds()) == Set(before), "a pinned row stays in the list")
    #expect(try rows("iphone-inbox.list").first { $0["id"] as? String == t.id }?["pinned"] as? Bool == true)
    #expect(try rows("iphone-inbox.undo").isEmpty, "a pin takes no row out, so no undo control shows")
    try swipe("iphone-inbox.swipe-settings.leading.archive", m)
    #expect(try rows("iphone-inbox.swipe-settings").first?["leading"] as? String == "archive", "the pick can be put back")
}

@MainActor @Test func pullToRefreshReadsTheCoreAgainStampsTheTimeAndKeepsTheRowsItHad() throws {
    let m = try model()
    let ui = InboxPhoneState.shared
    let stamp0 = try #require(try rows("iphone-inbox.refresh").first?["refreshed_at"] as? Double)
    let t = try swipeTarget(m)
    let before = try listIds()
    _ = try #require(m.host.fixture).run(command: .archive(messageIds: [t.id]))               // the core changes behind the screen's back
    #expect(try listIds() == before, "the rows on screen stay until the new read lands")
    Thread.sleep(forTimeInterval: 0.002)
    try swipe("iphone-inbox.list.refresh", m)
    let stamp1 = try #require(try rows("iphone-inbox.refresh").first?["refreshed_at"] as? Double)
    #expect(stamp1 > stamp0, "the refresh stamps a later time")                                // mutant: the refresh click reads nothing
    #expect(try listIds() == before.filter { $0 != t.id }, "the read brought the core's change in and kept every other row")
    #expect(ui.rows(m).map(\.messageId) == (try listIds()))
}

@MainActor @Test func thePhonePrimaryCountEqualsTheMacsForTheSameFixture() throws {
    let m = try model()
    let phone = try rows("iphone-inbox.tabs")
    let mac = try rows("mac-main.tabs")
    let phonePrimary = try #require(phone.first { $0["id"] as? String == "primary" }, "the phone strip has Primary")
    let macPrimary = try #require(mac.first { $0["id"] as? String == "primary" }, "the Mac strip has Primary")
    #expect(phonePrimary["unread"] as? Int == macPrimary["unread"] as? Int)                    // mutant: the phone count is off by one
    var shared = 0
    for p in phone {
        guard let same = mac.first(where: { $0["id"] as? String == p["id"] as? String }) else { continue }   // the Mac strip shows the pinned tabs only
        shared += 1
        #expect(p["unread"] as? Int == same["unread"] as? Int, "tab \(p["id"] ?? "?") unread")
    }
    #expect(shared >= 2, "the two strips share more than Primary, got \(shared)")
    #expect(phonePrimary["unread"] as? Int == Int(primary(try #require(m.host.fixture))), "and the count the core serves")
}
