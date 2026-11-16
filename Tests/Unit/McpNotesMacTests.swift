import Foundation
import Testing
@testable import WhooshUI

// McpNotesMacTests: what the McpNotesMacFeature lane must register so the J5 checks (checks/*-check.py, journeys/J5.py) can drive it.
// Written from docs/JOURNEYS.md J5, docs/DECISION-MODEL.md v2 and docs/CONTROL-SOCKET.md, never from the built lane.
// The feature is a stub at this commit: every registration test below fails, in words naming the missing id.

/// The wire names the J5 checks drive (fixtures/tests-j5.json, "ax"), read from the repo, never from a built screen.
private func j5Ax() throws -> [String: String] {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let data = try Data(contentsOf: root.appendingPathComponent("fixtures/tests-j5.json"))
    let json = try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
    // fixtures/tests-j5.json "ax" also holds list-valued entries (search_answer_regions); the wire names are the string ones.
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

@MainActor @Test func McpNotesMacFeatureDrawsTheSurfaceTheJourneysArriveAt() {
    #expect(McpNotesMacFeature.surfaces.contains("mac-mcp-notes"))
}

@MainActor @Test func McpNotesMacFeatureRegistersEveryRegionTheChecksRead() throws {
    // swift-tests lane: the pane moved into the Settings window in round 2 (main54, commit 7d6c72b5) and the approval prompt became one row per waiting request,
    // `mac-mcp-notes.requests`; the results are no row any more but the core path `mcp_request/<id>/results` (checks/mcpnotesmac-check.py:9-12, Sources/Features/McpNotesMac/McpNotesMacFeature.swift:26-30).
    // fixtures/tests-j5.json still names the retired `mac-mcp-notes.prompt` and `.results`, so those two names are no longer asked of the feature.
    let missing = try missingRows(McpNotesMacFeature.self, ["mcp_server", "mcp_apps"])
    #expect(missing.isEmpty, "no row source registered for: \(missing)")
    #expect(McpNotesMacFeature.rowSources()["mac-mcp-notes.requests"] != nil, "no row source registered for mac-mcp-notes.requests (one row per waiting approval)")
}

@MainActor @Test func McpNotesMacFeatureRegistersEveryControlTheChecksClick() throws {
    // `mcp_prompt_approve` (mac-mcp-notes.prompt.approve) is retired: allow and refuse are per request, `mac-mcp-notes.request.<id>.allow|refuse`, and exist only while a request waits
    // (McpNotesMacFeature.swift:49-50; checks/mcpnotesmac-check.py drives them against a real request).
    let missing = try missingClicks(McpNotesMacFeature.self, ["mcp_start", "mcp_allow_claude", "mcp_revoke_claude"])
    #expect(missing.isEmpty, "no click target registered for: \(missing)")
}

@MainActor @Test func McpNotesMacFeatureOffersItsPaletteEntries() {
    // swift-tests lane: `Words.text` answers in the Mac's language (fr-BE here) when no choice is stored (Sources/Localization/Words.swift `current`), so a title is compared with `Words.text` of the same key, not with an English literal.
    let have = Set(McpNotesMacFeature.paletteCommands().map { $0.title })
    let missing = [Words.text("Outside AI apps (MCP)")].filter { !have.contains($0) }
    #expect(missing.isEmpty, "palette entries missing: \(missing)")
}
