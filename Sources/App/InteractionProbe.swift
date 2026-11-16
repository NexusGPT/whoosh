import Foundation
import QuartzCore
#if os(macOS)
import AppKit
#endif

/// `measure mac-main.interaction.frame`, `.commit` and `.gap` (docs/CONTROL-SOCKET.md): for the LAST input event the socket's
/// `key` or `click` verb delivered, the milliseconds from the event's arrival to
///   commit: the first main run-loop idle point after Core Animation's commit (SwiftUI has applied the state change and
///           the transaction has gone to the render server); every pending update ran before that point, so it is
///           the earliest instant the change is committed;
///   frame:  the presentation time of the first display-link tick after that commit (the next frame the window shows).
///   gap:    the longest interval between two consecutive display-link ticks from the event's arrival to that frame
///           (the arrival counts as the first point, the frame's target time as the last): a main thread blocked for
///           three refreshes shows as a gap of about three refreshes, even when the frame itself lands on time.
/// Nothing here touches a feature view. A measure with no event yet, or a frame that did not present, is an error
/// reply (not judged), never a 0.
@MainActor
public enum InteractionProbe {
    /// Longest the `measure` verb lets the main run loop turn waiting for a pending frame, in seconds. A frame later
    /// than this answers not judged (docs/CONSTANTS.md INTERACTION_MEASURE_WAIT_S).
    static let frameWaitS = Double(WhooshConstants.INTERACTION_MEASURE_WAIT_S)

    private static var generation = 0
    private static var arrival: CFTimeInterval = 0
    private static var commitMs: Double?
    private static var frameMs: Double?
    private static var gapMs: Double?, times: [CFTimeInterval] = []   // the gap, and its tick timeline
    private static var events = 0
    #if os(macOS)
    private static var observer: CFRunLoopObserver?
    private static var link: CADisplayLink?
    private static var rigTimer: Timer?   // hidden rig only: the display link's stand-in
    private static let ticker = Ticker()
    #endif

    /// `measure mac-main.interaction.phases`: where the main thread's time went for the LAST socket event, in ms per phase, each
    /// named by the function that does it. A phase is two clock reads around an existing call (`phaseStart` / `phaseEnd`); they read
    /// the clock only while a socket event is armed (from `run` to its frame), so the owner's own keys pay one Bool test and no read.
    /// A phase that ran twice in one event adds up; one that did not run reads 0. `arrival.queue` is the socket's arrival to the
    /// dispatch, `dispatch` the whole key handler, `update` the handler's end to the commit (SwiftUI's view update, layout and
    /// Core Animation's commit), so `update` minus the phases timed inside it is the views' own share.
    public enum Phase: Int, CaseIterable {
        case arrivalQueue, dispatch, triageTargets, engineRun, refreshInbox, refreshCounts, searchActed, listSay, readerMake, readerSettle, readerSummary, update
        // round 3: `update` split at the first idle point after the handler: main-queue blocks and timers before it (`update.queue`), the
        // before-waiting observers ordered ahead of Core Animation's commit (AppKit's layout and display, SwiftUI's view graph:
        // `update.observers`), and the commit itself up to the measure's own observer (`update.commit`). `InboxListState.say` split
        // into its observed writes and its expiry task.
        case updateQueue, updateObservers, updateCommit, sayWrites, sayTask
        // round 6: COUNTS, not ms: ListChrome's body evaluations for the event, and those made while the key handler still ran (a
        // synchronous view update triggered by an observed write, the remaining suspect of say.writes' 9 ms)
        case chromeBodies, chromeBodiesInDispatch
        // round 7: COUNTS of the list rows' body evaluations for the event (all, and while the handler ran)
        case rowBodies, rowBodiesInDispatch
        var name: String {
            ["arrival.queue", "dispatch", "TriageActions.targets", "WhooshCore.run", "AppModel.refresh.inbox", "AppModel.refresh.counts",
             "SearchMacModel.acted", "InboxListState.say", "ReaderThread.make", "ReaderFocus.changed", "ReaderSummaryStore.load", "update",
             "update.queue", "update.observers", "update.commit", "InboxListState.say.writes", "InboxListState.say.task",
             "ListChrome.body (count)", "ListChrome.body in dispatch (count)", "InboxRowView.body (count)", "InboxRowView.body in dispatch (count)"][rawValue]
        }
    }
    private static var phaseMs = [Double](repeating: 0, count: Phase.allCases.count)   // allocated once; reset in place per event
    private static var phasesOn = false
    private static var dispatchEnd: CFTimeInterval = 0
    private static var idleFirst: CFTimeInterval = 0, idleBeforeCommit: CFTimeInterval = 0   // the two split points of `update`
    #if os(macOS)
    private static var splitObservers: [CFRunLoopObserver] = []
    #endif

