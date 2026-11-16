import SwiftUI
#if os(macOS)
import AppKit
#endif

/// The socket's window verbs: frames by accessibility id, main-window resize, the app's appearance.
@MainActor
public enum WindowControl {
    /// The frame of the first element whose accessibility id is `ax`, in its window's coordinates
    /// (origin top-left, points), with the window's title. Nil when no visible window shows it.
    /// probes-r1 (round 4): one row per visible titled window for the verb `winbands`: its id, title, the title-bar band's real
    /// height (`frame.height - contentLayoutRect.height`), whether its content is full-size (drawn under the band), and the
    /// traffic lights' frames in the content view's coordinates (origin top-left), so a check can leave them out by their own size.
    public static func bands() -> [ControlRow] {
        #if os(macOS)
        return NSApp.windows.filter { $0.isVisible && $0.styleMask.contains(.titled) }.map { w in
            let h = w.contentView?.bounds.height ?? w.frame.height
            let lights: [ControlRow] = [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton].compactMap { t in
                guard let b = w.standardWindowButton(t), let sv = b.superview else { return nil }
                let r = sv.convert(b.frame, to: nil)
                return ["x": r.minX, "y": h - r.maxY, "w": r.width, "h": r.height]
            }
            return ["id": WindowBridge.id(of: w), "title": w.title, "band": w.frame.height - w.contentLayoutRect.height,
                    "fullsize": w.styleMask.contains(.fullSizeContentView), "lights": lights,
                    "title_shown": w.titleVisibility == .visible]   // title-hidden: the window draws its title text in the title band
        }
        #else
        return []
        #endif
    }

    public static func frame(of ax: String) -> ControlRow? {
        let published = FrameProbe.request(ax)
        #if os(macOS)   // the anchor's frame in its window's content view, read now (scroll included); `.global` is pane-relative in the shell.
        // Tried first and even when no `.global` frame was published for the id: `shot` reads contentFrame alone, so a view it can place is a view `drag` and `measure` can place
        // (hid-start: the welcome rows had an anchor and no published frame, and `drag` answered "no drawn frame").
        if let (c, w) = FrameProbe.contentFrame(ax) {
            // Inside a scrolled list neither the anchor view's frame nor SwiftUI's published `.global` frame follows the scroll (main100, probes-check
            // frame-follows-the-scroll: the list scrolled 52.5 pt and the served y stayed 152.0): the accessibility tree is built from what is drawn, so when it
            // carries the id and says another place, that place is the answer. Only on request, never per frame.
            if let a = accessibilityFrame(of: ax), abs(a.minX - c.minX) > 0.5 || abs(a.minY - c.minY) > 0.5 {
                return ["x": a.minX, "y": a.minY, "w": a.width, "h": a.height, "source": "accessibility", "window": a.window]
            }
            return ["x": c.minX, "y": c.minY, "w": c.width, "h": c.height, "source": "probe", "window": w.title]
        }
        #endif
        #if os(macOS)
        // No anchor in a window: the accessibility tree (built from what is drawn, scroll included) answers before SwiftUI's published `.global` frame, which stays
        // where layout put a lazy list's row (main103: served y 152.0 before and after a 52.5 pt scroll).
        if let a = accessibilityFrame(of: ax) { return ["x": a.minX, "y": a.minY, "w": a.width, "h": a.height, "source": "accessibility", "window": a.window] }
        #endif
        if let r = published {
            var row: ControlRow = ["x": r.minX, "y": r.minY, "w": r.width, "h": r.height, "source": "published"]
            #if os(macOS)
            // The published frame is the row's place BEFORE any scroll: the offset of the window's main scrolled list is served beside it (`scroll_y`, points from the
            // top, the number the `scroll` verb replies as `offset`), and the place as drawn is y - scroll_y. A caller that moves the pointer to the row subtracts it.
            if let w = WindowBridge.mainWindow(), let root = w.contentView { row["scroll_y"] = mainScrollOffset(in: root) }
            // ax ids are `<surface>.<region>...`: the surface names the window that draws it.
            let surface = SurfaceSlot(String(ax.prefix { $0 != "." }))
            if let w = WindowBridge.window(for: surface) ?? WindowBridge.mainWindow() { row["window"] = w.title }
            #endif
            return row
        }
        #if os(macOS)
        walked = 0
        for w in NSApp.windows where w.isVisible {
            guard let hit = find(ax, in: w, depth: 0) else { continue }
            let screen = hit.accessibilityFrame()
            let inWin = w.convertFromScreen(screen)
            let h = w.contentView?.bounds.height ?? w.frame.height
            return ["x": inWin.minX, "y": h - inWin.maxY, "w": inWin.width, "h": inWin.height, "window": w.title, "source": "accessibility"]
        }
        #endif
        return nil
    }

