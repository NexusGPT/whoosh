import Foundation
#if os(macOS)
import AppKit
#endif

/// `drag <from-ax-id> <to-ax-id>`: a pointer drag inside the app, from the first element's centre to the
/// second's: mouse down, moves in steps, mouse up. Each event is handed to THIS app's own dispatch, never the
/// system's, so the owner's cursor never moves (rig-hygiene.md rule 1).
@MainActor
public enum PointerDrag {
    /// Moves between down and up (docs/CONSTANTS.md DRAG_STEPS).
    static let steps = WhooshConstants.DRAG_STEPS

    /// `path`: `app` hands each event to NSApp.sendEvent (local monitors see it: `received` counts them); `view` hands it
    /// to the window's own sendEvent (`received` counts what was handed). Default: `app` on the hidden rig, `view` otherwise.
    public static func drag(from a: String, to b: String, path: String? = nil) throws -> ControlRow {
        #if os(macOS)
        guard let (w, p) = centre(of: a) else { throw ControlFailure("nothing at \(a)") }
        guard let (w2, q) = centre(of: b) else { throw ControlFailure("nothing at \(b)") }
        guard w == w2 else { throw ControlFailure("\(a) and \(b) are in different windows") }
        // Hidden rig: the app never comes forward (HiddenRig); a synthetic click acts as on an active app there.
        let viaApp = (path ?? (HiddenRig.isOn ? "app" : "view")) == "app"
        let front = HiddenRig.isOn ? nil : Self.activate(w)
        defer { front?.activate() }
        if HiddenRig.isOn { HiddenRig.pretendActive = true; w.makeKey() }   // inside the app only; never the front app
        defer { HiddenRig.pretendActive = false }
        let monitor = NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .leftMouseDragged, .leftMouseUp]) { e in received += 1; return e }
        defer { if let monitor { NSEvent.removeMonitor(monitor) } }
        if !HiddenRig.isOn && !(NSApp.isActive && w.isKeyWindow) {
            // Activation by request failed: a click in the window activates the app the way a person's does (the window
            // swallows it as an activation click, seen on build shell-r7-11), then the real drag runs on an active app.
            eventNumber += 1
            post(.leftMouseDown, at: p, in: w, viaApp: false)
            post(.leftMouseUp, at: p, in: w, viaApp: false)
            pump { NSApp.isActive && w.isKeyWindow }
            pump(for: 0.2) { false }   // the activated window lays out before the drag (a short settle, not a design number)
        }
        received = 0   // the priming click is not part of the drag
        eventNumber += 1   // one number for the whole press-drag-release, as the window server gives it
        // Queued events (postEvent, then nextEvent + sendEvent) never reached an inactive window (prototype,
        // _captures/appshell/r3/hidden-mechanics.md); each event is handed to a dispatch directly.
        // Lead, 13 Nov (owner A162, walk h119): a click INSIDE A TEXT (a field's editor, a text view) is tracked by the text itself, and its
        // loop takes the release FROM THE EVENT QUEUE (-[NSTextView mouseDown:] waits in nextEventMatchingMask). A release handed to
        // sendEvent after the press never reached that loop: the press never returned and the test copy stopped answering (a click on the
        // search field with results listed). For a click on a text the release is queued first, so the loop finds it; a real mouse was never
        // concerned (its release comes through the queue).
        let onText = a == b && pressLandsOnText(w, p)
        if onText, let up = event(.leftMouseUp, at: q, in: w) { NSApp.postEvent(up, atStart: false) }
        post(.leftMouseDown, at: p, in: w, viaApp: viaApp)
        if !onText {
            for i in 1...steps {
                let t = CGFloat(i) / CGFloat(steps)
                post(.leftMouseDragged, at: CGPoint(x: p.x + (q.x - p.x) * t, y: p.y + (q.y - p.y) * t), in: w, viaApp: viaApp)
            }
            post(.leftMouseUp, at: q, in: w, viaApp: viaApp)
        }
        // A SwiftUI drag commits only on an activated app (3b), refused when hidden: the screen's own drop function ends it.
        RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05))
        let dropBy = Registrations.drop(from: a, onto: b, AppModel.shared)
        return ["from": ["x": p.x, "y": p.y], "to": ["x": q.x, "y": q.y], "window": w.title, "delivered": steps + 2, "received": received,
                "active": NSApp.isActive, "key": w.isKeyWindow, "path": viaApp ? "app" : "view", "hidden": HiddenRig.isOn, "drop": dropBy == nil ? NSNull() : "screen", "drop_by": dropBy ?? NSNull()]
        #else
        throw ControlFailure("drag is served on the Mac only")
        #endif
    }

    /// `activate`: asks for the app to come forward and returns at once. The window server's answer lands only after the
    /// verb returns to the main run loop (shell-r7-15: a drag that asked in its own call still ran inactive), so a
    /// check calls this, then a verb, then `drag`. The app that was in front gets the focus back after the next drag.
    public static func requestActivation() throws -> ControlRow {
        #if os(macOS)
        guard !HiddenRig.isOn else { return ["active": NSApp.isActive, "hidden": true] }   // the hidden rig never comes forward
        if !NSApp.isActive { askedFrom = NSWorkspace.shared.frontmostApplication.flatMap { $0.processIdentifier == getpid() ? nil : $0 } }
        NSApp.activate(ignoringOtherApps: true)
        WindowBridge.mainWindow()?.makeKeyAndOrderFront(nil)
        return ["active": NSApp.isActive]
        #else
        throw ControlFailure("activate is served on the Mac only")
        #endif
    }

    #if os(macOS)
    private static var received = 0
    private static var eventNumber = 0
    private static var askedFrom: NSRunningApplication?

    /// A window of an inactive app swallows the first mouse-down as an activation click, and the posted
    /// events never reach the row. Make the app active and the window key for the drag; returns the app that
    /// was in front so the caller can give it back. `WHOOSH_DRAG_NOACTIVATE=1` is the mutant: the old path.
    private static func activate(_ w: NSWindow) -> NSRunningApplication? {
        guard ProcessInfo.processInfo.environment["WHOOSH_DRAG_NOACTIVATE"] != "1" else { w.makeKeyAndOrderFront(nil); return nil }
        let prev = askedFrom ?? NSWorkspace.shared.frontmostApplication.flatMap { $0.processIdentifier == getpid() ? nil : $0 }
        askedFrom = nil
        NSApp.activate(ignoringOtherApps: true)
        w.makeKeyAndOrderFront(nil)
        // NSApp.activate is refused on the rig (also through Launch Services' `open`, builds shell-r7-4 to 9); the caller
        // primes the app with an activation click instead of waiting for it here.
        return prev
    }

    /// Waits (bounded by SURFACE_OPEN_WAIT) for `done`, dispatching the app's queued AppKit events as it goes: the
    /// activation the window server sends arrives as an NSEvent that only `NSApp.sendEvent` acts on, and a bare
    /// RunLoop turn leaves it queued (seen on build shell-r7-6: the first drag ran inactive, the second active).
    private static func pump(for seconds: Double = SurfaceSettle.surfaceOpenWaitS, until done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done() && Date() < deadline {
            while let ev = NSApp.nextEvent(matching: .any, until: Date().addingTimeInterval(0.02), inMode: .default, dequeue: true) { NSApp.sendEvent(ev) }
        }
    }

    /// Centre in AppKit window coordinates (origin bottom-left), and the window that shows the element.
    private static func centre(of ax: String) -> (NSWindow, CGPoint)? {
        guard let f = WindowControl.frame(of: ax), let x = f["x"] as? CGFloat, let y = f["y"] as? CGFloat,
              let fw = f["w"] as? CGFloat, let fh = f["h"] as? CGFloat else { return nil }
        let title = f["window"] as? String
        let visible = NSApp.windows.filter(\.isVisible)
        guard let w = visible.first(where: { title != nil && $0.title == title }) ?? NSApp.keyWindow ?? WindowBridge.mainWindow(),
              let h = w.contentView?.bounds.height else { return nil }
        return (w, CGPoint(x: x + fw / 2, y: h - (y + fh / 2)))
    }

    private static func event(_ type: NSEvent.EventType, at p: CGPoint, in w: NSWindow) -> NSEvent? {
        NSEvent.mouseEvent(with: type, location: p, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                           windowNumber: w.windowNumber, context: nil, eventNumber: eventNumber, clickCount: 1, pressure: type == .leftMouseUp ? 0 : 1)
    }

    private static func post(_ type: NSEvent.EventType, at p: CGPoint, in w: NSWindow, viaApp: Bool) {
        guard ProcessInfo.processInfo.environment["WHOOSH_DRAG_NOSEND"] != "1" else { return }   // the mutant: drag sends nothing
        guard let e = event(type, at: p, in: w) else { return }
        if viaApp { NSApp.sendEvent(e) } else { received += 1; w.sendEvent(e) }
    }

    /// The view under the press is a text field, a field's editor or a text view (each tracks its own mouse from the event queue).
    private static func pressLandsOnText(_ w: NSWindow, _ p: CGPoint) -> Bool {
        guard ProcessInfo.processInfo.environment["WHOOSH_DRAG_NOSEND"] != "1", let root = w.contentView?.superview ?? w.contentView else { return false }
        var v = root.hitTest(root.convert(p, from: nil))
        while let x = v {
            if x is NSTextView || x is NSTextField { return true }
            v = x.superview
        }
        return false
    }
    #endif
}
