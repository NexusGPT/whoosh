import Foundation

/// Process start, read from the kernel, so `measure mac-main.launch` is launch-to-first-paint.
public enum LaunchClock {
    public static func msSinceProcessStart() -> Double {
        var info = kinfo_proc()
        var size = MemoryLayout<kinfo_proc>.stride
        var mib: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, getpid()]
        guard sysctl(&mib, 4, &info, &size, nil, 0) == 0 else { return -1 }
        let start = info.kp_proc.p_un.__p_starttime
        let startS = Double(start.tv_sec) + Double(start.tv_usec) / 1_000_000
        return (Date().timeIntervalSince1970 - startS) * 1000
    }

    /// WHOOSH_LAUNCH_TRACE=1 (the rig's LAUNCH_ENV only; the owner's run never sets it): one stderr line per launch phase.
    private static let tracing = ProcessInfo.processInfo.environment["WHOOSH_LAUNCH_TRACE"] == "1"

    /// `launch-trace <phase> <ms since process start>` to stderr (the rig keeps it in the run folder's app.out); one flag read when off.
    /// Phases, in order: app-init, root-body, corehost-begin, corehost-end, appmodel-init-end, first-appear, first-frame, control-socket-end,
    /// settled-refresh-end, settled-changes-end, settled-push-end, settled-notch-end (checks/launch-order-check.py reads the order).
    public static func trace(_ phase: String) {
        mark(phase)
        guard tracing else { return }
        FileHandle.standardError.write(Data("launch-trace \(phase) \(String(format: "%.1f", msSinceProcessStart()))\n".utf8))
    }

    /// `trace` for a phase reached from code that runs again and again (a view body): only the first pass writes a line.
    /// launch-r3 round 2: `app-init` (the App's init, the first Swift code that runs) and `root-body` (the main window's first body)
    /// split the time before `first-appear` into the OS and dyld, AppKit and the scene, and the first layout.
    nonisolated(unsafe) private static var seen: Set<String> = []
    private static let seenLock = NSLock()
    public static func once(_ phase: String) {
        if !marked { mark(phase) }   // the launch phases: only until the first frame's turn closes them (one Bool read after it)
        guard tracing else { return }
        let first = seenLock.withLock { seen.insert(phase).inserted }
        if first { trace(phase) }
    }

    /// `measure mac-main.launch.phases` (perf-archive round 2): the first time each trace point was reached, as one clock read
    /// (CFAbsoluteTimeGetCurrent) kept in a small table; turned into ms since process start only when the measure asks.
    /// Kept whether or not WHOOSH_LAUNCH_TRACE is set, so the owner's own launch is measurable; the table stops growing at 48 points (round 4: 37 points named).
    nonisolated(unsafe) private static var marks: [(phase: String, at: CFAbsoluteTime)] = []
    nonisolated(unsafe) private static var marked = false   // set once the late phases are in: `once` then costs one Bool read
    private static let markLock = NSLock()
    public static func mark(_ phase: String) {
        let at = CFAbsoluteTimeGetCurrent()
        markLock.withLock {
            guard marks.count < 48, !marks.contains(where: { $0.phase == phase }) else { return }
            if marks.isEmpty { marks.reserveCapacity(48) }
            marks.append((phase, at))
            if phase == "settled-notch-end" { marked = true }
        }
    }

    /// Each point in the order reached, in ms since the process started (the kernel's start time, as `msSinceProcessStart`).
    public static func phases() -> [[String: Any]] {
        var info = kinfo_proc()
        var size = MemoryLayout<kinfo_proc>.stride
        var mib: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, getpid()]
        guard sysctl(&mib, 4, &info, &size, nil, 0) == 0 else { return [] }
        let start = info.kp_proc.p_un.__p_starttime
        let startAbs = Double(start.tv_sec) + Double(start.tv_usec) / 1_000_000 - kCFAbsoluteTimeIntervalSince1970
        return markLock.withLock { marks.map { ["phase": $0.phase, "ms": ($0.at - startAbs) * 1000] } }
    }
}
