#if os(macOS)
import AppKit

/// The socket's way into the real menu: shown at the chip, photographed, closed.
extension AttachmentMenu {
    /// The socket's way in: shows the real menu at the chip, photographs it once it is drawn (the rig's
    /// attachments-menu shot) and closes it. A menu tracks modally and the socket is served on the main thread,
    /// so a menu the socket opened always closes itself; a person's right-click menu stays until they choose.
    static func popUp(_ ref: AttachmentRef) -> Bool {
        guard let v = AttachmentChipFrames.view(ref) else { return false }
        AttachmentKeys.select(ref)
        // Hidden rig (owner 16:30, no test window ever): the system draws a menu in a window this process does not own,
        // so a hidden copy builds the menu and never shows it; the socket reads it from `mac-main.attachments.menu`.
        if HiddenRig.isOn { shownFor = ref; builtHidden += 1; return true }
        let m = make(ref)
        m.delegate = AttachmentMenuWatch.shared
        shown = m; shownFor = ref
        // A menu tracks only in the active app; the rig copy runs in the background, so it comes forward for the menu and
        // hands the front back to the app that had it the moment the menu closes (as the socket's drag verb does).
        let front = (NSApp as NSApplication?)?.isActive ?? true ? nil : NSWorkspace.shared.frontmostApplication
        (NSApp as NSApplication?)?.activate()
        AttachmentMenuWatch.shared.open = false
        AttachmentMenuWatch.shared.openTicks = 0
        ticks = 0
        AttachmentShots.last["menu"] = nil       // a picture of an earlier menu is never handed out as this one
        Task { @MainActor in
            // Once the menu is on screen: photograph it, then close it, retrying until it says it closed (a cancel sent
            // before tracking starts is lost, and a menu left open holds the main thread).
            let t = Timer(timeInterval: Double(WhooshConstants.POPOVER_MS) / 1000, repeats: true) { _ in
                MainActor.assumeIsolated {
                    // one tick after the open notice the window server has drawn the menu: photograph it then; once
                    // photographed, or after a few ticks with no open notice, cancel every tick until popUp returns
                    AttachmentMenu.ticks += 1
                    let watch = AttachmentMenuWatch.shared
                    if watch.open { watch.openTicks += 1 }
                    if watch.openTicks >= 2 && AttachmentMenu.shotOpen != AttachmentMenu.opens {
                        _ = AttachmentShots.capture("menu"); AttachmentMenu.shotOpen = AttachmentMenu.opens
                    }
                    if AttachmentMenu.shotOpen == AttachmentMenu.opens && watch.open || AttachmentMenu.ticks > Int(WhooshSpacing.s4) {
                        AttachmentMenu.shown?.cancelTracking()
                    }
                }
            }
            RunLoop.main.add(t, forMode: .common)
            m.popUp(positioning: nil, at: NSPoint(x: v.bounds.midX, y: v.bounds.midY), in: v)
            t.invalidate()
            if shown === m { shown = nil; shownFor = nil }
            front?.activate()
        }
        return true
    }
}

/// Counts the menus that really opened (a menu the window server never drew does not count).
@MainActor
final class AttachmentMenuWatch: NSObject, NSMenuDelegate {
    static let shared = AttachmentMenuWatch()
    var open = false
    var openTicks = 0
    func menuWillOpen(_ menu: NSMenu) { AttachmentMenu.opens += 1; open = true }
    func menuDidClose(_ menu: NSMenu) { open = false }
}
#endif
