import Foundation
import Testing
@testable import WhooshUI

// SettingsAIMacTests: what the SettingsAIMacFeature lane must register so the J5 checks (checks/*-check.py, journeys/J5.py) can drive it.
// Written from docs/JOURNEYS.md J5, docs/DECISION-MODEL.md v2 and docs/CONTROL-SOCKET.md, never from the built lane.
// The feature is a stub at this commit: every registration test below fails, in words naming the missing id.

/// The wire names the J5 checks drive (fixtures/tests-j5.json, "ax"), read from the repo, never from a built screen.
private func j5Ax() throws -> [String: String] {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let data = try Data(contentsOf: root.appendingPathComponent("fixtures/tests-j5.json"))
    let json = try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
    // fixtures/tests-j5.json "ax" also holds a list-valued entry (search_answer_regions, since 5712286a); the wire names are the string ones.
    // The old `as? [String: String]` cast failed on the whole dictionary, so every test that read a name died here (main76).
    let ax = try #require(json["ax"] as? [String: Any])
    return ax.compactMapValues { $0 as? String }
}

@MainActor private func missingRows<F: FeatureModule>(_ f: F.Type, _ names: [String]) throws -> [String] {
    let ax = try j5Ax(), have = Set(f.rowSources().keys)
    return names.compactMap { n in let id = ax[n]!; return have.contains { $0 == id || $0.hasPrefix(id + ".") } ? nil : id }
}
@MainActor private func missingClicks<F: FeatureModule>(_ f: F.Type, _ names: [String]) throws -> [String] {
    let ax = try j5Ax(), have = Set(f.clickTargets().keys)
    return names.compactMap { n in let id = ax[n]!; return have.contains(id) ? nil : id }
}

@MainActor @Test func SettingsAIMacFeatureDrawsTheSurfaceTheJourneysArriveAt() {
    #expect(SettingsAIMacFeature.surfaces.contains("mac-settings-ai"))
}

// Broken copy: delete the `"mac-settings-ai.decisions.result"` entry (or the `.consent` or `.rule-editor` one) from SettingsAIMacFeature.rowSources(); the test names it as missing.
// The decision read has not landed (DecisionCatalog.swift "INTERIM"): the three regions answer from what the pane holds (last preview run, consent record, the rule "Add a rule" opened).
@MainActor @Test func SettingsAIMacFeatureRegistersEveryRegionTheChecksRead() throws {
    let missing = try missingRows(SettingsAIMacFeature.self, ["providers", "speech", "agents_form", "agents_card", "dec_pages", "dec_engine", "dec_result", "dec_consent", "dec_rule_editor"])
    #expect(missing.isEmpty, "no row source registered for: \(missing)")
}

@MainActor @Test func SettingsAIMacFeatureRegistersEveryControlTheChecksClick() throws {
    let missing = try missingClicks(SettingsAIMacFeature.self, ["speech_local", "agents_new"])
    #expect(missing.isEmpty, "no click target registered for: \(missing)")
}

@MainActor @Test func SettingsAIMacFeatureOffersItsPaletteEntries() {
    // swift-tests lane: `Words.text` answers in the Mac's language (fr-BE here) when no choice is stored (Sources/Localization/Words.swift `current`), so a title is compared with `Words.text` of the same key, not with an English literal.
    let have = Set(SettingsAIMacFeature.paletteCommands().map { $0.title })
    let missing = [Words.text("AI settings"), Words.text("Decisions")].filter { !have.contains($0) }
    #expect(missing.isEmpty, "palette entries missing: \(missing)")
}

@MainActor @Test func SettingsAIMacClicksTheThreeEnginesAndEveryDecisionPage() throws {
    let have = Set(SettingsAIMacFeature.clickTargets().keys)
    let engines = ["local", "jev", "off"].map { "mac-settings-ai.decisions.engine.\($0)" }
    let pages = ["which-tab", "needs-reply", "sign-in-codes", "vips", "notify-me", "labels"]
    let previews = pages.map { "mac-settings-ai.decisions.page.\($0).preview" }
    let missing = (engines + previews).filter { !have.contains($0) }
    #expect(missing.isEmpty, "DECISION-MODEL.md v2: an Engine block (Local, Jev, Off) and a Preview on each of six pages; missing \(missing)")
}

@MainActor @Test func SettingsAIMacHasNoPageBuiltAroundTheMethod() throws {
    // Version 1 had a page "Rules" and a page "Decision model"; the owner rejected it on 2026-11-07 18:30.
    let regions = SettingsAIMacFeature.rowSources().keys
    let control = regions.contains { $0.hasPrefix("mac-settings-ai.decisions.page.") }  // positive control: the v2 pages are there
    let v1 = regions.filter { $0.contains("decision-model") || $0.hasSuffix(".rules") }
    #expect(control, "no v2 decision page is registered at all, so the absence of v1 pages below means nothing")
    #expect(v1.isEmpty, "version 1 regions still registered: \(v1)")
}
