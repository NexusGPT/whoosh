import Foundation
import CoreFoundation

/// A measurement the list serves for itself (docs/MOTION.md rule 10): how long each main-thread run loop turn took
/// between `start` and `stop`. A turn longer than FRAME_MS is a frame the owner can see; `rows mac-main.list.perf`
/// answers the count, the median and the worst. It adds one run loop observer and no timer, and it records nothing
/// until a check starts it.
enum ListPerf {
    nonisolated(unsafe) private static var observer: CFRunLoopObserver?
    nonisolated(unsafe) private static var began: CFAbsoluteTime = 0
    nonisolated(unsafe) private static var recording = false
    nonisolated(unsafe) private static var turns: [Double] = []

    static func install() {
        guard observer == nil else { return }
        let activities = CFRunLoopActivity.afterWaiting.rawValue | CFRunLoopActivity.beforeWaiting.rawValue
        let o = CFRunLoopObserverCreateWithHandler(nil, activities, true, 0) { _, activity in
            if activity == .afterWaiting { began = CFAbsoluteTimeGetCurrent(); return }
            guard recording, began > 0 else { return }
            if turns.count < 4000 { turns.append((CFAbsoluteTimeGetCurrent() - began) * 1000) }
        }
        observer = o
        CFRunLoopAddObserver(CFRunLoopGetMain(), o, .commonModes)
    }

    static func start() { turns = []; recording = true; return }
    static func stop() { recording = false }

    static func row() -> ControlRow {
        let busy = turns.filter { $0 >= 1 }.sorted()
        func at(_ q: Double) -> Double { busy.isEmpty ? 0 : busy[min(busy.count - 1, Int(Double(busy.count) * q))] }
        return ["turns": turns.count, "busy_turns": busy.count, "p50_ms": at(0.5), "p90_ms": at(0.9), "max_ms": busy.last ?? 0,
                "over_frame": busy.filter { $0 >= Double(WhooshConstants.FRAME_MS) }.count,
                "over_commit": busy.filter { $0 >= Double(WhooshConstants.INTERACTION_COMMIT_MS) }.count]
    }
}
