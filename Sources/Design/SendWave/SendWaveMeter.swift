import QuartzCore
#if os(macOS)
import AppKit

/// What one wave costs, measured while it runs: the display link's frame intervals (a dropped frame is an
/// interval over 1.5 nominal frames) and every main run-loop turn's busy time (a turn at FRAME_MS or longer is
/// a frame the owner can see; the same rule as Sources/Features/InboxList/ListPerf.swift). The link also ends
/// the wave when SENDWAVE_MS has passed.
@MainActor
final class SendWaveMeter: NSObject {
    private var link: CADisplayLink?
    private var lastTick: CFTimeInterval = 0
    private let pane: String
    private let id: Int
    private var started: CFTimeInterval?
    private let triggered: CFTimeInterval
    private let onTick: @MainActor () -> Void
    private let onEnd: @MainActor () -> Void

    private let duration: Double

    init(pane: String, id: Int, triggered: CFTimeInterval, duration: Double = SendWaveTuning.durationS,
         onTick: @escaping @MainActor () -> Void, onEnd: @escaping @MainActor () -> Void) {
        self.pane = pane; self.id = id; self.triggered = triggered; self.duration = duration; self.onTick = onTick; self.onEnd = onEnd
    }

    /// The wave's clock starts when its first picture is on screen.
    func begin(at t: CFTimeInterval) { started = t; SendWaveTurns.start() }

    func start(in window: NSWindow) {
        let l = window.displayLink(target: self, selector: #selector(tick(_:)))
        l.add(to: .main, forMode: .common)
        link = l
        SendWaveTurns.install()
    }

    func stop() {
        link?.invalidate(); link = nil
        guard started != nil else { return }
        let (maxMs, over, at) = SendWaveTurns.stop()
        SendWaveCenter.shared.update(pane, id: id) { r in
            r.turns = SendWaveTurns.count
            r.maxBusyAtMs = at
            r.maxBusyMs = maxMs
            r.overBusy = over
        }
    }

    @objc private func tick(_ l: CADisplayLink) {
        let nominal = (l.targetTimestamp - l.timestamp) * 1000
        let interval = lastTick > 0 && started != nil ? (l.timestamp - lastTick) * 1000 : 0
        lastTick = l.timestamp
        onTick()
        guard let started else { return }
        SendWaveCenter.shared.update(pane, id: id) { r in
            r.frames += 1
            r.nominalFrameMs = nominal
            if r.firstFrameMs == nil { r.firstFrameMs = (l.targetTimestamp - triggered) * 1000 }
            if interval > 0 {
                r.maxFrameMs = max(r.maxFrameMs, interval)
                if interval > nominal * 1.5 { r.dropped += 1 }
            }
        }
        if l.timestamp - started >= duration { stop(); onEnd() }
    }
}
#endif

/// Main run-loop turn lengths while a wave runs (the recorder of Sources/Features/InboxList/ListPerf.swift, one
/// observer installed once, recording only between start and stop).
enum SendWaveTurns {
    nonisolated(unsafe) private static var observer: CFRunLoopObserver?
    nonisolated(unsafe) private static var began: CFAbsoluteTime = 0
    nonisolated(unsafe) private static var recording = false
    nonisolated(unsafe) private static var turns: [Double] = []
    nonisolated(unsafe) private static var ends: [CFAbsoluteTime] = []
    nonisolated(unsafe) private static var startedAt: CFAbsoluteTime = 0
    static var count: Int { turns.count }

    static func install() {
        guard observer == nil else { return }
        let activities = CFRunLoopActivity.afterWaiting.rawValue | CFRunLoopActivity.beforeWaiting.rawValue
        let o = CFRunLoopObserverCreateWithHandler(nil, activities, true, 0) { _, activity in
            if activity == .afterWaiting { began = CFAbsoluteTimeGetCurrent(); return }
            guard recording, began > 0 else { return }
            let now = CFAbsoluteTimeGetCurrent()
            if turns.count < 2000 { turns.append((now - began) * 1000); ends.append(now) }
        }
        observer = o
        CFRunLoopAddObserver(CFRunLoopGetMain(), o, .commonModes)
    }

    static func start() { turns = []; ends = []; startedAt = CFAbsoluteTimeGetCurrent(); recording = true }
    /// The longest turn, the turns at FRAME_MS or longer, and when the longest one ended (ms into the wave).
    static func stop() -> (Double, Int, Double) {
        recording = false
        guard let i = turns.indices.max(by: { turns[$0] < turns[$1] }) else { return (0, 0, 0) }
        return (turns[i], turns.filter { $0 >= SendWaveTuning.frameMs }.count, (ends[i] - startedAt) * 1000)
    }
}
