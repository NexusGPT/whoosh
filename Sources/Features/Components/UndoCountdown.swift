import Foundation

/// The undo window as a value: counts down from UNDO_SEND_DEFAULT_S (docs/CONSTANTS.md) unless the setting
/// moved it inside UNDO_SEND_MIN...UNDO_SEND_MAX. The view reads it on every tick; nothing here owns a timer.
public struct UndoCountdown: Equatable, Sendable {
    public let total: Double
    public let startedAt: Date

    public init(startedAt: Date, seconds: Int = WhooshConstants.UNDO_SEND_DEFAULT_S) {
        self.startedAt = startedAt
        total = Double(min(max(seconds, WhooshConstants.UNDO_SEND_MIN), WhooshConstants.UNDO_SEND_MAX))
    }

    public func remaining(at now: Date) -> Double { max(0, total - now.timeIntervalSince(startedAt)) }
    /// The whole seconds shown: 10, 9, … 1, then expired.
    public func secondsLeft(at now: Date) -> Int { Int(remaining(at: now).rounded(.up)) }
    /// 1 at the start, 0 when the window has closed.
    public func fraction(at now: Date) -> Double { remaining(at: now) / total }
    public func isExpired(at now: Date) -> Bool { remaining(at: now) <= 0 }
}

public enum UndoKey {
    /// The key cap shown on every undo control.
    public static let letter = "Z"
    /// Z undoes; the chord arrives as "Z" from the socket and the keyboard monitor, "z" from SwiftUI.
    public static func emitsUndo(_ chord: String) -> Bool { chord == "Z" || chord == "z" }
}