    #if os(macOS)
    /// The scroll offset (points from the top) of the largest scroll view in `root` that has more to show than fits, as the `scroll` verb reads it; 0 when none.
    @MainActor static func mainScrollOffset(in root: NSView) -> Double {
        let all = ScrollVerb.scrollViews(in: root).filter { $0.documentView != nil }
        let movable = all.filter { Double($0.documentView!.frame.height - $0.contentView.bounds.height) > 1 }
        guard let s = movable.max(by: { $0.contentView.bounds.width * $0.contentView.bounds.height < $1.contentView.bounds.width * $1.contentView.bounds.height }) else { return 0 }
        let doc = s.documentView!, clip = s.contentView
        let far = Double(doc.frame.height - clip.bounds.height), raw = Double(clip.bounds.origin.y - doc.frame.origin.y)
        return doc.isFlipped ? raw : far - raw
    }
    #endif

    #if os(macOS)
    /// The id's frame from the accessibility tree, in its window's content view (top-left origin), with the window's title; nil when no visible window carries it.
    private static func accessibilityFrame(of ax: String) -> (minX: CGFloat, minY: CGFloat, width: CGFloat, height: CGFloat, window: String)? {
        for w in NSApp.windows where w.isVisible {
            guard let hit = find(ax, in: w, depth: 0) else { continue }
            let inWin = w.convertFromScreen(hit.accessibilityFrame())
            let h = w.contentView?.bounds.height ?? w.frame.height
            return (inWin.minX, h - inWin.maxY, inWin.width, inWin.height, w.title)
        }
        return nil
    }
    #endif

    /// Elements visited by the last `frame(of:)`, reported when nothing matched.
    public static var walked = 0

    #if os(macOS)
    /// Depth bound: the accessibility tree is walked, never a cycle (a ceiling, not a design number).
    private static let maxDepth = 64

    private static func find(_ ax: String, in e: NSAccessibilityProtocol, depth: Int) -> NSAccessibilityProtocol? {
        if depth > maxDepth { return nil }
        walked += 1
        if e.accessibilityIdentifier() == ax { return e }
        for c in (e.accessibilityChildren() ?? []) {
            guard let child = c as? NSAccessibilityProtocol else { continue }
            if let hit = find(ax, in: child, depth: depth + 1) { return hit }
        }
        return nil
    }
    #endif

    #if os(macOS)
    static func name(_ a: NSAppearance) -> String { a.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? "dark" : "light" }
    #endif

    /// Sets the main window's content size; replies the size AppKit kept (its minimum may clamp it).
    public static func resizeMain(width: Double, height: Double) throws -> ControlRow {
        try resizeFront(width: width, height: height, surface: SurfaceSlot.macMain.id)
    }

    /// Sets the content size of the window that draws `surface` (the main window, the Settings window, a
    /// surface window); the shell's minimum clamps it only when the request is smaller. Replies the
    /// content and frame AppKit kept, and the window's title.
    public static func resizeFront(width: Double, height: Double, surface: String) throws -> ControlRow {
        #if os(macOS)
        guard width.isFinite, height.isFinite, width > 0, height > 0 else { throw ControlFailure("resize takes two positive numbers of points") }
        let slot = SurfaceSlot(surface)
        guard let w = WindowBridge.window(for: slot) ?? WindowBridge.mainWindow() else { throw ControlFailure("no window to resize for \(surface)") }
        // A scene may fix its window (the Settings window opened at about 900x450 with no way to grow): lift
        // that, so the requested size is the size asked for, not the scene's.
        w.styleMask.insert(.resizable)
        w.contentMaxSize = NSSize(width: 100_000, height: 100_000)
        w.setContentSize(NSSize(width: width, height: height))
        // The shell's own minimum may re-size it after layout: wait until the frame holds still, reply that.
        var last = w.frame
        SurfaceSettle.wait { defer { last = w.frame }; return w.frame == last && w.frame.size != NSSize(width: width, height: height) }
        let c = w.contentView?.bounds.size ?? .zero
        return ["content": ["w": c.width, "h": c.height], "frame": ["w": w.frame.width, "h": w.frame.height], "window": w.title]
        #else
        throw ControlFailure("resize is served on the Mac only")
        #endif
    }

    /// `light`, `dark`, or `system` (follow macOS). Returns the appearance now in effect.
    public static func setTheme(_ name: String) throws -> String {
        #if os(macOS)
        switch name {
        case "light": NSApp.appearance = NSAppearance(named: .aqua)
        case "dark": NSApp.appearance = NSAppearance(named: .darkAqua)
        case "system": NSApp.appearance = nil
        default: throw ControlFailure("theme takes light, dark or system")
        }
        // Each window takes the appearance too: a window keeps its resolved one until told (seen on the
        // rig: a dark main window shot identical after `theme light`).
        for w in NSApp.windows { w.appearance = NSApp.appearance; w.contentView?.needsDisplay = true }
        let want = theme
        SurfaceSettle.wait { NSApp.windows.filter(\.isVisible).allSatisfy { Self.name($0.effectiveAppearance) == want } }
        return theme
        #else
        throw ControlFailure("theme is served on the Mac only")
        #endif
    }

    /// The appearance the app draws in now, read from AppKit: "light" or "dark".
    public static var theme: String {
        #if os(macOS)
        return name(NSApp.effectiveAppearance)
        #else
        return "light"
        #endif
    }
}
