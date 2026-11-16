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

@MainActor @Test func theFeaturesRegisterTheMoveActionWithAKeyColumn() {
    let cmds = FeatureRegistry.all.flatMap { $0.paletteCommands() }
    // swift-tests lane: `Words.text` answers in the Mac's language (fr-BE here) when no choice is stored (Sources/Localization/Words.swift `current`), so a title is compared with `Words.text` of the same key, not with an English literal.
    #expect(cmds.contains { $0.title == Words.text("Move to another account") })    // mutant: action missing
    // the command registry's own titles are the English action words in lower case ("archive": Sources/Features/Shortcuts/Commands.swift:31, `live("archive", "archive")`, main76 read
    // ["archive"]); the palette capitalises and translates them when it draws them (PaletteMacCatalog.swift:11-14, `display` then `Words.text`), so the registry is compared as lower-case English.
    #expect(cmds.contains { $0.title.lowercased() == "archive" && $0.key == "E" },
            "archive rows: \(cmds.filter { $0.title.lowercased() == "archive" }.map(\.key))")   // mutant: wrong key hint
}
@MainActor @Test func everyPaletteTitleIsUnique() {
    let titles = FeatureRegistry.all.flatMap { $0.paletteCommands() }.map(\.title)
    let repeated = Dictionary(grouping: titles, by: { $0 }).filter { $0.value.count > 1 }.keys.sorted()
    #expect(Set(titles).count == titles.count, "titles registered twice: \(repeated)")     // mutant: two features register "Archive"
    #expect(!titles.isEmpty)                       // positive control for the uniqueness check
}
@MainActor @Test func theShortcutHintMatchesTheBoundKey() {
    for c in FeatureRegistry.all.flatMap({ $0.paletteCommands() }) where !c.key.isEmpty {
        // swift-tests lane: a hold-to-talk hint ("Fn") is bound as the pair `Fn:down` / `Fn:up` (Sources/Features/Dictation/DictationFeature.swift:55-60; AISettingsStore.swift:243), never as the bare key.
        let bound = Registrations.key(AppModel.normalise(c.key)) != nil || Registrations.key(AppModel.normalise(c.key) + ":down") != nil
        #expect(bound, "\(c.title) shows \(c.key) but that key is unbound")   // mutant: hint for a dead key
    }
}