    /// The clock when a socket event is armed, else 0 (no read). Pair with `phaseEnd`.
    static func phaseStart() -> CFTimeInterval { phasesOn ? CACurrentMediaTime() : 0 }
    /// Adds the time since `t0` to the phase; nothing (no read) when `t0` is 0.
    static func phaseEnd(_ p: Phase, _ t0: CFTimeInterval) {
        guard t0 > 0, phasesOn else { return }
        phaseMs[p.rawValue] += (CACurrentMediaTime() - t0) * 1000
    }
    /// Counts one evaluation of a view body (round 6): one Bool test when no socket event is armed, no clock read at all.
    static func noteBody(_ p: Phase, inDispatch q: Phase) {
        guard phasesOn else { return }
        phaseMs[p.rawValue] += 1
        if dispatchEnd == 0 { phaseMs[q.rawValue] += 1 }
    }
    /// Adds a duration a caller already measured with its own two reads (ReaderThread.make's ReaderPerf stamp).
    static func phaseAdd(_ p: Phase, ms: Double) {
        guard phasesOn else { return }
        phaseMs[p.rawValue] += ms
    }

    /// Runs one input event's dispatch and arms the two measures for it.
    static func run<T>(_ dispatch: () throws -> T) rethrows -> T {
        generation += 1; events += 1
        arrival = ControlArrival.take()
        commitMs = nil; frameMs = nil; gapMs = nil
        times = [arrival]
        for i in phaseMs.indices { phaseMs[i] = 0 }
        phasesOn = true
        dispatchEnd = 0; idleFirst = 0; idleBeforeCommit = 0
        arm(generation)
        let began = CACurrentMediaTime()
        phaseMs[Phase.arrivalQueue.rawValue] = (began - arrival) * 1000
        let out = try dispatch()
        dispatchEnd = CACurrentMediaTime()
        phaseMs[Phase.dispatch.rawValue] = (dispatchEnd - began) * 1000
        debugSlow()
        return out
    }

    private static func arm(_ gen: Int) {
        #if os(macOS)
        if let o = observer { CFRunLoopObserverInvalidate(o) }
        link?.invalidate(); link = nil
        // after Core Animation's own commit observer (order 2_000_000), at the first idle point of the main run loop
        let o = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.beforeWaiting.rawValue, false, 2_000_001) { _, _ in
            MainActor.assumeIsolated { committed(gen) }
        }
        observer = o
        CFRunLoopAddObserver(CFRunLoopGetMain(), o, .commonModes)
        // the two split points of `update`: one clock read each, at the first idle point after the handler, then removed (committed)
        for o in splitObservers { CFRunLoopObserverInvalidate(o) }
        splitObservers = [(0, true), (1_999_999, false)].map { order, first in
            let s = CFRunLoopObserverCreateWithHandler(nil, CFRunLoopActivity.beforeWaiting.rawValue, true, order) { _, _ in
                MainActor.assumeIsolated {
                    guard gen == generation, dispatchEnd > 0, commitMs == nil else { return }
                    if first { if idleFirst == 0 { idleFirst = CACurrentMediaTime() } } else if idleBeforeCommit == 0 { idleBeforeCommit = CACurrentMediaTime() }
                }
            }!
            CFRunLoopAddObserver(CFRunLoopGetMain(), s, .commonModes)
            return s
        }
        rigTimer?.invalidate(); rigTimer = nil
        ticker.armed = (gen, .infinity)   // no commit yet: ticks only extend the gap timeline
        // Hidden rig: NO display link ticks there, neither the alpha-0 window's (main43) nor the screen's (main44: commit
        // 21 to 41 ms on every event, frame and gap never, even 1 s after the key with the main queue free). A main run-loop
        // timer at RIG_FRAME_TICK_HZ stands in: it fires in the measure's own run-loop wait, and a blocked main thread delays
        // its ticks exactly as it delays frames, so the gap reads the same. The owner's copy keeps the window's display link.
        if HiddenRig.isOn {
            let t = Timer(timeInterval: 1 / Double(WhooshConstants.RIG_FRAME_TICK_HZ), repeats: true) { _ in
                MainActor.assumeIsolated { let now = CACurrentMediaTime(); tick(at: now, target: now, gen: ticker.armed.gen, after: ticker.armed.commit) }
            }
            RunLoop.main.add(t, forMode: .common)
            rigTimer = t
            return
        }
        guard let w = WindowBridge.mainWindow() else { return }
        let l = w.displayLink(target: ticker, selector: #selector(Ticker.tick(_:)))
        l.add(to: .main, forMode: .common)
        link = l
        #endif
    }

