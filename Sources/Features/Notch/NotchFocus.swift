import Foundation
#if os(macOS)
import AppKit
import Intents
#endif

/// N16: the system's Focus, read where macOS exposes it. Focus has no NSWorkspace property; the one public
/// door is Intents' `INFocusStatusCenter`, which answers only after the person allowed Whoosh to see the Focus
/// status (and the app carries the focus-status entitlement and NSFocusStatusUsageDescription: the shell's
/// files, handed over in docs/lanes/notch.md). Until then it answers nil and the notch keeps the value it has.
///
/// No timer: Focus is read on the inputs that matter (a mail arrives, the pointer reaches the notch, the
/// frontmost app changes), each a plain property read. The rig's `stage.focus.*` clicks set the same state.
/// A test copy never reads the system's Focus: it carries the app's permission, so an authorised read replaced the
/// stage's Focus by the Mac's own at the first pointer on the edge, and par-notch J6.19 passed or failed by whether the
/// owner's Mac was in a Focus at that minute (main205, 16 Nov 2026: `stage.focus.on`, `stage.edge` -> focus false, the list opens).
@MainActor
enum NotchFocus {
    private static var started = false
    /// The owner's own copy on a real engine: the only one that reads the Mac's Focus. A hidden test copy and the test engine keep the stage's value.
    static var readsTheSystem: Bool { !HiddenRig.isOn && AppModel.shared.host.fixture == nil }

    /// Once, from the first install: asks for the permission when Focus is followed and this is a real core
    /// (a rig run must never wait on a system dialog), then reads on app switches.
    static func start(real: Bool) {
        #if os(macOS)
        guard !started else { return }
        started = true
        NSWorkspace.shared.notificationCenter.addObserver(forName: NSWorkspace.didActivateApplicationNotification, object: nil, queue: nil) { _ in
            MainActor.assumeIsolated { _ = refresh() }
        }
        guard real, NotchModel.shared.settings.followFocus, INFocusStatusCenter.default.authorizationStatus == .notDetermined else { return }
        INFocusStatusCenter.default.requestAuthorization { _ in DispatchQueue.main.async { MainActor.assumeIsolated { _ = refresh() } } }
        #endif
    }

    /// The system Focus into the model; false when the system gives no answer.
    @discardableResult
    static func refresh(_ model: NotchModel = .shared) -> Bool {
        #if os(macOS)
        guard readsTheSystem else { return false }
        guard INFocusStatusCenter.default.authorizationStatus == .authorized,
              let on = INFocusStatusCenter.default.focusStatus.isFocused else { return false }
        if on != model.focusOn { model.setFocus(on) }
        return true
        #else
        return false
        #endif
    }
}
