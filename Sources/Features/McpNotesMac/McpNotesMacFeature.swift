import SwiftUI

/// Registration for the mcpnotesmac lane: the MCP half of the mac-mcp-notes surface (the notes half waits for J8's note record).
public enum McpNotesMacFeature: FeatureModule {
    public static let id = "mcpnotesmac"
    public static let surfaces: [String] = ["mac-mcp-notes"]

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        let s = McpSession.shared
        return [
            // The core is the truth for the server and the grants (`fixture mcp-server off` acts on the core, not on this session):
            // each read of a row first believes the core again, so a change made outside this pane shows on the next read.
            "mac-mcp-notes.server": { _ in
                s.syncFromCore()
                var row: ControlRow = ["state": s.serverOn ? "on" : "off",
                                       "line": s.serverOn ? McpWords.serverOnTitle : McpWords.serverOffTitle]
                if !s.serverOn { row["start"] = "mac-mcp-notes.server.start" }
                return [row]
            },
            "mac-mcp-notes.apps": { _ in
                s.syncFromCore()
                return s.ordered.map { ["ax": "mac-mcp-notes.apps.\($0.slug)", "app": $0.name, "state": $0.allowed ? "allowed" : "not allowed",
                                 "icon": McpAppIcons.source($0.name), "allow": "mac-mcp-notes.allow.\($0.slug)", "revoke": "mac-mcp-notes.revoke.\($0.slug)"] }
            },
            "mac-mcp-notes.requests": { _ in
                s.subscribe()
                return s.requests.map { ["id": $0.id, "app": $0.app, "tool": $0.tool, "words": $0.words, "icon": McpAppIcons.source($0.app),
                                  "allow": "mac-mcp-notes.request.\($0.id).allow", "refuse": "mac-mcp-notes.request.\($0.id).refuse"] }
            },
            // The shared title-bar case (probes-r1 round 4) reads `<surface>.controls`: the controls drawn inside the window's content.
            "mac-mcp-notes.controls": { _ in [["ax": "mac-mcp-notes.close", "kind": "button"]] },
            "mac-mcp-notes.notice": { _ in s.failure.map { [["text": $0]] } ?? [] },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        let s = McpSession.shared
        var out: [String: (AppModel) -> Bool] = [
            "mac-mcp-notes.server.start": { _ in s.start() },
            "mac-mcp-notes.start": { _ in s.start() },
            // Test seam until the root serves `fixture mcp-server off|on` (the check drives that verb).
            "mac-mcp-notes.fixture.server-off": { _ in s.stopForFixture() },
        ]
        for r in s.requests {
            out["mac-mcp-notes.request.\(r.id).allow"] = { _ in s.answer(r.id, allowed: true) }
            out["mac-mcp-notes.request.\(r.id).refuse"] = { _ in s.answer(r.id, allowed: false) }
        }
        for name in Set(McpSession.Known.apps).union(s.apps.map(\.name)) {
            let slug = McpWords.slug(name)
            out["mac-mcp-notes.allow.\(slug)"] = { _ in s.set(name, allowed: true) }
            out["mac-mcp-notes.revoke.\(slug)"] = { _ in s.set(name, allowed: false) }
        }
#if os(macOS)
        // The palette lane wins "outside-ai-apps" by registry order once it lands; until then the pane opens from here.
        out["mac-palette.result.outside-ai-apps"] = { m in McpWindowDoor.open(m) }
        out["mac-mcp-notes.open"] = { m in McpWindowDoor.open(m) }
        out["mac-mcp-notes.window.default"] = { m in McpWindowDoor.resize(m, to: McpLayout.windowDefault) }
        out["mac-mcp-notes.window.min"] = { m in McpWindowDoor.resize(m, to: McpLayout.windowMin) }
        out["mac-mcp-notes.close"] = { m in McpWindowDoor.close(m) }
#endif
        return out
    }

    /// The shell's surface window draws the pane for its slot; the Settings window has no page for it yet (round 2 gap 1).
    public static func view(for slot: SurfaceSlot) -> AnyView? {
        slot == McpLayout.slot ? AnyView(McpPaneView()) : nil
    }

    /// Escape closes the pane while it holds the focus; any other surface passes the key on.
    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        #if os(macOS)
        ["Escape": { m in m.surface == McpLayout.slot.id && McpWindowDoor.close(m) }]
        #else
        [:]
        #endif
    }

    public static func paletteCommands() -> [(title: String, key: String)] { [(McpWords.paletteTitle, "")] }
}
