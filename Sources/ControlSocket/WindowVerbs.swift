import Foundation
#if os(macOS)
import AppKit
#endif

/// The window verbs: winframes, measure (launch time, or a frame by accessibility id), labels (what each marked view's text
/// needs against its box), resize, theme.
@MainActor
enum WindowVerbs {
    static let all: [String: ControlVerbs.Handler] = [
        "winframes": { _, _ in ["frames": WindowBridge.frames()] },
        "winbands": { _, _ in ["bands": WindowControl.bands()] },   // probes-r1: each titled window's title-bar band, full-size flag and traffic lights
        "measure": { m, a in
            guard let ax = a.first else { throw ControlFailure("measure needs an ax-id") }
            if ax.hasPrefix("mac-main.interaction.") { return try InteractionProbe.answer(ax) }
            if let served = try PerfMeasures.answer(ax) { return served }   // app.memory, mac-search.first-screen
            if ax == "mac-main.launch", let ms = m.launchMs { return ["ms": ms] }
            if ax == "mac-main.launch.phases" { return ["phases": LaunchClock.phases()] }   // perf-archive round 2: the named marks of the launch, ms since process start
            if ax == "mac-main.first-frame" {
                guard let ms = m.firstFrameMs else { throw ControlFailure("not judged: no first frame has been presented yet") }
                return ["ms": ms]
            }
            guard let f = WindowControl.frame(of: ax) else { throw ControlFailure("no measurement at \(ax): nothing on screen carries that id (\(WindowControl.walked) accessibility elements walked)") }
            var reply: ControlRow = ["frame": f]
            if let r = FrameProbe.radius(ax) { reply["radius"] = Double(r) }   // a view marked `whooshAx(id, radius:)` says the radius it draws
            return reply
        },
        "labels": { _, a in
            // `labels [window:<id>]`: one row per `.whooshAx` view on screen in that window (default: the key window, else the
            // first visible one, as `shot` picks): {ax, text|null, need_w, need_h, box_w, box_h}, plus judged, marked, capped. Its own switch (FrameProbe.switchLabelsOn).
            guard a.count <= 1, a.allSatisfy({ $0.hasPrefix("window:") }) else { throw ControlFailure("usage: labels [window:<id>]") }
            #if os(macOS)
            let asked = a.first.map { String($0.dropFirst("window:".count)) }
            FrameProbe.switchLabelsOn()
            let open = NSApp.windows.filter(\.isVisible)
            let win: NSWindow? = if let asked { open.first { WindowBridge.id(of: $0) == asked || $0.identifier?.rawValue == asked } ?? open.first { !$0.title.isEmpty && $0.title == asked } }
                else { NSApp.keyWindow.flatMap { $0.isVisible ? $0 : nil } ?? open.first }
            guard let w = win else { throw ControlFailure(asked.map { "no open window has the id \($0)" } ?? "no visible window to read labels from") }
            var reply = FrameProbe.labels(in: w)
            reply["window"] = WindowBridge.id(of: w).isEmpty ? w.title : WindowBridge.id(of: w)
            return reply
            #else
            throw ControlFailure("labels needs the Mac app")
            #endif
        },
        "scroll": { _, a in
            // `scroll top|end|page-down|page-up [window:<id>]`: scrolls the window's main scroll view (the largest visible NSScrollView
            // that has more to show than fits) and replies {window, command, scrollable, offset, max, content_h, view_h, at}. See ScrollVerb.
            guard (1...2).contains(a.count), ScrollVerb.commands.contains(a[0]), a.dropFirst().allSatisfy({ $0.hasPrefix("window:") }) else {
                throw ControlFailure("usage: scroll top|end|page-down|page-up [window:<id>]")
            }
            #if os(macOS)
            let asked = a.dropFirst().first.map { String($0.dropFirst("window:".count)) }
            let open = NSApplication.shared.windows.filter(\.isVisible)
            let win: NSWindow? = if let asked { open.first { WindowBridge.id(of: $0) == asked || $0.identifier?.rawValue == asked } ?? open.first { !$0.title.isEmpty && $0.title == asked } }
                else { NSApplication.shared.keyWindow.flatMap { $0.isVisible ? $0 : nil } ?? open.first }
            guard let w = win, let root = w.contentView else { throw ControlFailure(asked.map { "no open window has the id \($0)" } ?? "no visible window to scroll") }
            var reply = try ScrollVerb.run(a[0], in: root)
            reply["window"] = WindowBridge.id(of: w).isEmpty ? w.title : WindowBridge.id(of: w)
            return reply
            #else
            throw ControlFailure("scroll needs the Mac app")
            #endif
        },
        "resize": { m, a in
            guard a.count == 2, let w = Double(a[0]), let h = Double(a[1]) else { throw ControlFailure("usage: resize <width> <height>") }
            return try WindowControl.resizeFront(width: w, height: h, surface: ProcessInfo.processInfo.environment["WHOOSH_RESIZE_MAIN"] == "1" ? SurfaceSlot.macMain.id : m.surface)
        },
        "activate": { _, _ in try PointerDrag.requestActivation() },
        "app-active": { _, a in
            guard a.count == 1, ["on", "off"].contains(a[0]) else { throw ControlFailure("usage: app-active on|off") }
            #if os(macOS)
            MacHintMonitor.shared.simulate(active: a[0] == "on")
            #endif
            return ["active": a[0] == "on", "simulated": true]
        },
        "window": { _, a in
            #if os(macOS)
            // title-hidden (owner's picture, 16 Nov, main195: the word "Whoosh" drawn over the sidebar button of the installed copy, never on the hidden copy):
            // `window title show` turns the main window's title text on, as the system or SwiftUI did there. The app must hide it again (MacSplitController).
            if a == ["title", "show"] {
                guard let w = WindowBridge.window(for: SurfaceSlot.macMain) else { throw ControlFailure("no main window to turn the title on") }
                w.titleVisibility = .visible
                return ["title_shown": w.titleVisibility == .visible]
            }
            #endif
            guard a.count == 2, a[0] == "fullscreen", ["on", "off"].contains(a[1]) else { throw ControlFailure("usage: window fullscreen on|off, or window title show") }
            return ["fullscreen": try WindowControl.fullScreen(a[1] == "on")]
        },
        "drag": { m, a in
        #if os(macOS)
        if a.count == 2, a[1] == AttachmentDesk.target {   // hid-compose: the rig's desktop stand-in, a folder of the run folder; no pointer moves
            guard let by = Registrations.drop(from: a[0], onto: a[1], m) else { throw ControlFailure("\(a[0]) dropped nothing on the desktop") }
            return ["drop": "desktop", "drop_by": by]
        }
        #endif
        guard a.count == 2 || (a.count == 3 && ["app", "view"].contains(a[2])) else { throw ControlFailure("usage: drag <from-ax-id> <to-ax-id> [app|view]") }
        return try PointerDrag.drag(from: a[0], to: a[1], path: a.count == 3 ? a[2] : nil)
    },
    "theme": { _, a in
            guard a.count == 1 else { throw ControlFailure("usage: theme light|dark|system") }
            return ["theme": try WindowControl.setTheme(a[0])]
        },
    ]
}

