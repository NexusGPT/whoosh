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

@MainActor @Test func theSnoozePickerOffersTimesInWords() throws {
    let titles = try rows("mac-main.snooze.options").compactMap { ($0["title"] as? String)?.lowercased() }
    #expect(Array(titles.prefix(3)) == ["later today", "tomorrow morning", "next week"])   // mutant: raw dates only
    #expect(titles.count >= 4)                     // the fourth is a date picker
}
@MainActor @Test func theLabelPickerListsLabelsFromEveryAccountNested() throws {
    let names = try rows("mac-main.label.picker").compactMap { $0["name"] as? String }
    #expect(names.contains("Suppliers"))           // mutant: only top-level labels
    #expect(names.contains("Nordlicht"))           // a grand-child of Work > Clients
}
@Test func theSnoozeConstantsAreOrderedWords() {
    #expect(WhooshConstants.UNDO_SEND_MIN < WhooshConstants.UNDO_SEND_DEFAULT_S)     // mutant: default below the minimum
    #expect(WhooshConstants.UNDO_SEND_DEFAULT_S < WhooshConstants.UNDO_SEND_MAX)
}
