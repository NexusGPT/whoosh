import Foundation
import Testing
@testable import WhooshUI

@MainActor private func model() throws -> AppModel {
    // The shared model reads its core once, from the launch flag; point it at the design fixture before first use.
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()   // launch state once per test (Tests/Support/SharedAppFresh.swift)
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    m.refresh()
    return m
}

// Written from docs/JOURNEYS.md J6.6 and docs/CAPABILITY-CONTRACT.md C-J6.6, never from a built screen.
// A test of a feature that is still a stub FAILS on purpose. MUTANTS: each test names, in its trailing comment,
// the mutation of the target that must turn it red.

@MainActor @Test func settingsGeneralIsTheMacSettingsGeneralSurface() {
    #expect(SettingsGeneralMacFeature.surfaces == ["mac-settings-general"])                  // mutant: the stub claims mac-main
}

@MainActor @Test func theSettingsTabsAreTheSixOfTheJourneyInOrder() throws {
    // swift-tests lane: the lane no longer fills `settingsTabs` (the protocol default is []); "The old six tabs are sections now" (Sources/Features/SettingsGeneralMac/SettingsGeneralMacFeature.swift:54)
    // and the six are served as the rows of `mac-settings-general.tabs`, one `...tab.<id>` each, in `GeneralStore.Tab.allCases` order (GeneralStore.swift:13-14, SettingsGeneralMacFeature.swift:24).
    // The ids are compared, not the titles, which follow the Mac's language.
    let src = try #require(Registrations.rowSource("mac-settings-general.tabs"), "no rows are registered at mac-settings-general.tabs")
    let ids = try src(SharedApp.begin()).compactMap { ($0["ax"] as? String)?.split(separator: ".").last.map(String.init) }
    #expect(ids == ["shortcuts", "notch", "notifications", "privacy", "appearance", "sync"])   // mutant: Privacy and Notifications swap
}

@MainActor @Test func theNotchControlsAndTheTabRowsAreRegistered() {
    let rows = Set(SettingsGeneralMacFeature.rowSources().keys)
    let clicks = Set(SettingsGeneralMacFeature.clickTargets().keys)
    let needRows: Set<String> = ["mac-settings-general.tabs", "mac-settings-general.notch.error"]
    let needClicks: Set<String> = ["mac-settings-general.notch.hidden", "mac-settings-general.notch.focus-filter",
                                   "mac-settings-general.notch.buttons.archive", "mac-settings-general.notch.buttons.mark-read",
                                   "mac-settings-general.notifications.account.acc-05"]
    #expect(needRows.subtracting(rows).sorted() == [], "rows missing: \(needRows.subtracting(rows).sorted())")        // mutant: the error line is not registered
    #expect(needClicks.subtracting(clicks).sorted() == [], "controls missing: \(needClicks.subtracting(clicks).sorted())")   // mutant: the Mark read checkbox is not registered
}

@MainActor @Test func everyAccountNotifiesUntilTheOwnerMutesIt() throws {
    // C-J6.1 reads notification-setting; an account with no stored choice is on (a muted account is the exception).
    #expect(try model().core.state(path: "notification_setting/acc-05/enabled") == "true")   // mutant: unknown accounts default to muted
}

@MainActor @Test func theFocusFilterStartsOff() throws {
    #expect(try model().core.state(path: "notch_setting/follow_focus") == "false")       // mutant: the filter starts on
}