/// The `scroll` verb: the socket had no way to reach the end of a long page, so a hidden picture could only show its top.
/// `top` and `end` go to the ends, `page-down` and `page-up` move nine tenths of the visible height. The scroll view is the
/// largest visible NSScrollView under the window's content view that has more to show than fits (a SwiftUI ScrollView is one);
/// with none, the reply says `scrollable: false` and nothing moves. Offsets are in points from the top of the content, whatever
/// the document's flip.
enum ScrollVerb {
    static let commands = ["top", "end", "page-down", "page-up"]
    static let pageShare = 0.9
    static let slack = 0.5   // points: a document no taller than its view by more than this does not scroll

    /// The offset (points from the top) the command asks for, or nil for a word that is not a command.
    nonisolated static func target(_ command: String, offset: Double, content: Double, view: Double) -> Double? {
        let far = max(0, content - view)
        switch command {
        case "top": return 0
        case "end": return far
        case "page-down": return min(far, offset + view * pageShare)
        case "page-up": return max(0, offset - view * pageShare)
        default: return nil
        }
    }

    nonisolated static func at(_ offset: Double, far: Double) -> String {
        far <= slack ? "top" : offset <= slack ? "top" : offset >= far - slack ? "end" : "middle"
    }

    #if os(macOS)
    @MainActor static func scrollViews(in root: NSView) -> [NSScrollView] {
        var out: [NSScrollView] = []
        func walk(_ v: NSView) {
            if v.isHidden { return }
            if let s = v as? NSScrollView { out.append(s) }
            v.subviews.forEach(walk)
        }
        walk(root)
        return out
    }

