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

@Test func theWindowSizeComesFromTheDesign() throws {
    let w = try #require(try fixture()["window"] as? [String: Any])
    #expect(w["w"] as? Int == 1280 && w["h"] as? Int == 820)
    #expect(w["min_w"] as? Int == 960 && w["min_h"] as? Int == 600)                 // mutant: minimum 800x500
}
@MainActor @Test func theShellOwnsMacMainAndTheLaneSurfacesDoNotCollide() {
    let all = FeatureRegistry.all.flatMap { $0.surfaces }
    #expect(all.contains("mac-main"))
    // swift-tests lane: "mac-main" is the ONE window that seven features draw regions into (the shell, MessageBody, Sidebar, InboxList, Reader, QuickReply,
    // AttachmentsMac: `grep -rn 'surfaces' Sources/Features`; main76: 35 entries, 29 distinct, the 6 repeats are all "mac-main"; Sources/App/FeatureModule.swift:49-50 treats it as
    // the shared case). The old check counted that sharing as a collision. A collision is two features claiming the same OTHER surface.
    let others = all.filter { $0 != "mac-main" }
    #expect(Set(others).count == others.count, "two features draw one surface")      // mutant: two lanes claim mac-settings-ai
}
@MainActor @Test func theFailingAccountLineLinksToSettingsAccounts() throws {
    let line = try rows("mac-main.sync.line")
    #expect(line.first?["link"] as? String == "mac-settings-accounts")              // mutant: no link
}
