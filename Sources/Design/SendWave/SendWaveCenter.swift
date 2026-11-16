import SwiftUI
import QuartzCore

/// Where every wave stands, read by the socket region `mac-main.sendwave` (SendWaveFeature). One run per pane:
/// a new send restarts its pane's wave; an input in the window or the send leaving its state ends it at once.
@MainActor @Observable
public final class SendWaveCenter {
    public static let shared = SendWaveCenter()

    public enum State: String, Sendable { case idle, running, done, interrupted, off }

    /// One pane's latest wave, as the region reports it.
    public struct Run: Sendable {
        public var pane: String
        public var id: Int
        public var state: State
        public var scheme: SendWaveScheme
        public var origin: CGPoint
        public var hole: CGRect = .zero
        public var started: CFTimeInterval
        public var ended: CFTimeInterval?
        public var reason: String
        public var snapshotMs: Double = 0
        public var snapshotSource = ""
        public var firstFrameMs: Double?
        public var frames = 0
        public var dropped = 0
        public var maxFrameMs = 0.0
        public var nominalFrameMs = 0.0
        public var maxBusyMs = 0.0
        public var overBusy = 0
        public var turns = 0
        public var windows = 0
        /// "send" (the shockwave) or "landing" (AuroraLanding over a card)
        public var kind = "send"
        public var maxBusyAtMs = 0.0
    }

    public private(set) var runs: [String: Run] = [:]
    private var nextId = 0

    /// Test switches (launch environment): WHOOSH_REDUCE_MOTION=1 stands for the system setting;
    /// WHOOSH_MUTANT_SENDWAVE=never|gate|gate-landing|ignore-motion|freeze-button are par-sendwave's mutants
    /// (gate-landing: the landing holds the main thread for its whole run; the others: the wave never
    /// starts; the wave holds the main thread for its whole run, so the next action waits on it; reduced motion is
    /// ignored; the send button is drawn from the picture instead of live). WHOOSH_SENDWAVE_SCHEME picks a scheme
    /// for the owner's preview strips until Settings carries the choice.
    nonisolated static let env = ProcessInfo.processInfo.environment
    nonisolated static var reduceMotionOverride: Bool { env["WHOOSH_REDUCE_MOTION"] == "1" }
    static var mutant: String { env["WHOOSH_MUTANT_SENDWAVE"] ?? "" }

    /// A send succeeded in `pane`: start (or restart) its wave from `origin` (pane points). Returns the run id,
    /// or nil when reduced motion turns the wave off.
    @discardableResult
    func start(pane: String, origin: CGPoint, hole: CGRect = .zero, reduceMotion: Bool, kind: String = "send") -> Int? {
        nextId += 1
        let now = CACurrentMediaTime()
        var run = Run(pane: pane, id: nextId, state: .running, scheme: SendWaveScheme.current, origin: origin,
                      started: now, ended: nil, reason: "")
        run.hole = hole
        run.kind = kind
        if (reduceMotion || Self.reduceMotionOverride) && Self.mutant != "ignore-motion" {
            run.state = .off; run.ended = now; run.reason = "reduced motion"
            runs[pane] = run
            return nil
        }
        if Self.mutant == "never" { run.state = .idle; run.reason = "mutant never"; runs[pane] = run; return nil }
        runs[pane] = run
        if Self.mutant == "gate", kind == "send" { Thread.sleep(forTimeInterval: SendWaveTuning.durationS) }
        if Self.mutant == "gate-landing", kind == "landing" { Thread.sleep(forTimeInterval: SendWaveTuning.landingDurationS) }
        return nextId
    }

    func update(_ pane: String, id: Int, _ change: (inout Run) -> Void) {
        guard var r = runs[pane], r.id == id else { return }
        change(&r)
        runs[pane] = r
    }

    /// Ends a running wave: `done` when it ran its course, `interrupted` (with why) when an input cut it short.
    func end(_ pane: String, id: Int, interrupted reason: String? = nil) {
        update(pane, id: id) { r in
            guard r.state == .running else { return }
            r.state = reason == nil ? .done : .interrupted
            r.reason = reason ?? ""
            r.ended = CACurrentMediaTime()
        }
    }

    func isRunning(_ pane: String, id: Int) -> Bool { runs[pane].map { $0.id == id && $0.state == .running } ?? false }

    /// The region's rows: one per pane that has waved, newest state first.
    func rows() -> [ControlRow] {
        let now = CACurrentMediaTime()
        return runs.values.sorted { $0.pane < $1.pane }.map { r in
            let elapsed = ((r.ended ?? now) - r.started) * 1000
            var row: ControlRow = [
                "pane": r.pane, "run": r.id, "state": r.state.rawValue, "scheme": r.scheme.rawValue,
                "elapsed_ms": elapsed, "duration_ms": WhooshConstants.SENDWAVE_MS, "reason": r.reason,
                "origin_x": r.origin.x, "origin_y": r.origin.y,
                "live_x": r.hole.minX, "live_y": r.hole.minY, "live_w": r.hole.width, "live_h": r.hole.height,
                "snapshot_ms": r.snapshotMs, "snapshot_source": r.snapshotSource,
                "frames": r.frames, "dropped": r.dropped, "max_frame_ms": r.maxFrameMs,
                "nominal_frame_ms": r.nominalFrameMs, "max_busy_ms": r.maxBusyMs, "over_busy": r.overBusy, "kind": r.kind, "turns": r.turns, "windows": r.windows, "max_busy_at_ms": r.maxBusyAtMs,
                "rings": SendWaveTuning.rings, "channel_offset": SendWaveTuning.channelOffset,
            ]
            if let f = r.firstFrameMs { row["first_frame_ms"] = f }
            return row
        }
    }
}
