#if os(macOS)
import AppKit

/// Moving the notch along the top edge with an option-drag. Adapted from the owner's ghostty fork
/// (macos/Sources/Features/OneTerminal/NotchPanel.swift, "moving the notch by dragging it", by the owner, MIT).
///
/// Taken in the panel's own `sendEvent`, so a drag never falls through to be read as a click. Refused on a
/// notched Mac (the shape is centred on the housing, a drag there would write a number that moves no pixel) and
/// under a typist (the reply holds the keyboard). The pointer's own travel since the press is measured: no
/// NSEvent delta is read, because those are unflipped and a sign error makes the shape run away from the cursor.
/// One write, at the release.
@MainActor
final class NotchDrag {
    static let shared = NotchDrag()
    private static let key = "notch.horizontalFraction"
    private struct Press { var from: Double; var at: NSPoint; var span: CGFloat }
    private var press: Press?
    /// Where the shape sits along the top edge on a Mac without a housing: the live drag, else the saved place.
    private(set) var fraction: Double = UserDefaults.standard.object(forKey: NotchDrag.key) as? Double ?? 0.5

    var dragging: Bool { press != nil }

    /// The test door for the whole gesture (sweep-notch F4): the same write the release makes, for a travel of `delta` of the screen's width. Refused where the real
    /// drag is (a notched Mac, a typist), so a door cannot move what a person cannot.
    @discardableResult
    func moveByDoor(_ delta: Double) -> Bool {
        let panel = NotchPanel.shared
        guard panel.geometry.shape == .none, !NotchKeyboard.shared.holding, press == nil else { return false }
        fraction = min(1, max(0, fraction + delta))
        UserDefaults.standard.set(fraction, forKey: Self.key)
        panel.place()
        return true
    }

    /// True when the event was taken.
    func take(_ e: NSEvent, in window: NotchWindow) -> Bool {
        switch e.type {
        case .leftMouseDown: return begin(e, in: window)
        case .leftMouseDragged: guard press != nil else { return false }; move(); return true
        case .leftMouseUp: guard press != nil else { return false }; end(); return true
        default: return false
        }
    }

    private func begin(_ e: NSEvent, in window: NotchWindow) -> Bool {
        let panel = NotchPanel.shared
        guard e.modifierFlags.contains(.option), panel.geometry.shape == .none, !NotchKeyboard.shared.holding,
              let screen = NotchGeometry.screen else { return false }
        guard panel.pointer.shapeRect.contains(NSEvent.mouseLocation) else { return false }
        press = Press(from: fraction, at: NSEvent.mouseLocation, span: max(1, screen.frame.width))
        return true
    }

    private func move() {
        guard let p = press else { return }
        fraction = min(1, max(0, p.from + Double((NSEvent.mouseLocation.x - p.at.x) / p.span)))
        NotchPanel.shared.place()
    }

    private func end() {
        press = nil
        UserDefaults.standard.set(fraction, forKey: Self.key)
        NotchPanel.shared.place()
    }
}
#endif
