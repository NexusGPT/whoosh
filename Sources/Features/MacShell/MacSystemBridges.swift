import SwiftUI
#if os(macOS)
import AppKit
#endif

/// The Mac-only parity features (par-macshell): the menu bar item with the unread count, the Dock icon
/// switch, the default mail app, and one window per message. Each reports its visible state as a row.
@MainActor
enum MacSystemBridges {
    #if os(macOS)
    static let menuBar = MacMenuBar()
    #endif
    private static var messageWindows: [String: AnyObject] = [:]

    /// All inboxes unread: the core's mailbox count, else the All view's own figure. Never a sum over the tabs: Primary, All and each pinned label
    /// hold the same mail, so the sum showed about twice the unread on the real core, which filled no "all inboxes" key (the lead's read, 14 Nov 2026).
    static func totalUnread(_ m: AppModel) -> Int {
        if let all = m.counts.mailboxes.first(where: { $0.key.lowercased() == "all inboxes" }) { return Int(all.value) }
        let allView = inboxViewId(view: .all)
        return Int(m.counts.tabs.first { $0.tabId == allView }?.unread ?? 0)
    }

    #if os(macOS)
    static var dockHidden: Bool { NSApp.activationPolicy() == .accessory }

    static func toggleDock() -> Bool {
        NSApp.setActivationPolicy(dockHidden ? .regular : .accessory)
    }

    static var mailHandler: String {
        guard let probe = URL(string: "mailto:someone@example.com"),
              let app = NSWorkspace.shared.urlForApplication(toOpen: probe) else { return "" }
        return Bundle(url: app)?.bundleIdentifier ?? app.lastPathComponent
    }

    static var isDefaultMailApp: Bool { mailHandler == Bundle.main.bundleIdentifier }

    /// Asks macOS to make this copy the mailto handler; macOS shows its own confirmation.
    static func makeDefaultMailApp() -> Bool {
        NSWorkspace.shared.setDefaultApplication(at: Bundle.main.bundleURL, toOpenURLsWithScheme: "mailto") { _ in }
        return true
    }

    /// Opens the focused message in its own window, titled with its subject (Apple Mail: double-click).
    static func openMessageWindow(_ m: AppModel) -> Bool {
        guard let row = m.shownRow else { return false }
        let id = row.messageId
        if let w = messageWindows[id] as? NSWindow { w.makeKeyAndOrderFront(nil); return true }
        let w = NSWindow(contentRect: NSRect(x: 0, y: 0, width: MacShellGeometry.listMaxW + MacShellGeometry.readerMinW,
                                             height: MacShellGeometry.windowMinH),
                         styleMask: [.titled, .closable, .resizable, .miniaturizable], backing: .buffered, defer: false)
        w.title = row.subject
        w.isReleasedWhenClosed = false
        let host = NSHostingController(rootView: MacReaderPlaceholder())
        host.sizingOptions = []
        w.contentViewController = host
        w.setContentSize(NSSize(width: MacShellGeometry.listMaxW + MacShellGeometry.readerMinW, height: MacShellGeometry.windowMinH))
        w.center()
        w.makeKeyAndOrderFront(nil)
        messageWindows[id] = w
        MacShellState.shared.messageWindows = Array(messageWindows.keys).sorted()
        return true
    }
    #endif
}

#if os(macOS)
/// The menu bar item: the envelope and the unread count; clicking it brings the main window forward.
@MainActor
final class MacMenuBar: NSObject {
    private var item: NSStatusItem?
    private(set) var unread = 0
    var visible: Bool { item != nil }
    var title: String { item?.button?.title ?? "" }

    func show(unread: Int) {
        self.unread = unread
        if item == nil {
            let i = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
            i.button?.image = NSImage(systemSymbolName: "envelope", accessibilityDescription: "Whoosh")
            i.button?.imagePosition = .imageLeading
            i.button?.target = self
            i.button?.action = #selector(bringForward)
            i.button?.setAccessibilityIdentifier("menubar.whoosh")
            item = i
        }
        item?.button?.title = unread > 0 ? " \(unread)" : ""
    }

    func hide() {
        if let i = item { NSStatusBar.system.removeStatusItem(i) }
        item = nil
    }

    @objc private func bringForward() {
        NSApp.activate()
        WindowBridge.bringMainToFront()
    }
}
#endif
