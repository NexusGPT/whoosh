import SwiftUI
#if os(macOS)
import AppKit
#endif

/// The few window facts the socket needs (winframes, shot, measure); AppKit on the Mac only.
@MainActor
public enum WindowBridge {
    public static let settingsWindowId = "mac-settings"
    public static let surfaceWindowId = "surface"

    /// The words a surface window's title bar shows: the screen's name from the catalogue, in the app's language. A screen
    /// with no name of its own in the catalogue shows no title text. Never the surface id.
    static let titleKeys: [String: String] = [
        "mac-composer": "New message", "mac-palette": "Command palette", "mac-search": "Search",
        "mac-quickreply": "Quick reply", "mac-trainer": "Whoosh trainer", "mac-mcp-notes": "Outside AI apps",
        "mac-add-account": "Add account", "mac-labels-contacts": "Labels",
    ]
    public static func title(for slot: SurfaceSlot) -> String { liveTitles[slot.id]?() ?? titleKeys[slot.id].map { Words.text($0) } ?? "" }

    /// A window whose title follows what it holds (the composer: New message, Reply, Reply all, Forward): its feature gives the words here, read
    /// each time the window's view is drawn. The owner's picture of a forward (lead, 13 Nov 2026, main155) had "New message" in the title bar
    /// over the heading "Forward".
    public static var liveTitles: [String: @MainActor () -> String] = [:]

    /// SwiftUI's window opener, captured by the first RootView (the socket has no environment).
    public static var openWindow: OpenWindowAction?

    public static func bringMainToFront() {
        #if os(macOS)
        window(for: .macMain)?.orderFrontRegardless()
        #endif
    }

    /// The main window comes forward AND takes the keyboard: a door in another window (Settings) opened something that lives in the main one. Without the
    /// second half the other window stays key and every key the owner presses goes there (the quick start shown again from Settings, main203).
    public static func giveMainTheKeyboard() {
        #if os(macOS)
        guard let w = window(for: .macMain) else { return }
        w.orderFrontRegardless()
        w.makeKey()
        #endif
    }

    public static func frames() -> [ControlRow] {
        #if os(macOS)
        return visibleWindows().map { w in
            ["title": w.title, "x": w.frame.origin.x, "y": w.frame.origin.y, "w": w.frame.width, "h": w.frame.height, "kind": kind(w),
             "id": id(of: w), "surface": surfaceId(of: w) ?? ""]
        }
        #else
        return []
        #endif
    }

    #if os(macOS)
    /// The visible windows of the application, or none when there is no application: `NSApp` is an implicitly unwrapped
    /// optional, nil in a process that never made an NSApplication (a `swift test` process), and reading `.windows`
    /// through it traps ("Unexpectedly found nil", WindowBridge.swift:43, round main76). Assigning it to an Optional first reads nil safely.
    static func visibleWindows() -> [NSWindow] {
        let app: NSApplication? = NSApp
        return app?.windows.filter(\.isVisible) ?? []
    }

    /// The surface each surface window draws, kept beside the window (weak: a closed window drops out). The window's title is
    /// for the user and says nothing about this.
    private static let surfaceOf = NSMapTable<NSWindow, NSString>(keyOptions: .weakMemory, valueOptions: .strongMemory)

    /// Marks `w` as the window of `surface`: its identifier becomes the surface id, so `winframes`, `winbands` and `shot ... window:<id>` name it.
    static func tag(_ w: NSWindow, surface: String) {
        surfaceOf.setObject(surface as NSString, forKey: w)
        if w.identifier?.rawValue != surface { w.identifier = NSUserInterfaceItemIdentifier(surface) }
    }

    /// The content height a surface's window opens with when the system's own opening size is shorter. Its feature gives the number.
    public static var openingHeights: [String: CGFloat] = [:]
    private static let sizedAtOpen = NSHashTable<NSWindow>.weakObjects()

