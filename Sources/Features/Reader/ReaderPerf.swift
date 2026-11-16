import Foundation
import QuartzCore

/// The reader's own cost meter (round 5, owner 2026-11-08 10:51: "navigation with arrows is WAYYYY TOO SLOW AND HEAVY").
/// Counts what the reader does per focus change (thread builds, body loads, pane body evaluations, cross-fades) and,
/// between `mac-main.reader.perf.start` and `.stop`, the length of every main-thread run loop turn. A check reads
/// `rows mac-main.reader.perf`; nothing here runs unless a check starts it, except the plain counters.
@MainActor
final class ReaderPerf {
    static let shared = ReaderPerf()

    /// Thread builds (`ReaderThread.make`), the core.body loads among them, and the time they took.
    private(set) var makes = 0
    private(set) var bodyLoads = 0
    /// Reads of a body from the core (a thread build, an expanded earlier message, the link finder); a body the shown thread
    /// already read is served from `ReaderBodies` and counts nothing.
    private(set) var bodyReads = 0
    private(set) var makeMs = 0.0
    /// Evaluations of the pane's body and swaps (a new message id on screen: the cross-fade).
    private(set) var paneEvals = 0
    private(set) var swaps = 0
    private(set) var turns: [Double] = []
    /// What named work ran in each turn (`mark`), parallel to `turns`: "" for a turn nothing marked.
    private(set) var tags: [String] = []
    private var pendingTags: Set<String> = []
    /// A piece of the reader's work says it ran in the turn that is running (recording only): the turn then carries the name in `slow_tags`.
    func mark(_ name: String) { if recording { pendingTags.insert(name) } }
    private(set) var recording = false

    private let clock = TurnClock()
    private var observer: CFRunLoopObserver?

    func noteMake(ms: Double, loadedBody: Bool) { mark("thread.make"); makes += 1; makeMs += ms; if loadedBody { bodyLoads += 1 } }
    func notePaneEval() { mark("pane.eval"); paneEvals += 1 }
    func noteSwap() { swaps += 1 }
    func noteBodyRead() { bodyReads += 1 }

    func start() {
        makes = 0; bodyLoads = 0; bodyReads = 0; makeMs = 0; paneEvals = 0; swaps = 0; turns = []; tags = []; pendingTags = []
        stop()
        // The turn that is running now (the start click's own) is not measured: the observer is added in its middle, so its first event is that turn's end, and the
        // clock still held the start of the turn in which the PREVIOUS recording was stopped (main180: one "turn" of 120,247 ms with no tag, the time since that stop).
        clock.began = 0
        let box = clock
        let obs = CFRunLoopObserverCreateWithHandler(kCFAllocatorDefault,
                                                     CFRunLoopActivity.afterWaiting.rawValue | CFRunLoopActivity.beforeWaiting.rawValue, true, 0) { _, activity in
            if activity == .afterWaiting { box.began = CACurrentMediaTime(); return }
            guard box.began > 0 else { return }
            let ms = (CACurrentMediaTime() - box.began) * 1000
            box.began = 0
            // Recorded in this very callback (the observer runs on the main run loop). It used to post `DispatchQueue.main.async` from `beforeWaiting`: that wakes the loop,
            // whose next `beforeWaiting` posts again, so the meter spun the main thread by itself (main100: 339,437 turns in a 3 s window, `update.queue` 30 ms median in the
            // phases read while it ran) and the frame-budget case measured its own flood.
            MainActor.assumeIsolated {
                let p = ReaderPerf.shared
                p.turns.append(ms)
                p.tags.append(p.pendingTags.sorted().joined(separator: "+"))
                p.pendingTags = []
            }
        }
        observer = obs
        if let obs { CFRunLoopAddObserver(CFRunLoopGetMain(), obs, .commonModes) }
        recording = true
    }

    func stop() {
        if let observer { CFRunLoopRemoveObserver(CFRunLoopGetMain(), observer, .commonModes) }
        observer = nil
        recording = false
    }

    /// One row for the socket: turns of the main thread and the reader's counters since `start`.
    func row(frameMs: Double) -> ControlRow {
        let sorted = turns.sorted()
        let slowIdx = Array(turns.indices.sorted { turns[$0] > turns[$1] }.prefix(12)).filter { turns[$0] > frameMs }
        let slow = slowIdx.map { turns[$0] }
        let p90 = sorted.isEmpty ? 0 : sorted[min(sorted.count - 1, Int(Double(sorted.count) * 0.9))]
        return ["recording": recording, "turns": turns.count, "over_frame": turns.filter { $0 > frameMs }.count,
                "p90_ms": round(p90 * 10) / 10, "max_ms": round((sorted.last ?? 0) * 10) / 10,
                "busy_ms": round(turns.reduce(0, +) * 10) / 10, "frame_ms": frameMs,
                "slow_ms": slow.map { ($0 * 10).rounded() / 10 },   // the longest turns over the frame, longest first
                "slow_tags": slowIdx.map { $0 < tags.count ? tags[$0] : "" },   // what named work ran in each of them (ReaderPerf.mark)
                "makes": makes, "body_loads": bodyLoads, "body_reads": bodyReads, "make_ms": round(makeMs * 10) / 10, "pane_evals": paneEvals, "swaps": swaps]
    }
}

/// The run loop observer's one mutable cell; it is touched on the main thread only.
private final class TurnClock: @unchecked Sendable {
    var began = 0.0
}
