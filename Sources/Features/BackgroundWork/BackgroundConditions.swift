import Foundation
#if os(macOS)
import IOKit.ps
#endif

/// Tells the engine about the battery, the charger and the heat: once at start, then on each change the system announces
/// (a power-source notification, `ProcessInfo.thermalStateDidChangeNotification`). It never polls: a change of charge that the
/// gate holds back is sent by ONE timer set for the end of the minute (ConditionsGate), and nothing runs while nothing changes.
/// All state lives on one utility queue, so the main thread never waits on it.
public final class BackgroundConditions: @unchecked Sendable {
    private let queue = DispatchQueue(label: "whoosh.background.conditions", qos: .utility)
    private let engine: any BackgroundEngine
    private let state: BackgroundState
    private let system: any PowerReading
    private let clock: @Sendable () -> TimeInterval
    private var gate: ConditionsGate
    private var scripted: (any PowerReading)?
    private var pending: DispatchWorkItem?
    private var observing = false

    public init(engine: any BackgroundEngine, state: BackgroundState = .shared, system: any PowerReading = SystemPower(),
                minInterval: TimeInterval = BackgroundWorkConstants.CONDITIONS_MIN_INTERVAL_S,
                floor: Int = WhooshConstants.ATTACHMENT_INDEX_BATTERY_MIN,
                clock: @escaping @Sendable () -> TimeInterval = { ProcessInfo.processInfo.systemUptime }) {
        self.engine = engine
        self.state = state
        self.system = system
        self.clock = clock
        self.gate = ConditionsGate(minInterval: minInterval, floor: floor)
    }

    /// Reads once, tells the engine, and listens. Safe to call twice.
    public func start() {
        queue.async { [self] in
            if !observing {
                observing = true
                observe()
            }
            evaluate()
        }
    }

    /// `fixture power`: use this reading instead of the system's (nil = the system's again). It goes through the same gate.
    public func script(_ reading: PowerConditions?) {
        queue.async { [self] in
            scripted = reading.map { ScriptedPower(conditions: $0) }
            evaluate()
        }
    }

    /// Something may have changed: read again on the queue.
    func changed() { queue.async { [self] in evaluate() } }

    private func observe() {
        // Heat: no queue and no Notification passed on, so nothing crosses an actor.
        NotificationCenter.default.addObserver(forName: ProcessInfo.thermalStateDidChangeNotification, object: nil, queue: nil) { [weak self] _ in
            self?.changed()
        }
        #if os(macOS)
        // Battery and charger: the system's own change source on the main run loop; the callback only hops to the queue.
        let context = Unmanaged.passUnretained(self).toOpaque()
        if let source = IOPSNotificationCreateRunLoopSource({ raw in
            guard let raw else { return }
            Unmanaged<BackgroundConditions>.fromOpaque(raw).takeUnretainedValue().changed()
        }, context)?.takeRetainedValue() {
            CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        }
        #endif
    }

    /// Runs on `queue` only.
    private func evaluate() {
        let reading = (scripted ?? system).read()
        let now = clock()
        state.update { $0.reading = reading; $0.source = scripted == nil ? "real" : "fixture" }
        switch gate.decide(reading, at: now) {
        case .send:
            pending?.cancel(); pending = nil
            engine.setConditions(batteryPercent: reading.batteryPercent, charging: reading.charging, thermalSerious: reading.thermalSerious)
            gate.sent(reading, at: now)
            state.update { $0.sent = reading; $0.sends += 1; $0.pendingMs = nil }
        case .skip:
            pending?.cancel(); pending = nil
            state.update { $0.pendingMs = nil }
        case .later(let seconds):
            state.update { $0.pendingMs = Int(seconds * 1000) }
            guard pending == nil else { return }
            let item = DispatchWorkItem { [weak self] in self?.pending = nil; self?.evaluate() }
            pending = item
            queue.asyncAfter(deadline: .now() + seconds, execute: item)
        }
    }
}
