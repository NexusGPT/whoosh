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

@Test func theIconLimitsComeFromTheLedger() {
    #expect(WhooshConstants.ACCOUNT_ICON_MAX_BYTES == 1_048_576)                    // mutant: 1,000,000
    #expect(WhooshConstants.ACCOUNT_ICON_PX == 64)
    #expect(WhooshConstants.ACCOUNT_RING_MIN_PX == 20)
}
@MainActor @Test func theIconPickerRefusesAnOversizeFileAndSaysTheLimit() throws {
    let m = try model()
    // swift-tests lane: the id `mac-settings-accounts.icon.upload.oversize` no longer exists. The picker is opened by `mac-settings-accounts.<id>.icon` and the upload doors are
    // `mac-settings-accounts.fixture.upload-at-limit` / `.upload-over-limit` (Sources/Features/SettingsAccountsMac/SettingsAccountsMacFeature.swift:110 and :134-146); a refused upload is
    // the account's note ("The limit is 1 MB", AccountsWords.swift:39), read with `AccountsSession.shared.refusal`, not `AppModel.lastFailure`.
    let openPicker = try #require(Registrations.click("mac-settings-accounts.acc-01.icon"), "the icon picker is not registered")
    #expect(openPicker(m))
    let over = try #require(Registrations.click("mac-settings-accounts.fixture.upload-over-limit"), "the over-limit upload door is not registered")
    #expect(over(m))
    let said = AccountsSession.shared.refusal("acc-01") ?? ""
    #expect(said.contains("1"), "the refusal must name the limit (1 MB), got \(said.debugDescription)")    // mutant: an oversize icon is accepted
    let atLimit = try #require(Registrations.click("mac-settings-accounts.fixture.upload-at-limit"), "the at-limit upload door is not registered")
    AccountsSession.shared.clearNote("acc-01")
    #expect(atLimit(m))
    #expect(AccountsSession.shared.refusal("acc-01") == nil, "the limit itself is accepted")
}
