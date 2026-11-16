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

@Test func theRegistryHoldsOneFeatureTypePerLaneWithAUniqueId() async throws {
    let ids = await MainActor.run { FeatureRegistry.all.map { $0.id } }
    // swift-tests lane: the registry grew from the root commit's 38 to 45 feature types as lanes landed (Sources/App/FeatureRegistry.swift, `grep -c "Feature.self"` = 45 at 27f013bc:
    // the iPhone pair, Notch, FirstStart, SettingsRepliesMac, SendWave and others). A lane that lands moves this number in the same commit; a type dropped by accident still turns it red.
    #expect(ids.count == 45)
    #expect(Set(ids).count == ids.count)           // mutant: two stubs share one id
    #expect(!ids.contains(""))
}
@MainActor @Test func aLetterKeyIsCaseBlindAndANamedKeyKeepsItsName() {
    #expect(AppModel.normalise("e") == "E")        // mutant: normalise leaves lowercase
    #expect(AppModel.normalise("Enter") == "Enter")
    #expect(AppModel.normalise("⌘K") == "⌘K")
    #expect(AppModel.normalise("shift+tab") == "shift+tab")   // coercion bait: a named key is not upper-cased
}
@MainActor @Test func aKeyBoundToNothingDoesNothingAndSaysSo() throws {
    let m = try model()
    #expect(m.press("Q") == false)                 // mutant: press returns true for any chord
    #expect(m.press("J") == true)                  // positive control: a bound key is accepted
    _ = m.press("K")                               // leave the shared model where it was
}
@MainActor @Test func focusClampsAtBothEndsOfTheList() throws {
    let m = try model()
    m.focusIndex = 0
    _ = m.moveFocus(-1)
    #expect(m.focusIndex == 0)                     // mutant: focus goes negative
    _ = m.moveFocus(10_000)
    #expect(m.focusIndex == m.rows.count - 1)      // mutant: focus runs past the last row
    m.focusIndex = 0
}
@MainActor @Test func tabsWrapAroundInBothDirections() throws {
    let m = try model()
    let ids = m.tabIds
    m.tab = ids[0]
    _ = m.switchTab(-1)
    #expect(m.tab == ids[ids.count - 1])           // mutant: Shift+Tab stops at the first tab
    _ = m.switchTab(1)
    #expect(m.tab == ids[0])
}
