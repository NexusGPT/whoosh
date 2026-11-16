#if os(macOS)
import AppKit

/// The rig's HIDDEN mode (owner 2026-11-08 16:30: "on all tests, EVER, i want you to stop the preview opening EVER").
/// `scripts/launch-one.sh` sets `WHOOSH_RIG_HIDDEN=1` for every run (there is no visible mode); `--rig-hidden` does the
/// same. Then, from the app's init (accessory policy first), for the whole life of
/// the process, no window of this app reaches the owner's screen and the app never takes the keyboard:
///   - every window is drawn at alpha 0, click-through, and refuses to activate the app, from the first time it is
///     ordered in (orderFront, orderFrontRegardless, makeKeyAndOrderFront, orderWindow, a child window), and again on
///     every pass of the event loop as a safety net (a window shown by a path these hooks do not see);
///   - the app never activates (NSApp.activate, activateIgnoringOtherApps, NSRunningApplication.activate on itself);
///     an activation policy that would show a Dock tile is refused; a status item is created invisible;
///   - a synthetic click acts as on an active app (acceptsFirstMouse), so `click`, `drag` and the drives keep working.
/// Windows stay at their real place and size: measured on this Mac (prototype, _captures/appshell/r3/hidden-mechanics.md),
/// a window parked off every screen is occluded (0 display-link ticks in 0.5 s, SwiftUI content not drawn), so its
/// geometry, its frames and its pixels would no longer be the app's. Alpha 0 keeps the window rendering (61 ticks,
/// occlusion visible) and shows nothing. `shot` renders the window's view tree in this mode (WindowBridge.shot).
@MainActor
public enum HiddenRig {
    public static let isOn: Bool = ProcessInfo.processInfo.environment["WHOOSH_RIG_HIDDEN"] == "1"
        || CommandLine.arguments.contains("--rig-hidden")
    /// The proof's mutant: `WHOOSH_RIG_HIDDEN_MUTANT=alpha-1-255` gives every window alpha 1/255 instead of 0. The
    /// window server draws nothing at that alpha, yet lists it as 0.0039: the proof (alpha == 0 exactly) must go red.
    static let mutant = ProcessInfo.processInfo.environment["WHOOSH_RIG_HIDDEN_MUTANT"] ?? ""
    private static var installed = false
    /// Calls the hooks refused (activation, a Dock tile), for `uistate.hidden`.
    public private(set) static var refused: [String] = []
    /// True only while `drag` runs: AppKit and SwiftUI treat the app as active and its window as key for the gesture
    /// (a drag on the labels screen did not commit on an inactive window: as3d, both dispatch paths). The window server
    /// is not asked: the app never becomes the front app (checks/rig-hidden-check.py watches it during the drag).
    static var pretendActive = false

