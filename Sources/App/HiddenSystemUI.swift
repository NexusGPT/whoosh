#if os(macOS)
import AppKit

/// System-provided UI on a hidden rig (lead, 18:17: Quick Look panels of a hidden copy reached the owner's screen at
/// alpha 1). A panel the system draws is not one of our windows to conceal, so on a hidden rig it is never shown:
/// each entry point below records `<kind>` (socket: `core state system-ui/<kind>` = count, `system-ui/<kind>/<n>` =
/// what was asked) and returns as a cancel would. Lanes ask `HiddenRig.allowSystemUI(kind)` before showing one.
/// Classes are found by name, so no framework is linked for this (Quick Look, user notifications).
@MainActor
public extension HiddenRig {
    /// False on a hidden rig: no system panel, menu, alert, picker, other app or notification may show.
    static var systemUIAllowed: Bool { !isOn }

    /// The one call before showing system UI: true when it may show; on a hidden rig it records the ask and says no.
    @discardableResult
    static func allowSystemUI(_ kind: String, _ detail: String = "") -> Bool {
        guard isOn else { return true }
        systemUI[kind, default: []].append(detail)
        return false
    }

    /// What each kind of system UI was asked to show on this hidden rig, in order.
    internal static var systemUI: [String: [String]] = [:]
    /// Every entry point hooked, by "Class selector" (`fixture system-ui hooks` lists them).
    internal static var hooked: [String] = []

    internal static func installSystemUI() {
        typealias V1 = @convention(block) (AnyObject, AnyObject?) -> Void
        typealias V0 = @convention(block) (AnyObject) -> Void
        typealias Modal = @convention(block) (AnyObject) -> Int
        typealias Done = @convention(block) (AnyObject, @escaping (Int) -> Void) -> Void
        typealias SheetDone = @convention(block) (AnyObject, AnyObject?, @escaping (Int) -> Void) -> Void
        let cancel = NSApplication.ModalResponse.cancel.rawValue
        // Loaded now, so their classes exist to be hooked before a lane's first use (a later load would arrive unhooked).
        for fw in ["Quartz", "QuickLookUI", "UserNotifications", "EventKit", "AVFoundation"] { _ = Bundle(path: "/System/Library/Frameworks/\(fw).framework")?.load() }
        LaunchClock.mark("rig-frameworks-loaded")   // perf-archive r3: rig only; the owner's launch never loads these here
        for sel in ["orderFront:", "makeKeyAndOrderFront:"] {
            refuse("QLPreviewPanel", sel, { (_, _) in rec("quick-look", sel) } as V1)
        }
        refuse("QLPreviewPanel", "orderFrontRegardless", { _ in rec("quick-look", "orderFrontRegardless") } as V0)
        for cls in ["NSSavePanel", "NSOpenPanel"] {
            refuse(cls, "runModal", { _ in rec("save-panel", cls + " runModal"); return cancel } as Modal)
            refuse(cls, "beginWithCompletionHandler:", { _, done in rec("save-panel", cls + " begin"); DispatchQueue.main.async { done(cancel) } } as Done)
            refuse(cls, "beginSheetModalForWindow:completionHandler:", { _, _, done in rec("save-panel", cls + " sheet"); DispatchQueue.main.async { done(cancel) } } as SheetDone)
        }
        refuse("NSAlert", "runModal", { _ in rec("alert", "runModal"); return cancel } as Modal)
        refuse("NSAlert", "beginSheetModalForWindow:completionHandler:", { _, _, done in rec("alert", "sheet"); DispatchQueue.main.async { done(cancel) } } as SheetDone)
        refuse("NSMenu", "popUpMenuPositioningItem:atLocation:inView:",
               { (_, _, _, _) in rec("menu", "popUp"); return false } as @convention(block) (AnyObject, AnyObject?, NSPoint, AnyObject?) -> Bool)
        refuse("NSMenu", "popUpContextMenu:withEvent:forView:", meta: true,
               { (_, _, _, _) in rec("menu", "popUpContextMenu") } as @convention(block) (AnyObject, AnyObject?, AnyObject?, AnyObject?) -> Void)
        refuse("NSMenu", "popUpContextMenu:withEvent:forView:withFont:", meta: true,
               { (_, _, _, _, _) in rec("menu", "popUpContextMenu") } as @convention(block) (AnyObject, AnyObject?, AnyObject?, AnyObject?, AnyObject?) -> Void)
        refuse("NSSharingServicePicker", "showRelativeToRect:ofView:preferredEdge:",
               { (_, _, _, _) in rec("share", "show") } as @convention(block) (AnyObject, NSRect, AnyObject?, Int) -> Void)
        installOpenAndNotify()
        installPrivacyPrompts()
    }

