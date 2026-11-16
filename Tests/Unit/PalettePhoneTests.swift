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

// J1.9 parity (C-P-palette): "the same command registry as the Mac, minus Mac-only commands".
private let macOnly = ["notch"]
/// The phone palette lists its lines only while it is open (`PalettePhoneModel.open()` reads the pool); the test opens it the way the pill does, through its registered click.
@MainActor private func openPalette() throws {
    let open = try #require(Registrations.click("iphone-palette.open"), "no click registered at iphone-palette.open")
    let app = try model()
    let opened = open(app)
    try #require(opened, "the palette did not open")
}
/// Leaves the palette as a test found it (closed), through the registered door the pill's close uses. Every test that opens the palette defers this.
@MainActor private func closePalette() {
    _ = Registrations.click("iphone-palette.close")?(AppModel.shared)
}
@MainActor @Test func thePhonePaletteListsNoMacOnlyCommandAndKeepsTheSharedOnes() throws {
    try openPalette()
    defer { closePalette() }
    let r = try rows("iphone-palette.results")
    let titles = r.compactMap { $0["title"] as? String }
    #expect(titles.contains("Archive"))                                                       // positive control: the probe sees a command shared with the Mac (the phone leaves "Move to another account" out on purpose: docs/lanes/palettephone.md)
    #expect(titles.filter { t in macOnly.contains { t.lowercased().contains($0) } }.isEmpty)  // mutant: Notch settings listed on the phone
}
// What leaked into this test: `PalettePhoneModel.shared` keeps the pool of lines it read at its last open() (commands AND the places to go: mailboxes,
// accounts, tabs, labels) after the palette closes, and `iphone-palette.results` serves that pool whether the palette is open or not, so a test that
// read the rows without opening saw the neighbours' pool. The test now opens the palette itself, reads only the COMMAND lines (the rows of the groups
// "suggested" and "commands"; "go-to" lines are places, not commands), and compares them with the Mac palette's own command list in English (the phone
// writes its titles through Words, the Mac's registry holds the English source). The phone's one "Settings" line is the documented exception
// (docs/lanes/palettephone.md: the phone lists one "Settings" line that opens iphone-settings; the Mac's lines are General settings, AI settings, MCP).
@MainActor @Test func everyPhoneCommandAlsoExistsInTheMacRegistry() throws {
    defer { closePalette() }
    try LanguagePin.with("en") {
        try openPalette()   // inside the pin: the pool writes its titles when it opens
        let commandRows = try rows("iphone-palette.results").filter { ($0["group"] as? String) != "go-to" && ($0["id"] as? String) != "settings" }
        let phone = Set(commandRows.compactMap { $0["title"] as? String })
        try #require(phone.contains("Archive"), "positive control: the probe reads the phone's command lines")
        let mac = Set(PaletteCatalog.commands().map(\.title))
        try #require(!mac.isEmpty, "the Mac palette registers no commands yet")
        #expect(phone.isSubset(of: mac), "phone-only commands: \(phone.subtracting(mac))")        // mutant: a phone-only command
        #expect(!phone.contains { $0.contains(" › ") }, "a label path is a place to go, not a command")
    }
}
@MainActor @Test func theResultRowsCanBePicked() throws {
    try openPalette()
    defer { closePalette() }
    #expect(Registrations.click("iphone-palette.results.1") != nil)                            // mutant: results are not tappable
}

// mob-palettephone (proof file checks/proofs/g2/00a15e42.json): the phone's command ids are exactly the documented list (docs/lanes/palettephone.md
// "Command list"; the same ids as PHONE_COMMANDS in checks/palettephone-check.py), so a Mac-only command added to the phone is named whatever its title says,
// and a documented command that goes missing is red too.
// mutant: PalettePhoneItems.swift gains a Spec(id: "notch", ...) line after the search Spec (the exact edit is in docs/lanes/gap-phone-b.md)
@MainActor @Test func thePhoneCommandIdsAreExactlyTheDocumentedList() throws {
    try openPalette()
    defer { closePalette() }
    let ids = Set(try rows("iphone-palette.results").compactMap { $0["id"] as? String }.filter { $0.hasPrefix("command:") })
    let documented: Set<String> = ["command:archive", "command:snooze", "command:label", "command:pin", "command:unread", "command:undo", "command:compose", "command:search"]
    #expect(ids == documented, "extra: \(ids.subtracting(documented)), missing: \(documented.subtracting(ids))")
}
