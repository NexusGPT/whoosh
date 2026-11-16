import Foundation

/// What the engine's background download and indexing need to know about the Mac: the battery, the charger, the heat.
/// Pure value, no IOKit and no ProcessInfo, so the gate below is read and tested without a Mac's power state.
public struct PowerConditions: Equatable, Sendable {
    /// Charge in percent. A Mac with no battery reads 100.
    public var batteryPercent: Int
    /// True on mains power (a desktop Mac has no battery and counts as plugged in).
    public var charging: Bool
    /// ProcessInfo.thermalState serious or critical.
    public var thermalSerious: Bool

    public init(batteryPercent: Int, charging: Bool, thermalSerious: Bool) {
        self.batteryPercent = batteryPercent
        self.charging = charging
        self.thermalSerious = thermalSerious
    }

    /// The engine's own rule (core/crates/engine/src/crawl.rs `held_by`): serious heat, or battery power under the floor.
    /// `floor` is WhooshConstants.ATTACHMENT_INDEX_BATTERY_MIN. Used here only to tell a change that matters from noise,
    /// and to show the owner's row; the engine decides on its own.
    public func holdsWork(floor: Int) -> Bool {
        thermalSerious || (!charging && batteryPercent < floor)
    }
}

/// Decides when a reading goes to the engine. Power sources notify on every percent and on every estimate of time left;
/// the engine needs a call only when something that changes its behaviour changed, or once a minute for a slow drift.
///
/// Sent at once: the first reading; a change of charger or heat; a reading that crosses the floor (the engine starts or stops work).
/// Sent at most once per `minInterval`: a change of charge that crosses nothing. A suppressed change is not lost: the gate says
/// `.later(seconds)` and the caller reads again after that time. Nothing here polls.
public struct ConditionsGate: Sendable {
    public enum Decision: Equatable, Sendable { case send, skip, later(TimeInterval) }

    public let minInterval: TimeInterval
    public let floor: Int
    public private(set) var lastSent: PowerConditions?
    public private(set) var lastSentAt: TimeInterval?

    /// `minInterval` and `floor` come from the caller (BackgroundWork constants and WhooshConstants.ATTACHMENT_INDEX_BATTERY_MIN).
    public init(minInterval: TimeInterval, floor: Int) {
        self.minInterval = minInterval
        self.floor = floor
    }

    public func decide(_ now: PowerConditions, at time: TimeInterval) -> Decision {
        guard let last = lastSent, let at = lastSentAt else { return .send }
        if now == last { return .skip }
        if now.charging != last.charging || now.thermalSerious != last.thermalSerious { return .send }
        if now.holdsWork(floor: floor) != last.holdsWork(floor: floor) { return .send }
        let waited = time - at
        return waited >= minInterval ? .send : .later(minInterval - waited)
    }

    /// Call after the engine was told.
    public mutating func sent(_ c: PowerConditions, at time: TimeInterval) {
        lastSent = c
        lastSentAt = time
    }
}
