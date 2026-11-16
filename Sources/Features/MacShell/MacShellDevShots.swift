#if os(macOS)
import AppKit
import SwiftUI

/// The lane's own capture instrument while the root view does not host the shell yet: renders MacShellRoot
/// in a scratch window at each size and theme, writes `$WHOOSH_SOCKET_DIR/shots/macshell-<theme>-<w>x<h>.png`
/// plus the shell and sync rows as JSON, then closes the window. `click macshell.dev.shots` starts it;
/// the file `macshell-done.txt` appears when all four are written.
@MainActor
enum MacShellDevShots {
    private static let cells: [(String, NSAppearance.Name, CGFloat, CGFloat)] = [
        ("light", .aqua, MacShellGeometry.windowW, MacShellGeometry.windowH),
        ("dark", .darkAqua, MacShellGeometry.windowW, MacShellGeometry.windowH),
        ("light", .aqua, MacShellGeometry.windowMinW, MacShellGeometry.windowMinH),
        ("dark", .darkAqua, MacShellGeometry.windowMinW, MacShellGeometry.windowMinH),
    ]

    static func start() -> Bool {
        let dir = (ProcessInfo.processInfo.environment["WHOOSH_SOCKET_DIR"] ?? NSTemporaryDirectory()) + "/shots"
        try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
        run(0, dir: dir)
        return true
    }

    private static func run(_ i: Int, dir: String) {
        guard cells.indices.contains(i) else {
            try? "done".write(toFile: dir + "/macshell-done.txt", atomically: true, encoding: .utf8)
            return
        }
        let (theme, look, w, h) = cells[i]
        let win = NSWindow(contentRect: NSRect(x: 0, y: 0, width: w, height: h),
                           styleMask: [.titled, .closable, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
        win.isReleasedWhenClosed = false
        win.appearance = NSAppearance(named: look)
        win.contentViewController = NSHostingController(rootView: MacShellRoot(sizesWindow: false))
        win.setContentSize(NSSize(width: w, height: h))
        win.orderFront(nil)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.8) {
            MainActor.assumeIsolated {
                let name = "\(dir)/macshell-\(theme)-\(Int(w))x\(Int(h))"
                if let v = win.contentView, let rep = v.bitmapImageRepForCachingDisplay(in: v.bounds) {
                    v.cacheDisplay(in: v.bounds, to: rep)
                    try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: name + ".png"))
                }
                let m = AppModel.shared
                let rows: [String: Any] = ["shell": MacShellActions.shellRows(m), "sync": MacShellActions.syncRows(m),
                                           "content": ["w": win.contentView?.bounds.width ?? 0, "h": win.contentView?.bounds.height ?? 0]]
                if let data = try? JSONSerialization.data(withJSONObject: rows, options: [.prettyPrinted, .sortedKeys]) {
                    try? data.write(to: URL(fileURLWithPath: name + ".json"))
                }
                win.close()
                run(i + 1, dir: dir)
            }
        }
    }
}
#endif
