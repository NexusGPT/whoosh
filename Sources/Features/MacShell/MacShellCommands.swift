import Foundation
#if os(macOS)
import AppKit
#endif

/// The window commands the Superhuman preset names and the engine leaves to this lane (Commands.swift stubs wired by "macshell"):
/// focus left and right, a new window, the text size steps, the message link, the account digits. Key, palette and toolbar all
/// dispatch through the same CommandRegistry, so one handler serves every door.
extension MacShellFeature: CommandProviding {
    public static func commands() -> [String: (AppModel) -> Bool] {
        let s = MacShellState.shared
        var c: [String: (AppModel) -> Bool] = [
            "focus-left": { _ in MacShellGeometry.mutant == "no-focus-keys" ? false : s.shiftFocus(-1) },
            "focus-right": { _ in s.shiftFocus(1) },
            "font-bigger": { _ in s.stepText(1) },
            "font-smaller": { _ in s.stepText(-1) },
            "font-reset": { _ in s.resetText() },
            "copy-page-link": { m in MacShellLinks.copyLink(of: m) },
            "popout-reply": MacShellLinks.shiftR,
        ]
        #if os(macOS)
        c["new-window"] = { _ in MacShellLinks.newWindow() }
        #endif
        for n in 1...9 { c["switch-account-\(n)"] = { m in MacShellLinks.switchAccount(m, n) } }
        return c
    }
}

@MainActor
enum MacShellLinks {
    static let scheme = "whoosh://message/"

    /// ⇧R has two meanings in Superhuman's v8 list (docs/reference/superhuman-shortcuts-v8.json: Filters "No Reply", entry 35, and
    /// Pop Out Compose "Pop Out Reply", entry 107): one key, one meaning per place, the way Enter splits on `threadIsOpen`. With a
    /// thread open, the quick reply slab up or the composer in front, this passes the key on to composermac's pop-out. From the plain
    /// list there is no reply to pop out, so the key is the No Reply filter (resolved, so the dispatch log and the trainer read it).
    static func shiftR(_ m: AppModel) -> Bool {
        guard m.surface == SurfaceSlot.macMain.id, m.overlay == nil, !ShortcutEngine.threadIsOpen(m), !QuickReplyModel.shared.isOpen else { return false }
        ShortcutEngine.current.resolved = "filter-no-reply"
        return CommandRegistry.handler("filter-no-reply")?(m) ?? false
    }

    /// ⌃/ : the focused message's link on the pasteboard. WHOOSH_PASTEBOARD names a private pasteboard (the rig), else the general one.
    static func copyLink(of m: AppModel) -> Bool {
        guard let id = m.shownMessageId else { return false }
        let link = scheme + id
        #if os(macOS)
        let pb = ProcessInfo.processInfo.environment["WHOOSH_PASTEBOARD"].map { NSPasteboard(name: NSPasteboard.Name($0)) } ?? NSPasteboard.general
        pb.clearContents()
        guard pb.setString(link, forType: .string) else { return false }
        #endif
        MacShellState.shared.noteCopied(link)
        return true
    }

    /// ⌃1 to ⌃9 : the list shows the Nth account (the sidebar's account filter); false past the last account.
    static func switchAccount(_ m: AppModel, _ n: Int) -> Bool {
        let accounts = m.core.accounts()
        guard accounts.indices.contains(n - 1) else { return false }
        m.accountFilter = accounts[n - 1].id
        m.focusIndex = 0
        m.refresh()
        return true
    }

    #if os(macOS)
    /// ⌘N : another main window of the same scene group.
    static func newWindow() -> Bool {
        guard let open = WindowBridge.openWindow else { return false }
        if MacShellGeometry.mutant != "no-new-window" { open(id: SurfaceSlot.macMain.id) }
        return true
    }

    /// Closes every main window but the first one opened (the ⌘N copies). Their rows carry the same ids as the first
    /// window's, so while they are open a frame read by id can answer from the wrong window (main39: J0.15 measured a
    /// 1280-wide copy's icons). Mutant keep-extra: nothing closes.
    static func closeOtherMainWindows() -> Bool {
        let mains = (NSApp as NSApplication?)?.windows.filter { $0.isVisible && ($0.identifier?.rawValue ?? "").hasPrefix(SurfaceSlot.macMain.id) } ?? []
        guard mains.count > 1 else { return false }
        if MacShellGeometry.mutant != "keep-extra" {
            let first = mains.min { $0.windowNumber < $1.windowNumber }
            for w in mains where w !== first { w.close() }
        }
        return true
    }

    static var mainWindowCount: Int {
        (NSApp as NSApplication?)?.windows.filter { $0.isVisible && ($0.identifier?.rawValue ?? "").hasPrefix(SurfaceSlot.macMain.id) }.count ?? 0
    }
    #endif
}