    @MainActor static func run(_ command: String, in root: NSView) throws -> [String: Any] {
        let all = scrollViews(in: root).filter { $0.documentView != nil }
        func over(_ s: NSScrollView) -> Double { Double(s.documentView!.frame.height - s.contentView.bounds.height) }
        func area(_ s: NSScrollView) -> Double { Double(s.contentView.bounds.width * s.contentView.bounds.height) }
        let movable = all.filter { over($0) > slack }
        guard let s = movable.max(by: { area($0) < area($1) }) else {
            let one = all.max(by: { area($0) < area($1) })
            let h = Double(one?.contentView.bounds.height ?? 0)
            return ["command": command, "scrollable": false, "offset": 0.0, "max": 0.0, "content_h": Double(one?.documentView?.frame.height ?? 0), "view_h": h, "at": "top"]
        }
        let clip = s.contentView, doc = s.documentView!
        let content = Double(doc.frame.height), view = Double(clip.bounds.height), far = max(0, content - view)
        let flipped = doc.isFlipped
        let rawY = Double(clip.bounds.origin.y - doc.frame.origin.y)
        let before = flipped ? rawY : far - rawY
        guard let to = target(command, offset: before, content: content, view: view) else { throw ControlFailure("unknown scroll command \(command)") }
        let y = flipped ? to : far - to
        clip.scroll(to: NSPoint(x: clip.bounds.origin.x, y: CGFloat(y) + doc.frame.origin.y))
        s.reflectScrolledClipView(clip)
        let rawAfter = Double(clip.bounds.origin.y - doc.frame.origin.y)
        let now = flipped ? rawAfter : far - rawAfter
        return ["command": command, "scrollable": true, "offset": now, "max": far, "content_h": content, "view_h": view, "at": at(now, far: far)]
    }
    #endif
}

/// The two measures perf/thirty-one.py reads besides the interaction ones. Nothing runs until `measure` asks: no timer, no per-frame work.
///   app.memory               {"mb", "rss_mb"}: the app's physical footprint (what the ledger's memory rows judge) and its resident size, now.
///   mac-search.first-screen  {"ms", "event"}: from the last key (the Enter that launches a search) to the first presented frame after
///                            its commit, the quantity journeys/J2.py judges against SEARCH_FIRST_SCREEN_MS; it is the interaction `.frame`
///                            of that event, so a run that has delivered no key yet is not judged.
@MainActor
enum PerfMeasures {
    /// The interaction measure each id is read from; nil for an id this enum does not serve.
    static func probeId(for id: String) -> String? { id == "mac-search.first-screen" ? "mac-main.interaction.frame" : nil }

    static func answer(_ id: String) throws -> [String: Any]? {
        if id == "app.memory" { return try memory() }
        if let probe = probeId(for: id) { return try InteractionProbe.answer(probe) }
        return nil
    }

    static func memory() throws -> [String: Any] {
        guard let mb = Footprint.physicalMB(), mb > 0 else { throw ControlFailure("not judged: the app's memory could not be read") }
        var out: [String: Any] = ["mb": Double(mb)]
        if let rss = residentMB() { out["rss_mb"] = rss }
        return out
    }

    /// Resident size (task_vm_info.resident_size) in MB, nil when the kernel does not answer.
    static func residentMB() -> Double? {
        var info = task_vm_info_data_t()
        var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<natural_t>.size)
        let kr = withUnsafeMutablePointer(to: &info) {
            $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) { task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count) }
        }
        return kr == KERN_SUCCESS ? Double(info.resident_size) / 1_048_576 : nil
    }
}
