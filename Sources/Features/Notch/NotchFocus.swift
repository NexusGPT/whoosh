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
/// frontmost app changes), each a plain property read. The rig's `stage.focus.*` clicks set the same state and
/// are never overwritten by an unauthorised read.
@MainActor
enum NotchFocus {
    private static var started = false

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
        guard INFocusStatusCenter.default.authorizationStatus == .authorized,
              let on = INFocusStatusCenter.default.focusStatus.isFocused else { return false }
        if on != model.focusOn { model.setFocus(on) }
        return true
        #else
        return false
        #endif
    }
}