    /// The two macOS privacy prompts (probes-r1, R4): Calendar and microphone. A hidden rig never reaches either: the request is
    /// recorded as `system-ui/calendar-access` or `system-ui/microphone` and answered "not granted", as a refusal would.
    private static func installPrivacyPrompts() {
        refuse("EKEventStore", "requestFullAccessToEventsWithCompletion:",
               { (_, done) in rec("calendar-access", "full access"); done?(false, nil) } as @convention(block) (AnyObject, ((Bool, AnyObject?) -> Void)?) -> Void)
        refuse("AVCaptureDevice", "requestAccessForMediaType:completionHandler:", meta: true,
               { (_, _, done) in rec("microphone", "audio"); done?(false) } as @convention(block) (AnyObject, AnyObject?, ((Bool) -> Void)?) -> Void)
    }

    /// Another app's window (NSWorkspace) and the Notification Center (a banner and the permission prompt).
    private static func installOpenAndNotify() {
        typealias OpenCfg = @convention(block) (AnyObject, AnyObject?, AnyObject?, ((AnyObject?, AnyObject?) -> Void)?) -> Void
        refuse("NSWorkspace", "openURL:", { (_, u) in rec("open", "\(u.map { "\($0)" } ?? "")"); return false } as @convention(block) (AnyObject, AnyObject?) -> Bool)
        refuse("NSWorkspace", "openURL:configuration:completionHandler:", { _, u, _, done in rec("open", "\(u.map { "\($0)" } ?? "")"); done?(nil, nil) } as OpenCfg)
        refuse("NSWorkspace", "openApplicationAtURL:configuration:completionHandler:", { _, u, _, done in rec("open", "app \(u.map { "\($0)" } ?? "")"); done?(nil, nil) } as OpenCfg)
        refuse("NSWorkspace", "openURLs:withApplicationAtURL:configuration:completionHandler:",
               { (_, _, _, _, done) in rec("open", "urls"); done?(nil, nil) } as @convention(block) (AnyObject, AnyObject?, AnyObject?, AnyObject?, ((AnyObject?, AnyObject?) -> Void)?) -> Void)
        refuse("NSWorkspace", "activateFileViewerSelectingURLs:", { (_, _) in rec("open", "Finder") } as @convention(block) (AnyObject, AnyObject?) -> Void)
        refuse("UNUserNotificationCenter", "addNotificationRequest:withCompletionHandler:",
               { (_, _, done) in rec("notification", "add"); done?(nil) } as @convention(block) (AnyObject, AnyObject?, ((AnyObject?) -> Void)?) -> Void)
        refuse("UNUserNotificationCenter", "requestAuthorizationWithOptions:completionHandler:",
               { (_, _, done) in rec("notification", "authorization"); done?(false, nil) } as @convention(block) (AnyObject, UInt, ((Bool, AnyObject?) -> Void)?) -> Void)
    }

