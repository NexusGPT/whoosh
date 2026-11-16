import Foundation

/// What the app has told the engine and what it read, kept for the socket row `mac-main.background`. Written from the
/// queues of this folder, read by the row source; one lock, held for a copy.
public final class BackgroundState: @unchecked Sendable {
    public static let shared = BackgroundState()

    public struct Snapshot: Sendable {
        /// The last reading, real or scripted by `fixture power`.
        public var reading: PowerConditions?
        /// "real" (IOKit and ProcessInfo) or "fixture" (`fixture power`).
        public var source = "real"
        /// The last conditions the engine was told.
        public var sent: PowerConditions?
        /// Calls of `set_background_conditions` since launch.
        public var sends = 0
        /// Milliseconds until a held-back change of charge is sent, nil when nothing waits.
        public var pendingMs: Int?
        /// Keys and clicks that acted, and the calls of `note_owner_activity` they became (at most one per OWNER_NOTE_MIN_INTERVAL_S).
        public var ownerNotes = 0
        public var ownerCalls = 0
        /// True while the engine holds the embedding model handed over; hand-overs and take-backs counted since launch.
        public var embedderHanded = false
        /// The weights were looked for at launch and are not on this Mac.
        public var modelMissing = false
        public var handovers = 0
        public var takebacks = 0
    }

    private let lock = NSLock()
    private var value = Snapshot()

    public func snapshot() -> Snapshot { lock.lock(); defer { lock.unlock() }; return value }
    func update(_ change: (inout Snapshot) -> Void) { lock.lock(); change(&value); lock.unlock() }

    /// What the engine says about its own background pause, from the state path `background/pause` (fix-realcore round 13): nil when the core serves nothing at that path
    /// (the test engine), `.some(nil)` while it runs, `.some(reason)` ("held", "battery", "heat", "owner_active") while it is paused. Read when the row is asked, never on a timer.
    public static func enginePause(_ core: WhooshCore) -> String?? {
        guard let raw = core.state(path: "background/pause"),
              let value = try? JSONSerialization.jsonObject(with: Data(raw.utf8), options: [.fragmentsAllowed]) else { return nil }
        if value is NSNull { return .some(nil) }
        return (value as? String).map { .some($0) } ?? nil
    }

    /// The row `mac-main.background`. `crawl` and `reason` are the ENGINE's own answer (`background/pause`: non-null is a pause, the value is the reason) whenever the core serves
    /// it; the cells `sent_*` stay the conditions the app sent. A core that serves no such path (the test engine) falls back to the engine's rule applied to the conditions it was
    /// told (docs/lanes/bg-hooks.md finding 4).
    public func row(floor: Int, enginePause: String?? = nil) -> [String: Any] {
        let s = snapshot()
        let told = s.sent
        let toldHeld = told?.holdsWork(floor: floor) ?? false
        let toldReason: String = {
            guard let t = told, toldHeld else { return "" }
            return t.thermalSerious ? "heat" : "battery"
        }()
        let held: Bool
        let reason: String
        switch enginePause {
        case .some(.some(let r)): held = true; reason = r
        case .some(.none): held = false; reason = ""
        case .none: held = toldHeld; reason = toldReason
        }
        var row: [String: Any] = [
            "source": s.source,
            "battery_percent": s.reading?.batteryPercent ?? -1,
            "charging": s.reading?.charging ?? false,
            "thermal_serious": s.reading?.thermalSerious ?? false,
            "sent": told != nil,
            "sent_battery_percent": told?.batteryPercent ?? -1,
            "sent_charging": told?.charging ?? false,
            "sent_thermal_serious": told?.thermalSerious ?? false,
            "sends": s.sends,
            "crawl": held ? "paused" : "running",
            "reason": reason,
            "crawl_source": enginePause == nil ? "sent" : "engine",
            "owner_notes": s.ownerNotes,
            "owner_calls": s.ownerCalls,
            "embedder": s.modelMissing ? "missing" : (s.embedderHanded ? "handed" : "held"),
            "handovers": s.handovers,
            "takebacks": s.takebacks,
            "floor": floor,
        ]
        if let p = s.pendingMs { row["pending_ms"] = p }
        return row
    }
}
