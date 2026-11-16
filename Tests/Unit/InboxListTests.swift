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

@MainActor @Test func theListShowsTheFixtureMailPinnedFirst() throws {
    let msgs = try list("messages")
    let all = try rows("mac-main.list")
    // swift-tests lane: since commit 87517150 (corebridge-d17, 2026-11-09) the fixture core also files the file's `flow_example` rows (docs/design/fixtures/mac-main-sample.json,
    // `flow_example`; Sources/CoreBridge/FixtureFlow.swift:114-121) and the one the flow sends to the fallback tab, flow-rejected-1, shows in the Primary list (main76: 17 rows, not 16; the file now holds 8 such rows).
    // The fixture's own `messages` must still be in the list, in the fixture's order, with the pinned ones first; flow rows are the only extra rows allowed.
    let flowIds = Set(try (fixture()["flow_example"] as? [[String: Any]] ?? []).compactMap { $0["id"] as? String })
    let r = all.filter { !flowIds.contains($0["id"] as? String ?? "") }
    #expect(all.count - r.count <= flowIds.count, "the only extra rows are the flow example's")
    #expect(r.map { $0["id"] as? String } == msgs.map { $0["id"] as? String })      // mutant: pinned rows not on top
    #expect(all.prefix(2).allSatisfy { ($0["pinned"] as? Bool) == true })
    #expect(r.count == 16)
}
@MainActor @Test func theTabsCarryUnreadAndWaitingCounts() throws {
    let tabs = try list("tabs")
    let r = try rows("mac-main.tabs")
    // swift-tests lane: the strip is no longer the fixture file's `tabs` list. The tab list is derived from the labels by the contract's `inboxTabs(labels:)` (Sources/CoreBridge/FixtureLabels.swift:77-82):
    // main76 read primary, all, needs-reply, updates, promotions and the pinned label lbl-6, with unread 24 / 201 / 10 / 41 / 117 / 0 and waiting 4 / 10 / 6 / 0 / 0 / 0, against the file's
    // primary, needs-reply, updates, promotions, social, vips with 23 / 6 / 41 / 117 / 9 / 2. The tabs the two still share are compared with the file (Updates, Promotions: same figures);
    // Primary is the file's figure plus the flow example's rejected rows the fixture core files there (every `flow_example` row whose step is "rejected", see theListShowsTheFixtureMailPinnedFirst); Needs Reply and All
    // carry the other flow-example rows (FixtureFlow.swift:114-121), so they are not compared with the file.
    let byId = Dictionary(r.compactMap { x in (x["id"] as? String).map { ($0, x) } }, uniquingKeysWith: { a, _ in a })
    let file = Dictionary(uniqueKeysWithValues: tabs.compactMap { t in (t["id"] as? String).map { ($0, t) } })
    #expect(!byId.isEmpty, "the tab rows carry no `id`: \(r)")
    for id in ["updates", "promotions"] {
        #expect(byId[id]?["unread"] as? Int == file[id]?["unread"] as? Int, "\(id) unread")
        #expect(byId[id]?["waiting"] as? Int == file[id]?["waiting"] as? Int, "\(id) waiting")     // mutant: waiting dropped
    }
    let rejectedRows = try FixtureFileFigures.rejectedFlowRows()
    #expect(byId["primary"]?["unread"] as? Int == (file["primary"]?["unread"] as? Int).map { $0 + rejectedRows }, "primary unread is the file's plus the flow example's rejected rows")
    #expect(byId["primary"]?["waiting"] as? Int == file["primary"]?["waiting"] as? Int, "primary waiting")
}
@MainActor @Test func everyRowHasAMouseArchiveControl() throws {
    #expect(Registrations.click("mac-main.list.row.0.archive") != nil)              // mutant: no archive button
}
