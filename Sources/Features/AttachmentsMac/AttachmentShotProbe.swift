#if os(macOS)
import AppKit

/// Rig only: one picture of the main window WITH this process's floating windows over it (the Quick Look panel, the
/// right-click menu), which the socket's `shot` (one window) cannot show. Written to `$WHOOSH_SOCKET_DIR/shots/`.
/// Own windows need no Screen Recording permission; the call is looked up by name, as ShotLive does.
@MainActor
enum AttachmentShots {
    private typealias OneWindow = @convention(c) (CGRect, UInt32, UInt32, UInt32) -> Unmanaged<CGImage>?
    static var last: [String: String] = [:]

    static func capture(_ name: String) -> Bool {
        guard let dir = AttachmentPlaces.rigDir?.appendingPathComponent("shots", isDirectory: true),
              let main = WindowBridge.window(for: .macMain) ?? (NSApp as NSApplication?)?.mainWindow
                ?? (NSApp as NSApplication?)?.windows.filter({ $0.isVisible && !($0 is NSPanel) }).max(by: { $0.frame.width * $0.frame.height < $1.frame.width * $1.frame.height })
        else { return false }
        // Every on-screen window of this process that overlaps the main window: the main window, Quick Look, a menu.
        let info = (CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]]) ?? []
        let pid = Int(getpid())
        let mainBox = cgRect(main.frame)
        // the main window first, from its own frame (the on-screen list does not always carry it)
        var ids: [CGWindowID] = [CGWindowID(main.windowNumber)], rects: [CGRect] = [mainBox], box = mainBox
        let appWindows = Set(((NSApp as NSApplication?)?.windows ?? []).map(\.windowNumber))
        var seen: [String] = []
        let menuWindows = Set(((NSApp as NSApplication?)?.windows ?? []).filter { String(describing: type(of: $0)).contains("Menu") }.map(\.windowNumber))
        let quickLook = QuickLookController.shared.isOpen ? (QuickLookController.shared.panel?.windowNumber ?? -1) : -1
        for w in info where (w[kCGWindowOwnerPID as String] as? Int) == pid {
            guard let n = w[kCGWindowNumber as String] as? Int, let b = w[kCGWindowBounds as String] as? [String: CGFloat] else { continue }
            let r = CGRect(x: b["X"] ?? 0, y: b["Y"] ?? 0, width: b["Width"] ?? 0, height: b["Height"] ?? 0)
            // the main window itself and what this lane floats over it; never another window of the app
            // (the composer, the notch panel): the Quick Look panel by its own number, and menu-level windows
            let layer = w[kCGWindowLayer as String] as? Int ?? 0
            seen.append("\(n):\(layer):\(appWindows.contains(n) ? "app" : "sys"):\(menuWindows.contains(n) ? "menu" : "-")")
            // a menu draws in a window of a menu class (listed by the app or not); never the app's own content windows
            let isMenu = layer > 0 && (menuWindows.contains(n) || !appWindows.contains(n))
            guard n != main.windowNumber, n == quickLook || (isMenu && r.intersects(mainBox)) else { continue }
            ids.append(CGWindowID(n)); rects.append(r); box = box.union(r)
        }
        // One image per window (the call the shell's shot verb uses), drawn at its place on the screen, main first.
        guard let h = dlopen(nil, RTLD_NOW), let sym = dlsym(h, "CGWindowListCreateImage") else { return false }
        let one = unsafeBitCast(sym, to: OneWindow.self)
        let scale = main.backingScaleFactor
        guard let ctx = CGContext(data: nil, width: Int(box.width * scale), height: Int(box.height * scale), bitsPerComponent: 8,
                                  bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return false }
        var drawn = 0
        for (id, r) in zip(ids, rects) {
            // optionIncludingWindow (1 << 3); boundsIgnoreFraming (1 << 0) | bestResolution (1 << 3)
            guard let img = one(CGRect.null, 1 << 3, id, (1 << 0) | (1 << 3))?.takeRetainedValue() else { continue }
            let at = CGRect(x: (r.minX - box.minX) * scale, y: (box.maxY - r.maxY) * scale, width: r.width * scale, height: r.height * scale)
            ctx.draw(img, in: at)
            drawn += 1
        }
        guard let img = ctx.makeImage(), let png = NSBitmapImageRep(cgImage: img).representation(using: .png, properties: [:]) else { return false }
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let url = dir.appendingPathComponent("attachments-\(name).png")
        guard (try? png.write(to: url)) != nil else { return false }
        last[name] = url.path
        last["\(name).colours"] = "\(colours(img))"
        last["\(name).windows"] = "\(drawn)"
        last["\(name).seen"] = seen.joined(separator: " ")
        return true
    }

    /// Distinct colours on a grid of points across the picture: 1 means a blank (one-colour) shot.
    private static func colours(_ img: CGImage) -> Int {
        let rep = NSBitmapImageRep(cgImage: img)
        var seen = Set<String>()
        let steps = Int(WhooshSpacing.s16)
        for i in 0..<steps { for j in 0..<steps {
            let c = rep.colorAt(x: rep.pixelsWide * i / steps, y: rep.pixelsHigh * j / steps)
            seen.insert(c.map { "\($0.redComponent)\($0.greenComponent)\($0.blueComponent)" } ?? "")
        } }
        return seen.count
    }

    /// AppKit's screen frame (origin bottom-left) in the window server's space (origin top-left of the main screen).
    private static func cgRect(_ r: NSRect) -> CGRect {
        let top = NSScreen.screens.first?.frame.maxY ?? r.maxY
        return CGRect(x: r.minX, y: top - r.maxY, width: r.width, height: r.height)
    }
}
#endif
