#if os(macOS)
import AppKit
import SwiftUI

/// Opens the gallery in a window of its own and hides the shell's windows, so the socket's `shot` (which shoots the
/// first visible window) shows the shared controls. Reached only through the click targets `components.gallery.*`.
@MainActor
final class ComponentGalleryWindow {
    static let shared = ComponentGalleryWindow()
    private var window: NSWindow?

    @discardableResult
    func show(page: GalleryPage? = nil, dark: Bool? = nil) -> Bool {
        if let page { GalleryState.shared.page = page }
        if let dark { GalleryState.shared.dark = dark }
        let w = window ?? makeWindow()
        window = w
        w.appearance = NSAppearance(named: GalleryState.shared.dark ? .darkAqua : .aqua)
        for other in (NSApp as NSApplication?)?.windows ?? [] where other !== w { other.orderOut(nil) }
        w.makeKeyAndOrderFront(nil)
        w.contentView?.layoutSubtreeIfNeeded()
        w.displayIfNeeded()
        return true
    }

    private func makeWindow() -> NSWindow {
        let w = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1040, height: 680), styleMask: [.titled, .closable], backing: .buffered, defer: false)
        w.title = "Whoosh components"
        w.isReleasedWhenClosed = false
        w.contentView = NSHostingView(rootView: ComponentGalleryView())
        w.center()
        return w
    }
}
#endif