    /// Once per window, after SwiftUI placed it: a window shorter than its surface's opening height grows downwards to it (the title bar stays
    /// where it is), no further than the screen's visible frame. A taller window is left as it is.
    static func sizeAtOpen(_ w: NSWindow, surface: String) {
        guard let want = openingHeights[surface], !sizedAtOpen.contains(w) else { return }
        sizedAtOpen.add(w)
        guard let have = w.contentView?.bounds.height, have < want else { return }
        var f = w.frame
        let grow = want - have
        f.size.height += grow
        f.origin.y -= grow
        if let visible = w.screen?.visibleFrame, f.minY < visible.minY {
            f.origin.y = visible.minY
            f.size.height = min(f.size.height, visible.height)
        }
        w.setFrame(f, display: true)
    }

    /// The surface a window was opened for (a surface window), else nil.
    static func surfaceId(of w: NSWindow) -> String? { surfaceOf.object(forKey: w) as String? }

    /// The one id a socket answer gives for a window, and the one `window:<id>` matches: the surface id of a surface window, else its
    /// identifier (the main window's `mac-main-AppWindow-1`, the Settings window's). SwiftUI names a new surface window
    /// `surface-AppWindow-1` before the app tags it, so reading `identifier` alone could answer a name `winframes` never lists.
    static func id(of w: NSWindow) -> String { windowId(surface: surfaceId(of: w), identifier: w.identifier?.rawValue) }
    #endif

    /// The pure rule behind `id(of:)` (no window needed, so a test can run it): the surface id when there is one, else the identifier, else "".
    nonisolated static func windowId(surface: String?, identifier: String?) -> String {
        if let surface, !surface.isEmpty { return surface }
        return identifier ?? ""
    }

    #if os(macOS)

    static func mainWindow() -> NSWindow? { window(for: .macMain) }

    /// What a visible window is, for `winframes`: the menu bar's status item, a panel (the notch), a sheet, or a window.
    static func kind(_ w: NSWindow) -> String {
        if NSStringFromClass(type(of: w)).contains("StatusBar") { return "status-item" }
        if w.isSheet { return "sheet" }
        return w is NSPanel ? "panel" : "window"
    }

    /// The visible window that draws a surface: the main window, the Settings window on a settings
    /// page, or the surface window tagged with the surface id (never found by its title).
    static func window(for slot: SurfaceSlot) -> NSWindow? {
        let visible = visibleWindows()
        func id(_ w: NSWindow) -> String { w.identifier?.rawValue ?? "" }
        if slot == .macMain { return visible.first { surfaceId(of: $0) == nil && id($0).hasPrefix(slot.id) } ?? visible.first { surfaceId(of: $0) == nil && !id($0).hasPrefix(settingsWindowId) && !id($0).hasPrefix(surfaceWindowId) } }
        if isSettingsSlot(slot) { return visible.first { surfaceId(of: $0) == nil && id($0).hasPrefix(settingsWindowId) } }
        return visible.first { surfaceId(of: $0) == slot.id }
    }
    #endif

    /// A slot the Settings window draws: one of its four pages, or a pane that answers to a slot of its own (Replies). Such a slot is
    /// never a screen window: it moves the one Settings window to its page.
    static func isSettingsSlot(_ slot: SurfaceSlot) -> Bool {
        #if os(macOS)
        return slot.isSettingsPage || SettingsPane.pane(for: slot) != nil
        #else
        return slot.isSettingsPage
        #endif
    }

    /// True when the window that draws `slot` is visible.
    static func isShowing(_ slot: SurfaceSlot) -> Bool {
        #if os(macOS)
        return window(for: slot) != nil
        #else
        return false
        #endif
    }

    /// Opens (or brings forward) the window that draws `slot`; false when there is no opener yet.
    static func open(_ slot: SurfaceSlot, in m: AppModel) -> Bool {
        if isSettingsSlot(slot) { m.settingsPage = slot }
        m.surface = slot.id
        if slot == .macMain { bringMainToFront(); return true }
        guard let open = openWindow else { return false }
        if isSettingsSlot(slot) { open(id: settingsWindowId) } else { open(id: surfaceWindowId, value: slot.id) }
        return true
    }

}
