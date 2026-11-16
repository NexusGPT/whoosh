import Foundation

/// Lets the main run loop turn until a just-opened window shows (then one more turn, so its content
/// has laid out), bounded so a window that never opens fails the shot instead of hanging the socket.
@MainActor
public enum SurfaceSettle {
    /// Upper bound on the wait for a window to open, in seconds. Pending its docs/CONSTANTS.md row
    /// (SURFACE_OPEN_WAIT_S, asked of the contracts lane in docs/lanes/appshell.md "## For contracts").
    static let surfaceOpenWaitS = 2.0

    public static func wait(until done: () -> Bool) {
        let deadline = Date().addingTimeInterval(surfaceOpenWaitS)
        while !done() && Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05))
    }
}
