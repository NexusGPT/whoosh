import SwiftUI
#if os(macOS)
import AppKit
#endif

/// The two sizes the pane is drawn and shot at: docs/design/geometry/mac-main.json (default 1280x820, min 960x600).
enum McpLayout {
    static let windowDefault = CGSize(width: 1280, height: 820)
    static let windowMin = CGSize(width: 960, height: 600)
    /// The slot id the shell's surface window draws (SurfaceWindow titles the window with it).
    static let slot = SurfaceSlot("mac-mcp-notes")
}

#if os(macOS)
/// The shell's surface window carries the slot id as its title; the pane draws its own heading, so the title bar goes quiet
/// and the content runs under it, as the pane was drawn in round 1.
struct McpWindowChrome: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView { Probe() }
    func updateNSView(_ view: NSView, context: Context) {}
    private final class Probe: NSView {
        override func viewDidMoveToWindow() {
            guard let w = window else { return }
            w.titleVisibility = .hidden
            w.titlebarAppearsTransparent = true
            w.styleMask.insert(.fullSizeContentView)
            w.contentMinSize = McpLayout.windowMin
            if !opened { opened = true; w.setContentSize(McpLayout.windowDefault); w.center() }
        }
        private var opened = false
    }
}

enum McpWindowDoor {
    /// Opens (or raises) the shell's window for the pane.
    @MainActor static func open(_ m: AppModel) -> Bool { WindowBridge.open(McpLayout.slot, in: m) }
    /// Sets the content size (the two sizes the pane is shot at; the shell's `resize` verb moves the main window only).
    @MainActor static func resize(_ m: AppModel, to size: CGSize) -> Bool {
        guard WindowBridge.open(McpLayout.slot, in: m), let w = WindowBridge.window(for: McpLayout.slot) else { return false }
        w.setContentSize(size); w.center()
        w.contentView?.layoutSubtreeIfNeeded()
        return true
    }
    /// Closes it and hands the surface back to the main window.
    @MainActor static func close(_ m: AppModel) -> Bool {
        guard let w = WindowBridge.window(for: McpLayout.slot) else { return false }
        w.close()
        m.surface = SurfaceSlot.macMain.id
        return true
    }
}
#endif
