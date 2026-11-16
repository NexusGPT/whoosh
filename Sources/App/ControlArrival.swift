import Foundation
import QuartzCore

/// The wall-clock instant the control socket read the last request line (any thread). The probe takes it as the
/// arrival of an input event, so the wait for the main actor counts in the number, as it would for a real key.
public enum ControlArrival {
    private static let lock = NSLock()
    nonisolated(unsafe) private static var last: CFTimeInterval = 0

    public static func stamp() { lock.lock(); last = CACurrentMediaTime(); lock.unlock() }
    static func take() -> CFTimeInterval { lock.lock(); defer { lock.unlock() }; return last > 0 ? last : CACurrentMediaTime() }
}
