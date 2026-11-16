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

@MainActor @Test func theSidebarListsEveryAccountInFixtureOrderWithItsUnreadCount() throws {
    let accts = try list("accounts")
    let r = try rows("mac-main.sidebar.accounts")
    #expect(r.map { $0["id"] as? String } == accts.map { $0["id"] as? String })      // mutant: sorted by name
    #expect(r.map { $0["unread"] as? Int } == accts.map { $0["unread"] as? Int })
    #expect(r.count == 30)
}
@MainActor @Test func theFailingAccountIsMarked() throws {
    let r = try rows("mac-main.sidebar.accounts")
    #expect(r.filter { ($0["status"] as? String) == "sign in again" }.map { $0["id"] as? String } == ["acc-18"])   // mutant: unmarked
}
@MainActor @Test func mailboxesComeInTheDesignedOrder() throws {
    let r = try rows("mac-main.sidebar.mailboxes").compactMap { $0["name"] as? String }
    // swift-tests lane: the sidebar lane re-ordered and extended the Mailboxes group on purpose (docs/lanes/sidebar.md:69, "Mailboxes now: Outbox (while something waits) ·
    // All inboxes · VIPs · Pinned · Snoozed · Drafts · Sent · Junk · Trash · Archive"; Sources/Features/Sidebar/SidebarState.swift:12). The old expectation (Sent before Drafts, six rows) predates it.
    // The Outbox row exists only while an entry waits (none on the fresh fixture), so the list starts at All inboxes.
    #expect(r == ["All inboxes", "VIPs", "Pinned", "Snoozed", "Drafts", "Sent", "Junk", "Trash", "Archive"])   // mutant: Sent before Drafts
}
