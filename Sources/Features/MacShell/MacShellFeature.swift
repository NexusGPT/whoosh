import Foundation

/// The macshell lane's registrations: the sync line and shell rows, the pane keys, the window clicks.
public enum MacShellFeature: FeatureModule {
    public static let id = "macshell"
    public static let surfaces: [String] = ["mac-main"]

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        ["mac-main.status": MacShellActions.statusRows, "mac-main.sync.line": { m in MacShellActions.syncRows(m).map { $0.merging(MacSyncLine.indexFields()) { a, _ in a } } }, "mac-main.shell": MacShellActions.shellRows,
         "mac-main.toolbar": MacToolbarRows.rows, "mac-main.hints": { _ in MacHintTable.rows() }, "mac-main.hints.bands": MacHintBands.rows,
         "mac-main.hints.glyphs": { _ in MacHintGlyphs.rows() },
         "mac-main.foot.hints": MacFootHints.rows,
         "mac-main.controls": MacShellControls.rows]
    }

    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        let s = MacShellState.shared
        return [
            "⌃Tab": { _ in s.moveFocus(1) },              // macOS: move focus to the next view
            "⌃Shift+Tab": { _ in s.moveFocus(-1) },
            "⌃⌘L": { _ in s.toggle(.list) },
            "⌘+": { _ in s.stepText(1) }, "⌘=": { _ in s.stepText(1) },
            "⌘−": { _ in s.stepText(-1) }, "⌘-": { _ in s.stepText(-1) },
            "⌘0": { _ in s.textScaleIndex = MacShellGeometry.textScales.firstIndex(of: 1) ?? 0; return true },
            "Escape": MacShellActions.escape,
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        let s = MacShellState.shared
        var t: [String: (AppModel) -> Bool] = [
            "mac-main.status": MacShellActions.openAccounts,
            "mac-main.status.link": MacShellActions.openAccounts,
            "mac-main.sidebar.toggle": { _ in s.toggle(.sidebar) },
            "mac-main.list.toggle": { _ in s.toggle(.list) },
        ]
        for p in MacShellState.Pane.allCases {
            t["mac-main.pane.\(p.rawValue)"] = { _ in
                guard s.visiblePanes.contains(p) else { return false }
                s.focusedPane = p
                s.focusByKey = false
                return true
            }
        }
        // the toolbar's controls: the same run a click on the button does (the hosting view is not reachable by ax-id)
        for i in MacToolbarItem.all + MacToolbarItem.more { t["mac-main.toolbar.\(i.command)"] = { m in MacToolbarRun.run(i, m) } }
        #if os(macOS)
        t["mac-main.message.window"] = MacSystemBridges.openMessageWindow
        t["macshell.window.close-others"] = { _ in MacShellLinks.closeOtherMainWindows() }
        t["macshell.dock.toggle"] = { _ in MacSystemBridges.toggleDock() }
        t["macshell.mail.make-default"] = { _ in MacSystemBridges.makeDefaultMailApp() }
        t["macshell.menubar.toggle"] = { m in
            if MacSystemBridges.menuBar.visible { MacSystemBridges.menuBar.hide() }
            else { MacSystemBridges.menuBar.show(unread: MacSystemBridges.totalUnread(m)) }
            return true
        }
        t["macshell.hint.hold"] = { _ in MacHintState.shared.show(); return true }       // the socket cannot hold ⌘: the same show the timer calls
        t["macshell.hint.release"] = { _ in MacHintState.shared.holdEnded(); return true }
        t["macshell.hint.hold-by-flags"] = { _ in MacHintMonitor.shared.simulateHold(); return true }                  // the real ⌘-alone flags route: timer and key monitor
        t["macshell.hint.release-by-flags"] = { _ in MacHintMonitor.shared.simulateRelease(); return true }            // the real ⌘-up flags route
        t["macshell.hint.window-resign"] = { _ in MacHintMonitor.shared.simulateWindowResign(); return true }          // what a window losing the key runs
        t["macshell.hint.event-without-cmd"] = { _ in MacHintMonitor.shared.simulateEventWithoutCommand(); return true } // a key-up with no ⌘ in its flags
        t["macshell.dev.shots"] = { _ in MacShellDevShots.start() }
        #endif
        return t
    }

    public static func paletteCommands() -> [(title: String, key: String)] {
        // the sidebar's key is the live preset's (⌘\\ in Superhuman's): never a chord typed here (the owner banned ⌃⌘S-style chords)
        [("Hide or show the sidebar", ShortcutEngine.shared.chord(for: "sidebar") ?? ""), ("Hide or show the message list", "⌃⌘L"),
         ("Next pane", "⌃Tab"), ("Bigger text", "⌘+"), ("Smaller text", "⌘−")]
    }
}
