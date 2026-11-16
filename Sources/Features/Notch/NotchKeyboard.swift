#if os(macOS)
import AppKit

/// The keyboard, taken and given back. Adapted from the owner's ghostty fork
/// (macos/Sources/Features/OneTerminal/NotchPanel.swift `takeKeyboard` / `releaseKeyboard`, by the owner, MIT).
///
/// The panel is non-activating, so it can never become key on its own: the reply field would draw, take a click
/// and swallow every letter (the owner's measured "I can't even type on the notch chat input"). So the reply asks:
/// `take()` remembers the app that was in front, lets the panel become key and makes it key; `release()` drops the
/// key status (ordering the panel out and straight back) and hands the front back to that app, by yielding to it,
/// the one call measured to move the front reliably.
@MainActor
final class NotchKeyboard {
    static let shared = NotchKeyboard()
    /// Held strongly: a weak one is zeroed between the take and the release, and there is then nothing to hand back to.
    private var previousApp: NSRunningApplication?
    /// The last app in front that was not Whoosh: asking who is in front at the moment of typing can answer "you".
    private var lastOtherFront: NSRunningApplication?
    private(set) var holding = false
    private var observer: NSObjectProtocol?

    func start() {
        guard observer == nil else { return }
        let me = ProcessInfo.processInfo.processIdentifier
        lastOtherFront = NSWorkspace.shared.frontmostApplication.flatMap { $0.processIdentifier == me ? nil : $0 }
        observer = NSWorkspace.shared.notificationCenter.addObserver(forName: NSWorkspace.didActivateApplicationNotification,
                                                                     object: nil, queue: .main) { note in
            guard let app = note.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication,
                  app.processIdentifier != me else { return }
            MainActor.assumeIsolated { NotchKeyboard.shared.lastOtherFront = app }
        }
    }

    /// The reply field asks for the keyboard (on appearing, and on a click into it).
    func take() {
        guard let w = NotchPanel.shared.window else { return }
        if !holding {
            let me = ProcessInfo.processInfo.processIdentifier
            let front = NSWorkspace.shared.frontmostApplication
            previousApp = lastOtherFront ?? (front?.processIdentifier == me ? nil : front)
        }
        holding = true
        w.wantsKey = true
        // A hidden rig copy never takes the keyboard: becoming key there takes the keystrokes of whoever is typing on
        // this Mac (measured: a drive's open mail filled with "eeeee" from another window). It only records the ask.
        guard NotchPanel.parkOffset == 0 else { return }
        NSApp.activate(ignoringOtherApps: true)
        w.makeKeyAndOrderFront(nil)
    }

    /// The reply is gone: the keyboard goes back to the app that had it. A no-op when nothing was taken.
    func release() {
        guard holding, let w = NotchPanel.shared.window else { return }
        holding = false
        w.wantsKey = false
        // `canBecomeKey` turning false does not take key status away; out and straight back in does.
        w.orderOut(nil)
        w.orderFrontRegardless()
        let prev = previousApp
        previousApp = nil
        guard let prev, !prev.isTerminated, prev.processIdentifier != ProcessInfo.processInfo.processIdentifier else { return }
        NSApp.yieldActivation(to: prev)
        _ = prev.activate(options: [])
        NSApp.deactivate()
    }
}
#endif
