import Foundation
import Observation

/// The keyboard engine every Mac screen dispatches through: chord -> command id (by the chosen preset) ->
/// handler (CommandRegistry). A key and a mouse action both end in `run`, so both are counted the same way.
@MainActor @Observable
public final class ShortcutEngine {
    public static let shared = ShortcutEngine()
    /// The engine whose key is running a handler right now (the shared one outside a test): a command that picks
    /// another command by the app state reports its pick here.
    public private(set) static var current = shared

    @ObservationIgnored public var store: ShortcutStore
    /// bumped on every preset change so a view that reads `preset` redraws
    public private(set) var revision = 0

    public init(store: ShortcutStore = MemoryShortcutStore()) { self.store = store }

    /// What one key did, for the rows `mac-shortcuts.dispatch` and the checks: the chord, the command it reached,
    /// and what came of it. `ran` a handler took it, `passed` a stub or a handler said "not here", `notice` a
    /// coming-soon command showed its line, `armed` the first key of a sequence waits, `lapsed` and `cancelled`
    /// end a sequence without a command.
    public struct Dispatch: Sendable, Equatable {
        /// counts every record since launch, so a reader can ask for "the rows after n" past the 64-row window
        public let n: Int
        public let chord: String
        public let command: String
        public let resolved: String
        public let outcome: String
    }

    public private(set) var log: [Dispatch] = []
    /// The one line a coming-soon key shows ("Calendar is coming soon."), cleared by the next key.
    public private(set) var notice: String?
    /// Set by a command that picks another command by the app state (Enter: "open" or "reply-all"); read once.
    @ObservationIgnored public var resolved: String?
    /// The clock the sequence wait reads: seconds since boot. A test replaces it.
    @ObservationIgnored public var now: () -> TimeInterval = { ProcessInfo.processInfo.systemUptime }
    @ObservationIgnored private var pending: (first: String, at: TimeInterval)?
    @ObservationIgnored private var serial = 0
    private static let logLimit = 64

    // MARK: presets

    public var presetId: String { store.presetId }

    /// "your own": the Superhuman map with the owner's changes laid over it.
    public var customPreset: KeyPreset {
        Presets.superhuman.rebinding(store.overrides, id: Presets.customId, name: "Your own")
    }

    public var allPresets: [KeyPreset] { Presets.builtIn + [customPreset] }

    public func named(_ id: String) -> KeyPreset? {
        id == Presets.customId ? customPreset : Presets.named(id)
    }

    /// The map in force. An unknown stored id falls back to the default rather than leaving no keys.
    public var preset: KeyPreset { named(store.presetId) ?? Presets.superhuman }

    @discardableResult
    public func setPreset(_ id: String) -> Bool {
        guard named(id) != nil else { return false }
        store.presetId = id
        revision += 1
        return true
    }

    /// Moves one command to a chord in the custom preset and switches to it.
    @discardableResult
    public func rebind(_ command: String, to chord: String) -> Bool {
        guard Commands.title(command) != nil, !chord.isEmpty else { return false }
        store.overrides[command] = chord
        return setPreset(Presets.customId)
    }

    // MARK: lookup

    public func command(forChord chord: String) -> String? { commands(forChord: chord).first }

    /// Every command a chord tries, in order (context meanings, the row, coming soon last).
    public func commands(forChord chord: String) -> [String] { preset.commands(forChord: chord) }

    /// The key to show beside a command in the palette, the trainer and the show-keys overlay.
    public func chord(for command: String) -> String? { preset.chord(for: command) }

    /// Every chord any preset, the custom map or a fixed key can use. The feature registers each one; the
    /// preset in force decides what it does, so a preset switch needs no re-registration.
    public var knownChords: Set<String> {
        var s = Set(Presets.fixed.map(\.chord))
        for p in allPresets { s.formUnion(p.allChords) }
        return s
    }

    // MARK: dispatch

    /// The real keyboard and the socket `key` verb arrive here (through Registrations -> AppModel.press).
    /// A sequence's first key waits (and the key is taken); the second key within SEQUENCE_WAIT_MS runs the
    /// sequence's command. Any other second key, or a late one, drops the sequence and is read as a fresh key.
    public func press(_ chord: String, _ model: AppModel) -> Bool {
        notice = nil
        if let wait = pending {
            pending = nil
            if (now() - wait.at) * 1000 <= Double(WhooshConstants.SEQUENCE_WAIT_MS) {
                if let q = preset.sequences.first(where: { $0.first == wait.first && $0.second == chord }) {
                    // The second key is the sequence's, whatever its command did: a key that mailbox navigation has not
                    // claimed yet must not leak to the single-key command on the same letter (G then S is not star).
                    _ = dispatch(q.command, chord: q.label, model)
                    return true
                }
                add(chord, "-", "-", "cancelled")
                if chord == "Escape" { return true }
            } else {
                add(wait.first, "-", "-", "lapsed")
            }
        }
        let armed = preset.sequences.contains { $0.first == chord }
        if armed {
            pending = (chord, now())
            add(chord, "-", "-", "armed")
        }
        for command in commands(forChord: chord) where dispatch(command, chord: chord, model) { return true }
        return armed
    }

    /// A click on an action's control. The control calls this so the mouse use is counted and the key hint can follow.
    public func click(_ command: String, _ model: AppModel) -> Bool {
        run(command, model, key: false)
    }

    /// One command reached by a key: recorded, then run. A coming-soon command shows its notice and passes the key on.
    private func dispatch(_ command: String, chord: String, _ model: AppModel) -> Bool {
        resolved = nil
        Self.current = self
        defer { Self.current = .shared }
        if let line = Commands.command(command)?.notice {
            notice = line
            add(chord, command, command, "notice")
            return false
        }
        let took = run(command, model, key: true)
        add(chord, command, resolved ?? command, took ? "ran" : "passed")
        return took
    }

    private func add(_ chord: String, _ command: String, _ resolved: String, _ outcome: String) {
        serial += 1
        log.append(Dispatch(n: serial, chord: chord, command: command, resolved: resolved, outcome: outcome))
        if log.count > Self.logLimit { log.removeFirst(log.count - Self.logLimit) }
    }

    /// True while a thread is on screen with nothing laid over it: Enter then means reply all, not open.
    public static func threadIsOpen(_ m: AppModel) -> Bool {
        let thread = m.surface == "mac-reader" || m.overlay == "mac-thread"
        return thread && (m.overlay == nil || m.overlay == "mac-thread")
    }

    private func run(_ command: String, _ model: AppModel, key: Bool) -> Bool {
        BackgroundWork.noteOwnerActivity()   // bg-hooks: the owner acted; the background download stays quiet for a moment
        if let line = Commands.command(command)?.notice { notice = line; return false }
        guard let handler = CommandRegistry.handler(command), handler(model) else { return false }
        store.bump(action: command, key: key)
        return true
    }

    // MARK: records the trainer reads

    public func keyUses(_ command: String) -> Int { store.usage()[command]?.key ?? 0 }
    public func mouseUses(_ command: String) -> Int { store.usage()[command]?.mouse ?? 0 }
}
