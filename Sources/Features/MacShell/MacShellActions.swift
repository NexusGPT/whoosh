import Foundation
#if os(macOS)
import AppKit
#endif

/// What the shell's rows report and its clicks do; the registrations in MacShellFeature point here.
@MainActor
enum MacShellActions {
    static func syncRows(_ m: AppModel) -> [ControlRow] {
        let line = MacSyncLine.read(m)
        let sidebarW = MacShellState.shared.paneWidths[.sidebar] ?? MacShellGeometry.sidebarW
        let fit = MacSyncLine.fit(line.failing ?? line.summary, sidebarWidth: sidebarW)
        let sum = MacSyncLine.fit(line.summary, sidebarWidth: sidebarW + WhooshMetrics.warningDot + WhooshSpacing.s6)
        return [["ax": "mac-main.status.link", "text": line.failing ?? line.summary, "line": line.summary,
                 "link": line.failing == nil ? NSNull() : "mac-settings-accounts",
                 "account": line.failingAccountId ?? NSNull(), "text_w": fit.text, "box_w": fit.box,
                 "line_w": sum.text, "line_box_w": sum.box]]
    }

    /// `mac-main.status`: the one line the status shows, naming the failing account when there is one.
    static func statusRows(_ m: AppModel) -> [ControlRow] {
        let line = MacSyncLine.read(m)
        let tab = m.counts.tabs.first { $0.tabId == m.tab }
        return [["ax": "mac-main.status", "text": line.failing ?? line.summary, "unread": Int(tab?.unread ?? 0), "tab": m.tab,
                 "failing_account": line.failingAccountId ?? NSNull()]]
    }

    static func shellRows(_ m: AppModel) -> [ControlRow] {
        let s = MacShellState.shared
        var row: ControlRow = [
            "focused_pane": s.focusedPane.rawValue, "sidebar_collapsed": s.sidebarCollapsed,
            "list_collapsed": s.listCollapsed, "focus_by_key": s.focusByKey, "last_link": s.lastCopiedLink ?? NSNull(),
            "list_target_w": s.sidebarCollapsed ? Int(MacShellGeometry.listWideW) : Int(MacShellGeometry.listW), "text_scale": s.textScale, "message_windows": s.messageWindows,
        ]
        for p in MacShellState.Pane.allCases { row["\(p.rawValue)_w"] = s.paneWidths[p] ?? NSNull() }
        #if os(macOS)
        #if os(macOS)
        row["menubar_visible"] = MacSystemBridges.menuBar.visible
        row["menubar_title"] = MacSystemBridges.menuBar.title.trimmingCharacters(in: .whitespaces)
        #endif
        row["main_windows"] = MacShellLinks.mainWindowCount
        row["unread_total"] = MacSystemBridges.totalUnread(m)
        row["dock_hidden"] = MacSystemBridges.dockHidden
        row["default_mail_app"] = MacSystemBridges.mailHandler
        row["is_default_mail_app"] = MacSystemBridges.isDefaultMailApp
        #endif
        return [row]
    }

    /// The status line's link: Settings: Accounts comes forward (the settingsaccountsmac lane draws it).
    static func openAccounts(_ m: AppModel) -> Bool {
        guard MacSyncLine.read(m).failing != nil else { return false }
        // The Settings window is a Window scene opened by id (showSettingsWindow: has no Settings scene to answer).
        return WindowBridge.open(.macSettingsAccounts, in: m)
    }

    /// Esc: a later feature that owns an overlay (thread, palette, trainer) answers first; then the shell.
    static func escape(_ m: AppModel) -> Bool {
        let me = ObjectIdentifier(MacShellFeature.self)
        let later = FeatureRegistry.all.drop { ObjectIdentifier($0) != me }.dropFirst()
        for f in later { if let k = f.keyBindings()["Escape"], k(m) { return true } }
        return MacShellState.shared.escape(m)
    }
}
