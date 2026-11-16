#if os(macOS)
import AppKit

// The replacements. After the exchange each `whooshHidden_` name runs AppKit's original, so a replacement calls
// its own name to reach the original.
extension NSWindow {
    @objc func whooshHidden_orderFront(_ sender: Any?) { LaunchClock.once("rig-order-front"); HiddenRig.conceal(self); whooshHidden_orderFront(sender); LaunchClock.once("rig-order-front-end") }
    @objc func whooshHidden_setIsVisible(_ v: Bool) { if v { HiddenRig.conceal(self) }; whooshHidden_setIsVisible(v) }
    @objc func whooshHidden_orderFrontRegardless() { HiddenRig.conceal(self); whooshHidden_orderFrontRegardless() }
    @objc func whooshHidden_makeKeyAndOrderFront(_ sender: Any?) { LaunchClock.once("rig-make-key-front"); HiddenRig.conceal(self); whooshHidden_makeKeyAndOrderFront(sender); LaunchClock.once("rig-make-key-front-end") }
    @objc func whooshHidden_order(_ place: NSWindow.OrderingMode, relativeTo other: Int) {
        if place != .out { HiddenRig.conceal(self) }
        whooshHidden_order(place, relativeTo: other)
    }
    @objc func whooshHidden_addChildWindow(_ child: NSWindow, ordered place: NSWindow.OrderingMode) {
        HiddenRig.conceal(child); whooshHidden_addChildWindow(child, ordered: place)
    }
    @objc func whooshHidden_setAlphaValue(_ v: CGFloat) { whooshHidden_setAlphaValue(HiddenRig.alpha(self)) }
    @objc func whooshHidden_setIgnoresMouseEvents(_ v: Bool) { whooshHidden_setIgnoresMouseEvents(true) }
}

extension NSApplication {
    @objc func whooshHidden_activate(ignoringOtherApps flag: Bool) { HiddenRig.refuse("activate") }
    @objc func whooshHidden_activateNow() { HiddenRig.refuse("activate") }
    @objc func whooshHidden_isActive() -> Bool { HiddenRig.pretendActive || whooshHidden_isActive() }
    @objc func whooshHidden_setActivationPolicy(_ p: NSApplication.ActivationPolicy) -> Bool {
        guard p != .regular else { HiddenRig.refuse("dock-tile"); return false }
        return whooshHidden_setActivationPolicy(p)
    }
}

extension NSRunningApplication {
    @objc func whooshHidden_activate(options: NSApplication.ActivationOptions = []) -> Bool {
        guard processIdentifier != getpid() else { MainActor.assumeIsolated { HiddenRig.refuse("activate") }; return false }
        return whooshHidden_activate(options: options)
    }
}

extension NSView {
    @objc func whooshHidden_acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
}

extension NSStatusBar {
    @objc func whooshHidden_statusItem(withLength length: CGFloat) -> NSStatusItem {
        let item = whooshHidden_statusItem(withLength: length)
        item.isVisible = false   // the owner's menu bar keeps no slot for the rig
        return item
    }
}
#endif