    /// A test copy that is NOT hidden (a `.rig` bundle, a socket folder: `CorePlace.rig` without WHOOSH_RIG_HIDDEN) gets these two only:
    /// NSWorkspace refuses to open an http(s) address off this Mac (`RigNet.refusesWebOpen`), records it under `system-ui/open`, and opens
    /// everything else as before. A hidden rig refuses every open in `installOpenAndNotify` instead. (Rig network audit, gap 5.)
    internal static func installWebRefusals() {
        guard !webRefusalsInstalled, let c = NSClassFromString("NSWorkspace") else { return }
        webRefusalsInstalled = true
        let s1 = NSSelectorFromString("openURL:")
        if let m = class_getInstanceMethod(c, s1) {
            typealias Orig = @convention(c) (AnyObject, Selector, AnyObject?) -> Bool
            let orig = unsafeBitCast(method_getImplementation(m), to: Orig.self)
            let block: @convention(block) (AnyObject, AnyObject?) -> Bool = { ws, u in
                if RigNet.refusesWebOpen(u as? URL) { recordRefusedOpen(u); return false }
                return orig(ws, s1, u)
            }
            method_setImplementation(m, imp_implementationWithBlock(block))
            hooked.append("NSWorkspace openURL: (web off this Mac)")
        }
        let s2 = NSSelectorFromString("openURL:configuration:completionHandler:")
        if let m = class_getInstanceMethod(c, s2) {
            typealias Orig = @convention(c) (AnyObject, Selector, AnyObject?, AnyObject?, AnyObject?) -> Void
            let orig = unsafeBitCast(method_getImplementation(m), to: Orig.self)
            let block: @convention(block) (AnyObject, AnyObject?, AnyObject?, AnyObject?) -> Void = { ws, u, cfg, done in
                if RigNet.refusesWebOpen(u as? URL) {
                    recordRefusedOpen(u)
                    if let done { unsafeBitCast(done, to: (@convention(block) (AnyObject?, AnyObject?) -> Void).self)(nil, nil) }
                    return
                }
                orig(ws, s2, u, cfg, done)
            }
            method_setImplementation(m, imp_implementationWithBlock(block))
            hooked.append("NSWorkspace openURL:configuration:completionHandler: (web off this Mac)")
        }
    }
    private static var webRefusalsInstalled = false

    /// The decision of `install()` for a copy that is not hidden: the web refusals go in on every test copy (`env` explicit for the tests).
    nonisolated static func webRefusalsOnly(env: [String: String], arguments: [String] = []) -> Bool {
        let hidden = env["WHOOSH_RIG_HIDDEN"] == "1" || arguments.contains("--rig-hidden")
        return !hidden && RigNet.rig(env: env)
    }

    /// Records a refused open (`system-ui/open`) on any test copy, hidden or not.
    nonisolated private static func recordRefusedOpen(_ u: AnyObject?) {
        let text = "\(u.map { "\($0)" } ?? "")"   // the text is made here: the object itself must not cross into the main-actor block
        onMain { systemUI["open", default: []].append(text) }
    }

    /// Replaces `cls`'s `sel` (or adds it on that class, so a superclass's own method is never touched).
    private static func refuse(_ cls: String, _ sel: String, meta: Bool = false, _ block: Any) {
        guard let c0 = NSClassFromString(cls), let c = meta ? object_getClass(c0) : c0 else { return }
        let s = NSSelectorFromString(sel)
        guard let m = class_getInstanceMethod(c, s) else { return }
        let imp = imp_implementationWithBlock(block)
        if !class_addMethod(c, s, imp, method_getTypeEncoding(m)) { method_setImplementation(m, imp) }
        hooked.append(cls + (meta ? " +" : " ") + sel)
    }

    nonisolated private static func rec(_ kind: String, _ detail: String) {
        onMain { _ = allowSystemUI(kind, detail) }
    }

    /// A refused call is recorded on the main actor from whatever thread made it. The engine's event thread posts a notification for an arriving mail
    /// (ArrivalRouter.handle), and `MainActor.assumeIsolated` there ended the test copy (main111, real-engine attach-and-send slice: EXC_BREAKPOINT in
    /// dispatch_assert_queue under the hook of `addNotificationRequest:withCompletionHandler:`). On the main thread the record is written at once, as before.
    nonisolated private static func onMain(_ work: @escaping @MainActor @Sendable () -> Void) {
        if Thread.isMainThread { MainActor.assumeIsolated { work() } } else { DispatchQueue.main.async { MainActor.assumeIsolated { work() } } }
    }
}
#endif
