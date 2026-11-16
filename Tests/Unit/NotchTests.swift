import Foundation
import Testing
@testable import WhooshUI

// Written from docs/JOURNEYS.md J6, docs/CAPABILITY-CONTRACT.md C-J6.1-C-J6.5 and docs/CONSTANTS.md, never from a built screen.
// A test of a feature that is still a stub FAILS on purpose: the failure is the honest state until the notch lane lands.
// MUTANTS: each test names, in its trailing comment, the mutation of the target that must turn it red.

private func repoFile(_ rel: String) -> URL {
    URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent(rel)
}

@MainActor private func model() throws -> AppModel {
    // The shared model reads its core once, from the launch flag; point it at the design fixture before first use.
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    setenv("WHOOSH_FIXTURE_PATH", repoFile("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()   // launch state once per test (Tests/Support/SharedAppFresh.swift)
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    m.refresh()
    return m
}

/// First whole number in the value cell of a docs/CONSTANTS.md row (the ledger is the one home of every number).
private func ledgerNumber(_ name: String) throws -> Int {
    let text = try String(contentsOf: repoFile("docs/CONSTANTS.md"), encoding: .utf8)
    let line = try #require(text.split(separator: "\n").first { $0.hasPrefix("| \(name) |") }, "\(name) is not in docs/CONSTANTS.md")
    let cell = line.split(separator: "|", omittingEmptySubsequences: false)[2]
    let digits = cell.drop { !$0.isNumber }.prefix { $0.isNumber || $0 == "," }.filter { $0 != "," }
    return try #require(Int(digits), "\(name) has no number in the ledger")
}

@Test func theNotchArrivalBudgetInTheBuildIsTheLedgerNumber() throws {
    #expect(WhooshConstants.NOTCH_ARRIVAL_MS == (try ledgerNumber("NOTCH_ARRIVAL_MS")))      // mutant: the generated constant says 50000
}

@MainActor @Test func theNotchIsTheMacNotchSurface() {
    #expect(NotchFeature.surfaces == ["mac-notch"])                                         // mutant: the stub claims mac-main
}

@MainActor @Test func theNotchRegistersEveryRegionTheJourneyReads() {
    let have = Set(NotchFeature.rowSources().keys)
    let need: Set<String> = ["mac-notch.list", "mac-notch.codepill", "mac-notch.open.body", "mac-notch.open.actions",
                             "mac-notch.open.quickreply", "mac-notch.undo", "mac-notch.empty", "mac-notch.error"]
    #expect(need.subtracting(have).sorted() == [], "regions with no rows: \(need.subtracting(have).sorted())")   // mutant: the undo region is dropped
}

@MainActor @Test func theNotchRegistersEveryControlTheJourneyClicks() {
    let have = Set(NotchFeature.clickTargets().keys)
    let need: Set<String> = ["mac-notch.codepill", "mac-notch.open.quickreply", "mac-notch.settings",
                             "mac-notch.list.row.1", "mac-notch.list.row.2", "mac-notch.list.row.1.archive", "mac-notch.list.row.2.archive"]
    #expect(need.subtracting(have).sorted() == [], "controls with no click: \(need.subtracting(have).sorted())")  // mutant: the archive button is not registered
}

@MainActor @Test func theCodePillCopiesAndNeverArchivesOnItsOwnClick() throws {
    let m = try model()
    // C-J6.2: copying marks the mail read and never archives it. The click is the real entry path (socket verb `click`).
    let click = try #require(Registrations.click("mac-notch.codepill"), "nothing is registered at mac-notch.codepill yet")
    let id = try #require(m.rows.first?.messageId)
    let before = m.core.state(path: "message/\(id)/archived")
    _ = click(m)
    #expect(m.core.state(path: "message/\(id)/archived") == before)                          // mutant: the click archives the mail
}

@MainActor @Test func theNotchDefaultsToHiddenFromScreenShares() throws {
    // J6.6 "keeps 'hidden from screen shares' on": on is the first-launch default (docs/DATA-MAP.md notch-setting).
    #expect(try model().core.state(path: "notch_setting/hidden_in_screen_share") == "true")   // mutant: the default is false
}
