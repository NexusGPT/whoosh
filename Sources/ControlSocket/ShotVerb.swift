import Foundation
#if os(macOS)
import AppKit
#endif

/// The `shot` and `capture shot` verbs: the PNG path, which path drew it, and the frame-to-pixel mapping.
@MainActor
extension ControlVerbs {
    /// `shot <name>` shoots the front window; `shot <surface>` (a registered surface id) first opens the
    /// window that draws it, so every registered surface can be captured by its id. Ax ids after the name come back
    /// as `frames: {id: {x, y, w, h} | null}`, read in the same main-thread turn as the pixels: a frame measured by a
    /// separate `measure` can belong to an earlier layout (a thread still settling moved a chip 139.5 pt between them).
    static func shot(_ name: String, m: AppModel, frames ids: [String] = []) throws -> [String: Any] {
        var r = try shotOnly(name, m: m, mount: ids)
        if !ids.isEmpty { r["frames"] = Dictionary(uniqueKeysWithValues: ids.map { ($0, frame($0)) }) }
        return r
    }

    private static func frame(_ id: String) -> Any {
        #if os(macOS)
        if let (c, _) = FrameProbe.contentFrame(id) { return ["x": c.minX, "y": c.minY, "w": c.width, "h": c.height] }
        #endif
        return NSNull()
    }

    /// The visible window with this identifier (or, failing that, this title).
    #if os(macOS)
    private static func openWindow(_ key: String) -> NSWindow? {
        let open = NSApp.windows.filter(\.isVisible)
        return open.first { WindowBridge.id(of: $0) == key || $0.identifier?.rawValue == key } ?? open.first { !$0.title.isEmpty && $0.title == key }
    }
    #endif

    /// The open windows, named for the sentence that refuses a shot.
    private static func openWindowList() -> String {
        #if os(macOS)
        let rows = NSApp.windows.filter { $0.isVisible && $0.styleMask.contains(.titled) }.map { "\(WindowBridge.id(of: $0))|\($0.title)" }
        return rows.isEmpty ? "none" : rows.joined(separator: ", ")
        #else
        return "none"
        #endif
    }

    private static func shotOnly(_ name: String, m: AppModel, mount rawIds: [String]) throws -> [String: Any] {
        // `shot <name> window:<id>` (probes-r1, round 6): photographs the OPEN window with that identifier (or title), moving nothing.
        let ids = rawIds.filter { !$0.hasPrefix("window:") }
        let asked = rawIds.first { $0.hasPrefix("window:") }.map { String($0.dropFirst("window:".count)) }
        guard name.range(of: #"^[A-Za-z0-9._-]+$"#, options: .regularExpression) != nil else { throw ControlFailure("a shot name may hold only A-Z a-z 0-9 . _ -") }
        let dir = (ProcessInfo.processInfo.environment[ControlServer.socketEnv] ?? NSTemporaryDirectory()) + "/shots"
        try FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
        let path = dir + "/" + name + ".png"
        #if os(macOS)
        if let asked {
            guard let w = openWindow(asked) else { throw ControlFailure("no open window has the id \(asked); open windows (id|title): \(openWindowList())") }
            ids.forEach { _ = FrameProbe.request($0) }
            return reply(try WindowBridge.shot(to: path, window: w), path: path)
        }
        #endif
        guard Registrations.surfaces.contains(name) else {
            #if os(macOS)
            // A name that reads as a surface or window id but is neither is refused: the front window would be another window's picture.
            if name.range(of: #"^mac-[a-z0-9]+(-[a-z0-9]+)*(\.[a-z0-9-]+)*$"#, options: .regularExpression) != nil {
                guard let w = openWindow(name) else {
                    throw ControlFailure("\(name) is not a surface and not an open window, so there is no picture to give; `shot <name> window:<id>` photographs an open window by its id; open windows (id|title): \(openWindowList())")
                }
                ids.forEach { _ = FrameProbe.request($0) }
                return reply(try WindowBridge.shot(to: path, window: w), path: path)
            }
            #endif
            ids.forEach { _ = FrameProbe.request($0) }   // the readers mount (and settle) before the render, never between
            return reply(try WindowBridge.shot(to: path), path: path)
        }
        let slot = SurfaceSlot(name)
        #if os(macOS)
        // A slot a Settings pane answers to but that has no view of its own (Replies: SettingsPane.slot) is never opened as a window of its
        // own: it moves the Settings window to that pane and photographs that window.
        if !slot.isSettingsPage, SettingsPane.pane(for: slot) != nil {
            m.settingsPage = slot; m.surface = slot.id
            guard let open = WindowBridge.openWindow else { throw ControlFailure("\(name) cannot open yet: no window has appeared") }
            open(id: WindowBridge.settingsWindowId)
            SurfaceSettle.wait { openWindow(WindowBridge.settingsWindowId) != nil || NSApp.windows.contains { $0.isVisible && ($0.identifier?.rawValue ?? "").hasPrefix(WindowBridge.settingsWindowId) } }
            guard let w = NSApp.windows.first(where: { $0.isVisible && ($0.identifier?.rawValue ?? "").hasPrefix(WindowBridge.settingsWindowId) }) else {
                throw ControlFailure("\(name) is drawn by the Settings window and it is not open")
            }
            ids.forEach { _ = FrameProbe.request($0) }
            var r = reply(try WindowBridge.shot(to: path, window: w), path: path)
            r["surface"] = name
            return r
        }
        // The search (layout L1, docs/surfaces/mac-search.md) has no window of its own: the main window's list column draws its field, dropdown and results
        // (SearchMacFeature.slot). Its shot opens it there and photographs the main window, never a window of its own with the stub page (main88: `mac-search.stub`).
        if slot == SearchMacFeature.slot {
            guard WindowBridge.open(.macMain, in: m), SearchMacModel.shared.open() else { throw ControlFailure("\(name) cannot open yet: no window has appeared") }
            SurfaceSettle.wait { WindowBridge.isShowing(.macMain) }
            ids.forEach { _ = FrameProbe.request($0) }
            var r = reply(try WindowBridge.shot(to: path, slot: .macMain), path: path)
            r["surface"] = name
            return r
        }
        #endif
        guard WindowBridge.open(slot, in: m) else { throw ControlFailure("\(name) cannot open yet: no window has appeared") }
        SurfaceSettle.wait { WindowBridge.isShowing(slot) }
        ids.forEach { _ = FrameProbe.request($0) }
        var r = reply(try WindowBridge.shot(to: path, slot: slot), path: path)
        r["surface"] = name
        return r
    }

    /// `mapping: {x, y, scale}`: a `measure` frame point (x, y) is the PNG pixel ((x + mapping.x) * scale,
    /// (y + mapping.y) * scale); x, y are where the content view's top-left sits in the shot, in points.
    static func reply(_ s: WindowBridge.ShotResult, path: String) -> [String: Any] {
        ["path": path, "source": s.source, "window": s.window, "title": s.title, "mapping": ["x": s.contentX, "y": s.contentY, "scale": s.scale]]
    }
}
