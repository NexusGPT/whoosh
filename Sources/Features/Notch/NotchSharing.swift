#if os(macOS)
import AppKit

extension NotchPanel {
    /// Out of screen shares and other apps' captures when the setting is on; capturable when it is off.
    ///
    /// Measured on main43 and main44 (hidden rig, the window server's own list): a window made capturable while it is
    /// up turns capturable at once (sharing state 1), but setting `.none` on that live window, even ordered out and
    /// back in, leaves the window server at 1, so the hidden setting did not hide it. A window that gets `.none`
    /// before it is first ordered in reads 0 (every launch). So going back to hidden builds a fresh window that way.
    ///
    /// Round 7 (main51, hidden rig, window server list beside the row's `sharing_type`): after the setting went off the
    /// server read 1 while `NSWindow.sharingType` still answered none, and after it went on again the guard below saw
    /// none == none, never rebuilt, and the server stayed at 1 (the same window number all along). The getter is not
    /// the window server's state, so the guard reads `appliedSharing`, what this panel last asked the server for.
    func applySharing() {
        guard let w = window else { return }
        let want: NSWindow.SharingType = NotchModel.shared.settings.hiddenInScreenShare ? .none : .readOnly
        guard appliedSharing != want else { return }
        // Gated on the window server's window existing (windowNumber > 0), not on `isVisible`: a rig copy runs hidden and
        // parked, and a window it reports as not visible is still in the window server's list, still capturable.
        // A window the server has never seen (every launch) simply takes `.none` below (round 5).
        if want == .none && w.windowNumber > 0 { replaceWindow(sharing: .none); return }
        w.sharingType = want
        appliedSharing = want
    }

    /// The notch's borderless panel, configured the one way both the first window and a replacement are made.
    static func makeWindow() -> NotchWindow {
        let w = NotchWindow(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        w.sharingType = .none   // every window the notch creates is out of screen shares from its creation; a setting that is off lifts it later
        w.title = NotchIds.windowTitle
        w.level = NSWindow.Level(rawValue: Int(CGWindowLevelForKey(.statusWindow)) + 1)
        w.collectionBehavior = [.canJoinAllSpaces, .stationary, .fullScreenAuxiliary, .ignoresCycle]
        w.isFloatingPanel = true
        w.hidesOnDeactivate = false
        w.isOpaque = false
        w.backgroundColor = .clear
        w.hasShadow = false
        w.isMovable = false
        w.isReleasedWhenClosed = false
        w.animationBehavior = .none
        w.acceptsMouseMovedEvents = true
        w.ignoresMouseEvents = true
        return w
    }
}
#endif
