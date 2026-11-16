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

// FALLBACK, not a ledger value: docs/CONSTANTS.md holds no SYNC_RECENT_S yet (proposed row in docs/lanes/gap-phone-b.md). When the lead adds the row and the generated
// constant, this line becomes `Double(WhooshConstants.SYNC_RECENT_S)` and nothing else here changes.
private let syncRecentS: Double = 300

// J4.11 (Sync tab) and J4.8 (notification settings mirrored from the Mac) on the iPhone.
// Broken copy: delete the `settings_sync/devices` case of FixtureCore.state(path:) (the row reads "no Mac", the device is empty), or serve `last_sync_ms` as an hour ago (age_s fails).
// The sentence is read in the app's own language (Words.text), never as an English literal: the catalogue answers in the Mac's language.
@MainActor @Test func theSyncRowNamesTheMacWithARecentTimeAndOffersRetryOnlyWhenBroken() throws {
    let r = try rows("iphone-settings.sync")
    let row = try #require(r.first)
    let line = try #require(row["line"] as? String)
    let device = try #require(row["device"] as? String)
    let inSync = Words.text("In sync with %@", device)
    #expect(line == inSync || (row["retry"] as? Bool) == true)                                // mutant: a broken sync says nothing and offers nothing
    #expect(!device.isEmpty)                                                                  // mutant: the Mac is not named
    if line == inSync {
        #expect((row["age_s"] as? Double ?? 1e9) < syncRecentS)                                       // mutant: a stale time shown as current
        #expect((row["retry"] as? Bool) == false)                                             // mutant: retry shows while in sync
    }
}
// Broken copy: remove `"iphone-settings.notifications"` from SettingsPhoneFeature.rowSources(), or build the rows from `AccountsSession.shared.sections` (grouped order, not the core's).
@MainActor @Test func oneNotificationRowPerAccountInFixtureOrder() throws {
    let accts = try list("accounts").map { $0["id"] as! String }
    let r = try rows("iphone-settings.notifications")
    #expect(r.map { $0["id"] as? String } == accts)                                           // mutant: an account is missing or reordered
}
@Test func theNotificationSettingContractCarriesAnAccountAndAFlag() {
    let s = NotificationSetting(accountId: "acc-02", enabled: false)
    #expect(s == NotificationSetting(accountId: "acc-02", enabled: false))
    #expect(s != NotificationSetting(accountId: "acc-02", enabled: true))                     // mutant: equality ignores the flag
    #expect(s != NotificationSetting(accountId: "acc-03", enabled: false))                    // mutant: equality ignores the account
}

// mob-settingsphone (proof file checks/proofs/g2/ae4d1818.json): the two branches of the Sync row, each forced (the test above passes on either).
// The reading is built by hand, so the healthy branch cannot be skipped: no fixture state is involved.
// mutants (exact edits in docs/lanes/gap-phone-b.md): SettingsPhoneSync.swift `case .inSync: return SettingsPhoneWords.inSync(r.device)` -> `case .inSync: return SettingsPhoneWords.noMac`;
//                                                      `var retry: Bool { phase == .failing }` -> `var retry: Bool { phase == .inSync }`
@MainActor @Test func aHealthySyncReadsInSyncWithTheMacWithARecentTimeAndNoRetry() throws {
    try LanguagePin.with("en") {
        let now: Int64 = 1_700_000_000_000
        let r = SyncReading(phase: .inSync, device: "Mac", lastMs: now - 60_000, reason: "")
        #expect(SettingsPhoneSync.line(r) == "In sync with Mac")
        #expect(r.retry == false)
        let age = try #require(r.age(nowMs: now))
        #expect(Double(age) < syncRecentS && age >= 0, "the last sync reads as recent: \(age) s")
        let old = SyncReading(phase: .inSync, device: "Mac", lastMs: now - Int64(syncRecentS) * 1000 - 1000, reason: "")
        #expect(Double(try #require(old.age(nowMs: now))) > syncRecentS, "positive control: a time past SYNC_RECENT_S is not recent")
    }
}
@MainActor @Test func aFailingSyncShowsItsReasonAndOffersRetry() throws {
    try LanguagePin.with("en") {
        let r = SyncReading(phase: .failing, device: "Mac", lastMs: nil, reason: "The Mac did not answer.")
        #expect(r.retry == true)
        #expect(SettingsPhoneSync.line(r) != "In sync with Mac")
        #expect(SettingsPhoneSync.detail(r) == "The Mac did not answer.", "the core's reason is shown")
    }
}
