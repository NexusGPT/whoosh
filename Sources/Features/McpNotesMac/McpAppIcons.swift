import SwiftUI
#if os(macOS)
import AppKit
#endif

/// The real icon of an outside app: the installed app's own icon, found by its bundle id. An app that is not installed gets a
/// neutral glyph, never a letter square (owner, 2026-11-08: a logo is the real one or absent).
enum McpAppIcons {
    /// The bundle ids of the clients the core knows (the fixture core's four).
    static let bundleIds: [String: String] = [
        "Claude Desktop": "com.anthropic.claudefordesktop",
        "Raycast": "com.raycast.macos",
        "Cursor": "com.todesktop.230313mzl4w4u92",
        "Notion Calendar": "com.notion.calendar",
    ]

    /// "bundle" when the app is installed and its own icon is drawn, else "absent".
    static func source(_ app: String) -> String { path(app) == nil ? "absent" : "bundle" }

    static func path(_ app: String) -> String? {
        #if os(macOS)
        guard let id = bundleIds[app] else { return nil }
        return NSWorkspace.shared.urlForApplication(withBundleIdentifier: id)?.path
        #else
        return nil
        #endif
    }
}

struct McpAppIcon: View {
    let name: String
    var dim = false
    static let side = WhooshMetrics.actionButton - WhooshSpacing.s4

    var body: some View {
        Group {
            #if os(macOS)
            if let path = McpAppIcons.path(name) {
                Image(nsImage: NSWorkspace.shared.icon(forFile: path)).resizable().interpolation(.high)
            } else { neutral }
            #else
            neutral
            #endif
        }
        .frame(width: Self.side, height: Self.side)
        .opacity(dim ? 0.55 : 1)
        .accessibilityHidden(true)
    }

    private var neutral: some View {
        RoundedRectangle(cornerRadius: WhooshRadius.item, style: .continuous).fill(WhooshPalette.fill)
            .overlay(Image(systemName: "app.dashed").foregroundStyle(WhooshPalette.secondaryLabel))
    }
}
