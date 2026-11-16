import Foundation
import Observation

/// One hint on screen: the command the mouse just ran, the key that does it, and how many key uses of it the
/// store held when the hint appeared. The hint is visible only while that count has not moved (the owner pressing
/// the key is the answer to the hint), so the store is the one place that says "learned".
public struct TrainerHint: Hashable, Sendable {
    public let command: String
    public let chord: String
    public let verb: String
    public let usesAtShow: Int
    public var text: String { "Next time, press \(chord) to \(verb)" }
}

/// What the trainer remembers between a mouse action and the next key. Counts live in the shortcuts lane's
/// usage store (ShortcutEngine); this model keeps only the hint on screen and the hints switch.
@MainActor @Observable
public final class TrainerModel {
    public static let shared = TrainerModel()

    /// The hints switch. The Settings lane flips this same value; it defaults to on.
    public static let hintsKey = "whoosh.trainer.hints"

    public private(set) var shown: TrainerHint?
    public var hintsOn: Bool = (UserDefaults.standard.object(forKey: TrainerModel.hintsKey) as? Bool) ?? true {
        didSet {
            UserDefaults.standard.set(hintsOn, forKey: Self.hintsKey)
            if !hintsOn { shown = nil }
        }
    }

    @ObservationIgnored var engine: ShortcutEngine { .shared }

    /// `fixture reset` promises a clean screen (FixtureEvents.reset: the screens clear their own state on the notification): the hint a
    /// mouse action left goes with it, so the next check starts with no hint (sweep-trainer row cases read `before` = 0). The post is
    /// synchronous on the main thread, so the hint is gone before the next socket line runs.
    @ObservationIgnored private var resetToken: NSObjectProtocol?
    init() {
        resetToken = NotificationCenter.default.addObserver(forName: FixtureEvents.name("reset"), object: nil, queue: nil) { [weak self] _ in
            if Thread.isMainThread { MainActor.assumeIsolated { self?.shown = nil } }
            else { DispatchQueue.main.sync { MainActor.assumeIsolated { self?.shown = nil } } }
        }
    }
    static var stopAfter: Int { WhooshConstants.TRAINER_HINT_STOP_AFTER }

    /// The plain word after "press E to": the action as the owner says it.
    static func verb(_ command: String) -> String {
        switch command {
        case "compose": return "write"
        default: return Commands.title(command) ?? command
        }
    }

    /// The mouse ran `command`: count it as a mouse use through the engine, then show the hint when one is due.
    @discardableResult
    public func mouseUsed(_ command: String, _ m: AppModel) -> Bool {
        guard engine.click(command, m) else { return false }
        note(command)
        return true
    }

    /// Decide whether the hint for `command` shows now: hints on, the command has a key in this preset, and the
    /// key has not been used TRAINER_HINT_STOP_AFTER times.
    public func note(_ command: String) {
        let uses = engine.keyUses(command)
        guard hintsOn, let chord = engine.chord(for: command), uses < Self.stopAfter else { shown = nil; return }
        shown = TrainerHint(command: command, chord: chord, verb: Self.verb(command), usesAtShow: uses)
    }

    public func dismiss() { shown = nil }

    /// The hint to draw now, or nil: the key was used since it appeared, the owner turned hints off, or the keys overlay is up.
    public func visibleHint(_ m: AppModel) -> TrainerHint? {
        guard hintsOn, m.overlay != Self.overlayId, let h = shown, engine.keyUses(h.command) == h.usesAtShow else { return nil }
        return h
    }

    // MARK: the keys overlay

    public static let overlayId = "mac-trainer.keys"

    public struct KeyRow: Equatable, Sendable {
        public let command: String
        public let chord: String
        public let action: String
        public let used: Bool
    }

    /// Every key of the preset in force, in its order, with the ones the owner already presses marked.
    public var keyRows: [KeyRow] {
        engine.preset.bindings.compactMap { b in
            guard let title = Commands.title(b.command) else { return nil }
            return KeyRow(command: b.command, chord: b.chord, action: title, used: engine.keyUses(b.command) > 0)
        }
    }

    public func isOpen(_ m: AppModel) -> Bool { m.overlay == Self.overlayId }

    @discardableResult
    public func open(_ m: AppModel) -> Bool {
        shown = nil
        m.surface = "mac-trainer"
        m.overlay = Self.overlayId
        return true
    }

    @discardableResult
    public func close(_ m: AppModel) -> Bool {
        guard isOpen(m) else { return false }
        m.overlay = nil
        m.surface = "mac-main"
        return true
    }

    /// The show-keys key is a switch: a second press puts the overlay away.
    public func toggle(_ m: AppModel) -> Bool { isOpen(m) ? close(m) : open(m) }
}
