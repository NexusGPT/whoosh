import Foundation
import Observation

/// Hide and show (owner 2026-11-08 10:25, "apple style"). The shell owns the pane (MacShellState.sidebarCollapsed, the split item
/// that slides in TAB_MORPH_MS, within MOTION_MAX_MS); this type adds what the sidebar lane owns: the remembered state across
/// launches, the "sidebar" command, the key shown on the control, and the state the list's title band reads.
@MainActor
public enum SidebarVisibility {
    /// UserDefaults has no Spine settings door for layout state yet: a sidebar-owned default (docs/lanes/sidebar.md "## Round 6").
    static let defaultsKey = "whoosh.sidebar.hidden"
    /// The key the toggle shows until the shortcuts lane binds "sidebar" (docs/lanes/sidebar.md "## Round 6": a Whoosh extra).
    public static let fallbackChord = "⌘\\"
    private static var started = false

    /// True while the sidebar is hidden; the list lane reads this for its title band (picker chevron, show-sidebar row).
    public static var isHidden: Bool { MacShellState.shared.sidebarCollapsed }

    /// The key written on the control and in its tooltip: the active preset's chord for "sidebar", else ⌘\.
    public static var chord: String { ShortcutEngine.shared.chord(for: "sidebar") ?? fallbackChord }

    /// The toggle's tooltip, with its key.
    public static var tooltip: String { "\(Words.text(isHidden ? "Show sidebar" : "Hide sidebar"))  \(chord)" }

    /// One call from the first sidebar build: loads the remembered state before the split first lays out (the pane then
    /// starts in that state with no slide), and keeps the default current from every later change (toggle, key, divider drag).
    static func startOnce(defaults: UserDefaults = .standard) {
        guard !started else { return }
        started = true
        let remembered = defaults.bool(forKey: defaultsKey)
        if MacShellState.shared.sidebarCollapsed != remembered { MacShellState.shared.sidebarCollapsed = remembered }
        track()
    }

    private static func track() {
        withObservationTracking {
            _ = MacShellState.shared.sidebarCollapsed
        } onChange: {
            Task { @MainActor in
                UserDefaults.standard.set(MacShellState.shared.sidebarCollapsed, forKey: defaultsKey)
                track()
            }
        }
    }

    /// The control, the "sidebar" command and the socket click all land here.
    @discardableResult public static func toggle() -> Bool { MacShellState.shared.toggle(.sidebar) }

    /// Rows for `rows mac-main.sidebar.visibility`.
    static func row() -> ControlRow {
        var r: ControlRow = ["hidden": isHidden, "remembered": UserDefaults.standard.bool(forKey: defaultsKey), "chord": chord, "tooltip": tooltip,
         "slide_ms": WhooshConstants.TAB_MORPH_MS, "max_ms": WhooshConstants.MOTION_MAX_MS,
         "pane_w": Int(MacShellState.shared.paneWidths[.sidebar] ?? 0), "list_w": Int(MacShellState.shared.paneWidths[.list] ?? 0),
                                  "reader_w": Int(MacShellState.shared.paneWidths[.reader] ?? 0)]
        #if os(macOS)
        if let d = SidebarToggleHost.shared.centreOffset { r["lights_dy"] = d }
        if let w = WindowBridge.mainWindow()?.contentView?.bounds.width { r["window_w"] = Int(w) }
        #endif
        return r
    }
}