    /// Installs the hooks; call before the first window exists (the app's init).
    public static func install() {
        LaunchClock.once("app-init")   // launch-r3: the App's init calls this first, in the owner's run too (one flag read when tracing is off)
        defer { LaunchClock.mark("app-init-end") }   // perf-archive r3: launch phases inside app-init to corehost-begin (one clock read each)
        guard isOn else { if RigNet.onRig { installWebRefusals() }; return }   // a test copy shown on screen: still no web page off this Mac
        guard !installed else { return }
        installed = true
        RigCrashReason.install()   // hid-search round 18: an exception AppKit ends the app on names itself in the run folder (crash-reason.txt)
        let app = NSApplication.shared   // AppKit's own start (launch-r1 phase 3): the owner's copy pays it too, inside NSApplicationMain
        LaunchClock.mark("nsapp-made")   // perf-archive r4: the split point between AppKit's start and the hidden copy's own set-up
        _ = app.setActivationPolicy(.accessory)   // no Dock tile, no ⌘-Tab entry, before anything shows
        LaunchClock.mark("rig-nsapp-end")   // the hidden copy's own set-up from here to app-init-end (the owner's copy never runs it)
        if mutant != "no-system-ui" { installSystemUI() }   // panels, menus, alerts, pickers, apps, notifications
        LaunchClock.mark("rig-system-ui-end")
        let w = NSWindow.self
        swap(w, #selector(NSWindow.orderFront(_:)), #selector(NSWindow.whooshHidden_orderFront(_:)))
        swap(w, NSSelectorFromString("setIsVisible:"), #selector(NSWindow.whooshHidden_setIsVisible(_:)))
        swap(w, #selector(NSWindow.orderFrontRegardless), #selector(NSWindow.whooshHidden_orderFrontRegardless))
        swap(w, #selector(NSWindow.makeKeyAndOrderFront(_:)), #selector(NSWindow.whooshHidden_makeKeyAndOrderFront(_:)))
        swap(w, #selector(NSWindow.order(_:relativeTo:)), #selector(NSWindow.whooshHidden_order(_:relativeTo:)))
        swap(w, #selector(NSWindow.addChildWindow(_:ordered:)), #selector(NSWindow.whooshHidden_addChildWindow(_:ordered:)))
        swap(w, #selector(setter: NSWindow.alphaValue), #selector(NSWindow.whooshHidden_setAlphaValue(_:)))
        swap(w, #selector(setter: NSWindow.ignoresMouseEvents), #selector(NSWindow.whooshHidden_setIgnoresMouseEvents(_:)))
        let a = NSApplication.self
        swap(a, #selector(NSApplication.activate(ignoringOtherApps:)), #selector(NSApplication.whooshHidden_activate(ignoringOtherApps:)))
        swap(a, #selector(NSApplication.activate as (NSApplication) -> () -> Void), #selector(NSApplication.whooshHidden_activateNow))
        swap(a, #selector(getter: NSApplication.isActive), #selector(NSApplication.whooshHidden_isActive))
        swap(a, #selector(NSApplication.setActivationPolicy(_:)), #selector(NSApplication.whooshHidden_setActivationPolicy(_:)))
        swap(NSRunningApplication.self, #selector(NSRunningApplication.activate(options:)), #selector(NSRunningApplication.whooshHidden_activate(options:)))
        swap(NSView.self, #selector(NSView.acceptsFirstMouse(for:)), #selector(NSView.whooshHidden_acceptsFirstMouse(for:)))
        swap(NSStatusBar.self, #selector(NSStatusBar.statusItem(withLength:)), #selector(NSStatusBar.whooshHidden_statusItem(withLength:)))
        NotificationCenter.default.addObserver(forName: NSApplication.didUpdateNotification, object: nil, queue: .main) { _ in
            MainActor.assumeIsolated {
                for w in NSApp.windows where w.alphaValue != alpha(w) { conceal(w) }
                placeOnScreen()
            }
        }
    }

    /// Alpha 0, click-through, never activates the app (alpha 1/255 for the main window under the mutant).
    static func conceal(_ w: NSWindow) {
        w.whooshHidden_setAlphaValue(alpha(w))
        w.whooshHidden_setIgnoresMouseEvents(true)
        preventActivation(w)
    }

    /// 0, or 1/255 under the mutant.
    static func alpha(_ w: NSWindow) -> CGFloat {
        mutant == "alpha-1-255" ? 1.0 / 255.0 : 0   // every window: the SwiftUI ones never reach the screen (as3f)
    }

    static func refuse(_ what: String) { refused.append(what) }

    /// A background-launched app's SwiftUI windows are marked visible yet held OFF the screen by the window server
    /// (as3e: kCGWindowIsOnscreen false for the main and labels windows), and there a drag never commits. Each visible
    /// window is ordered in once, at alpha 0, the way a plain window is (the prototype's drags committed there).
    private static var placed = Set<ObjectIdentifier>()
    private static func placeOnScreen() {
        for w in NSApp.windows where w.isVisible && !placed.contains(ObjectIdentifier(w)) && !NSStringFromClass(type(of: w)).contains("StatusBar") {
            placed.insert(ObjectIdentifier(w))
            w.orderFrontRegardless()   // hooked: concealed first
        }
    }

    /// AppKit's own switch for a window whose clicks must not activate the app (a synthetic mouse-down through
    /// NSWindow.sendEvent activated the app in the prototype, the owner's keyboard with it; with this set it did not).
    private static func preventActivation(_ w: NSWindow) {
        let sel = NSSelectorFromString("_setPreventsActivation:")
        guard let m = class_getInstanceMethod(type(of: w), sel) else { return }
        typealias SetBool = @convention(c) (AnyObject, Selector, Bool) -> Void
        unsafeBitCast(method_getImplementation(m), to: SetBool.self)(w, sel, true)
    }

    private static func swap(_ cls: AnyClass, _ original: Selector, _ replacement: Selector) {
        guard let o = class_getInstanceMethod(cls, original), let r = class_getInstanceMethod(cls, replacement) else {
            FileHandle.standardError.write(Data("hidden rig: no method \(original) on \(cls)\n".utf8))
            return
        }
        method_exchangeImplementations(o, r)
    }
}
#endif
