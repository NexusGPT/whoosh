import Foundation

/// The facts the notch's socket row reports about its window and its last arrival, apart from the rows themselves.
@MainActor
extension NotchRows {
    /// How far below the screen a rig copy draws its panel (NotchPanel.parkedIfTesting); 0 off the Mac.
    static var parked: Double {
        #if os(macOS)
        return Double(NotchPanel.parkOffset)
        #else
        return 0
        #endif
    }

    /// The panel window is the key window right now (a hidden rig copy must never be: it would take the typing of
    /// whoever is at this Mac).
    static var isKey: Bool {
        #if os(macOS)
        return NotchPanel.shared.window?.isKeyWindow ?? false
        #else
        return false
        #endif
    }

    /// The panel holds the keyboard: NotchKeyboard took it and the window is key (the reply can type).
    static var keyed: Bool {
        #if os(macOS)
        // a hidden rig copy records the ask and never becomes key (NotchKeyboard.take), so there the ask is the reading
        return NotchKeyboard.shared.holding && (NotchPanel.parkOffset > 0 || (NotchPanel.shared.window?.isKeyWindow ?? false))
        #else
        return false
        #endif
    }

    /// The panel window's own sharing type, as AppKit holds it (0 is none: absent from screen shares); -1 before the window exists
    /// and off the Mac. par-notch reads it next to the window server's state, so a window that says none while the server
    /// lists it capturable still fails the check.
    static var sharingType: Int {
        #if os(macOS)
        return NotchPanel.shared.window.map { Int($0.sharingType.rawValue) } ?? -1
        #else
        return -1
        #endif
    }

    /// Milliseconds from the drop reaching the model to the first frame that showed it (NotchModel.shown); null until one showed.
    static var arrivalMs: Any { model.lastArrivalMs.map { $0 as Any } ?? NSNull() }

    /// Merged into the `mac-notch` row (NotchRows.all), kept here so NotchRows stays inside its line cap.
    static func withWindowFacts(_ row: ControlRow) -> ControlRow {
        row.merging(["arrival_ms": arrivalMs, "sharing_type": sharingType, "takes_keys": NotchFeature.takesKeys, "pointer_on": model.pointerOn,
                     "handed_over": model.handedOver ?? NSNull()]) { mine, _ in mine }
    }
}