    #if os(macOS)
    private static func committed(_ gen: Int) {
        guard gen == generation, commitMs == nil else { return }
        let now = CACurrentMediaTime()
        commitMs = (now - arrival) * 1000
        phaseMs[Phase.update.rawValue] = (now - dispatchEnd) * 1000
        if idleFirst > 0, idleBeforeCommit >= idleFirst {
            phaseMs[Phase.updateQueue.rawValue] = (idleFirst - dispatchEnd) * 1000
            phaseMs[Phase.updateObservers.rawValue] = (idleBeforeCommit - idleFirst) * 1000
            phaseMs[Phase.updateCommit.rawValue] = (now - idleBeforeCommit) * 1000
        }
        for o in splitObservers { CFRunLoopObserverInvalidate(o) }
        splitObservers = []
        if let o = observer { CFRunLoopObserverInvalidate(o) }   // committed: the observer no longer runs on every later idle point
        observer = nil
        ticker.armed = (gen, now)
    }

    fileprivate static func ticked(_ l: CADisplayLink, gen: Int, after commit: CFTimeInterval) {
        tick(at: l.timestamp, target: l.targetTimestamp, gen: gen, after: commit)
    }

    /// One tick of the display (or of the rig's stand-in timer): extends the gap timeline; the first tick after the commit
    /// is the frame.
    private static func tick(at ts: CFTimeInterval, target: CFTimeInterval, gen: Int, after commit: CFTimeInterval) {
        guard gen == generation, frameMs == nil else { return }
        if ts > (times.last ?? 0) { times.append(ts) }
        guard ts >= commit else { return }
        frameMs = (target - arrival) * 1000
        if target > (times.last ?? 0) { times.append(target) }
        gapMs = zip(times, times.dropFirst()).map { ($1 - $0) * 1000 }.max()
        phasesOn = false   // the frame is on screen: later work is not this event's
        link?.invalidate(); link = nil
        rigTimer?.invalidate(); rigTimer = nil
    }
    #endif

    static func answer(_ id: String) throws -> [String: Any] {
        // perf-archive r6: each call into the meaning model (EmbedTimes), served here because this file already owns the prefix; no key needed
        if id == "mac-main.interaction.embed" { return ["calls": EmbedTimes.rows()] }
        let kinds = ["frame", "commit", "gap", "phases"]
        guard let kind = kinds.first(where: { id == "mac-main.interaction." + $0 }) else {
            throw ControlFailure("no measurement at \(id): the interaction measures are mac-main.interaction.frame, .commit, .gap, .phases and .embed")
        }
        guard events > 0 else { throw ControlFailure("not judged: no key or click has been delivered yet") }
        let gen = generation
        let deadline = Date().addingTimeInterval(frameWaitS)
        while (commitMs == nil || frameMs == nil || gapMs == nil) && Date() < deadline && gen == generation {
            RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.002))
        }
        #if DEBUG
        if ProcessInfo.processInfo.environment["WHOOSH_MUTANT_CONSTANT_MEASURE"] == "1" { return ["ms": 1.0, "event": events] }
        #endif
        phasesOn = false   // the measure's own wait is not the event's work
        if kind == "phases" {
            guard commitMs != nil else { throw ControlFailure("not judged: event \(events) did not commit within \(Int(frameWaitS)) s") }
            var out: [String: Double] = [:]
            for p in Phase.allCases { out[p.name] = phaseMs[p.rawValue] }
            return ["phases": out, "event": events]
        }
        let v = kind == "frame" ? frameMs : kind == "gap" ? gapMs : commitMs
        guard let ms = v else { throw ControlFailure("not judged: the \(kind) of event \(events) did not arrive within \(Int(frameWaitS)) s") }
        return ["ms": ms, "event": events]
    }

    /// DEBUG only, for the check that proves the measures track a slow frame: WHOOSH_DEBUG_SLOW_FRAME_MS=<ms> sleeps that
    /// long in the main actor, after the state change, on every second event (the 2nd, 4th, ...).
    private static func debugSlow() {
        #if DEBUG
        guard events % 2 == 0, let s = ProcessInfo.processInfo.environment["WHOOSH_DEBUG_SLOW_FRAME_MS"], let ms = Double(s), ms > 0 else { return }
        Thread.sleep(forTimeInterval: ms / 1000)
        #endif
    }
}

#if os(macOS)
@MainActor
private final class Ticker: NSObject {
    var armed: (gen: Int, commit: CFTimeInterval) = (0, 0)
    @objc func tick(_ l: CADisplayLink) { InteractionProbe.ticked(l, gen: armed.gen, after: armed.commit) }
}
#endif
