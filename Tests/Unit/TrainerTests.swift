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

@Test func theHintStopsAfterThreeKeyUses() {
    #expect(WhooshConstants.TRAINER_HINT_STOP_AFTER == 3)                           // mutant: 1 or 5
}
@MainActor @Test func theOverlayListsEveryPresetKeyWithItsAction() throws {
    let preset = try #require(try fixture()["shortcuts"] as? [String: String])
    // swift-tests lane: the overlay's rows exist only while the keys overlay is open (TrainerFeature.swift:27 `guard model.isOpen(m)`, TrainerModel.swift:86 `m.overlay == overlayId`);
    // a fresh model has none open (main76: no rows), so open it the way the show-keys key does, then read.
    try model().overlay = TrainerModel.overlayId
    let r = try rows("mac-trainer.overlay.keys")
    #expect(Dictionary(uniqueKeysWithValues: r.compactMap { x in (x["key"] as? String).flatMap { k in (x["action"] as? String).map { (k, $0) } } }) == preset)   // mutant: a key missing
}
@MainActor @Test func noHintShowsBeforeAnyMouseAction() throws {
    #expect(try rows("mac-trainer.hint").isEmpty)  // mutant: hint at launch; the region must exist (the rows call throws otherwise)
}
