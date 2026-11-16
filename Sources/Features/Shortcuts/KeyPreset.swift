import Foundation

/// One chord bound to one command id (see Commands.swift). A chord is written the way the socket `key` verb
/// writes it: "E", "Enter", "Tab", "Shift+Tab", "⌘K", "⇧⌘P", "⇧?". Modifiers come in the order ⌃ ⌥ ⇧ ⌘.
public struct KeyBinding: Sendable, Equatable {
    public let chord: String
    public let command: String
    public init(_ chord: String, _ command: String) { self.chord = chord; self.command = command }
}

/// Two keys pressed one after the other: G then I, 0 then 0. The second key must come within SEQUENCE_WAIT_MS
/// (docs/CONSTANTS.md) of the first. A symbol second key is written the way ChordKit writes it ("⇧!").
public struct KeySequence: Sendable, Equatable {
    public let first: String
    public let second: String
    public let command: String
    public init(_ first: String, _ second: String, _ command: String) { self.first = first; self.second = second; self.command = command }
    /// The way Superhuman writes it: "G then !".
    public var label: String { first + " then " + (second.hasPrefix("⇧") && second.count == 2 ? String(second.dropFirst()) : second) }
}

public extension KeyBinding {
    /// One chord per digit 1 to 9 ("⌃1" ... "⌃9"), each bound to `prefix` plus the digit ("switch-account-3").
    static func digits(_ modifier: String, _ prefix: String) -> [KeyBinding] {
        (1...9).map { KeyBinding("\(modifier)\($0)", "\(prefix)-\($0)") }
    }
}

/// A named key map. Bindings stay an ordered list so a duplicate (a chord bound twice, or a command bound to
/// two chords) is visible instead of being swallowed by a dictionary.
public struct KeyPreset: Sendable, Equatable, Identifiable {
    public let id: String
    public let name: String
    public let bindings: [KeyBinding]
    /// Second ways into a command that already has its row key (the "/" that also opens the search). They
    /// dispatch like any key, show in no row, and never count as the command being bound twice.
    public let alsoKeys: [KeyBinding]
    /// Second meanings of a chord that a row already binds (Superhuman uses Tab, Esc, ⌘K, ⌘O, ⌘U, ⇧R, P, N and ⇧⌘A
    /// more than once, one meaning per place). The chord tries these first, then its row; a handler that says "not
    /// here" passes the key on. A coming-soon command always goes last.
    public let contextKeys: [KeyBinding]
    /// Two-key sequences (G then I). Their first key waits for the second.
    public let sequences: [KeySequence]

    public init(id: String, name: String, bindings: [KeyBinding], alsoKeys: [KeyBinding] = [],
                contextKeys: [KeyBinding] = [], sequences: [KeySequence] = []) {
        self.id = id; self.name = name; self.bindings = bindings; self.alsoKeys = alsoKeys
        self.contextKeys = contextKeys; self.sequences = sequences
    }

    public var map: [String: String] {
        Dictionary((bindings + alsoKeys).map { ($0.chord, $0.command) }, uniquingKeysWith: { first, _ in first })
    }

    /// The commands a chord tries, in order: its context meanings, its row, then any coming-soon meaning.
    public func commands(forChord chord: String, fixed: [KeyBinding] = Presets.fixed) -> [String] {
        let contexts = contextKeys.filter { $0.chord == chord }.map(\.command)
        let row = map[chord] ?? fixed.first { $0.chord == chord }?.command
        let all = contexts + (row.map { [$0] } ?? [])
        let live = all.filter { Commands.command($0)?.comingSoon == nil }
        return live + all.filter { Commands.command($0)?.comingSoon != nil }
    }

    /// Every chord this preset reacts to, sequences' two keys included.
    public var allChords: Set<String> {
        var s = Set((bindings + alsoKeys + contextKeys).map(\.chord))
        for q in sequences { s.insert(q.first); s.insert(q.second) }
        return s
    }

    public func chord(for command: String) -> String? { bindings.first { $0.command == command }?.chord }

    /// Every way into a command, in the order the key map lists them: a row, a second key, a context meaning, a
    /// sequence, then the fixed keys (↑ ↓ and Shift+Tab work in every preset, so the map shows them too).
    public var ways: [(kind: String, key: String, command: String)] {
        bindings.map { ("row", $0.chord, $0.command) } + alsoKeys.map { ("also", $0.chord, $0.command) }
            + contextKeys.map { ("context", $0.chord, $0.command) } + sequences.map { ("sequence", $0.label, $0.command) }
            + Presets.fixed.map { ("fixed", $0.chord, $0.command) }
    }

    /// Chords bound twice and commands reached by two chords. The acceptance bar is an empty list.
    public var duplicates: [String] {
        var seenChord = Set<String>(), seenCommand = Set<String>(), out: [String] = []
        for b in bindings {
            if !seenChord.insert(b.chord).inserted { out.append("chord \(b.chord)") }
            if !seenCommand.insert(b.command).inserted { out.append("command \(b.command)") }
        }
        for b in alsoKeys where !seenChord.insert(b.chord).inserted { out.append("chord \(b.chord)") }
        for b in contextKeys where !seenChord.contains(b.chord) && !Presets.fixed.contains(where: { $0.chord == b.chord }) { out.append("context chord \(b.chord) has no row") }
        var seenSequence = Set<String>()
        for q in sequences where !seenSequence.insert(q.first + q.second).inserted { out.append("sequence \(q.label)") }
        return out
    }

    /// Commands bound here that no handler can run (a typo in a preset). The acceptance bar is an empty list.
    public var unregistered: [String] {
        (bindings + alsoKeys + contextKeys).map(\.command).filter { Commands.title($0) == nil }
            + sequences.map(\.command).filter { Commands.title($0) == nil }
    }

    /// A copy where each command in `changes` moves to its new chord. The command's old chord is freed, and
    /// whatever sat on the new chord is unbound. Changes apply in command-id order so the result is stable.
    public func rebinding(_ changes: [String: String], id: String, name: String) -> KeyPreset {
        var list = bindings
        for command in changes.keys.sorted() {
            guard let chord = changes[command] else { continue }
            list.removeAll { $0.command == command || $0.chord == chord }
            list.append(KeyBinding(chord, command))
        }
        let moved = Set(changes.keys), taken = Set(changes.values)
        let also = alsoKeys.filter { !moved.contains($0.command) && !taken.contains($0.chord) }
        let contexts = contextKeys.filter { !moved.contains($0.command) }
        let seqs = sequences.filter { !taken.contains($0.first) }
        return KeyPreset(id: id, name: name, bindings: list, alsoKeys: also, contextKeys: contexts, sequences: seqs)
    }
}

public enum Presets {
    public static let customId = "custom"

    /// The default: Superhuman's v8 key list, key for key (docs/reference/superhuman-shortcuts-v8.json; the owner,
    /// 2026-11-08 10:24 and 10:29: "replicated exactly", "Superhuman's meaning, no second ways"). Two-modifier chords
    /// are allowed here, the copied-preset exception. One line per Superhuman section. A chord is written the way
    /// ChordKit writes a real key: "⇧#" and "⇧!" for the symbols, "Shift+Tab", "⇧⌘A" (⌃ ⌥ ⇧ ⌘ order).
    /// ⌘P (the palette) is the one Whoosh key v8 does not list. A, M and ⌘J are gone; V moves, N and P step through
    /// a thread, ⇧P pins (see the pin row), ⌃1 to ⌃9 switch accounts. checks/superhuman-v8-check.py reads this block.
    public static let superhuman = KeyPreset(id: "superhuman", name: "Superhuman",
    bindings: superhumanRows + KeyBinding.digits("⌃", "switch-account") + KeyBinding.digits("⌘", "switch-tab"),
    alsoKeys: [KeyBinding("/", "search"), KeyBinding("⇧?", "search")],
    contextKeys: [
        // ⇧I is the list's "important" filter (row, v8 Filters) and, while the READER pane has the focus, the colour switch of the open mail (owner: keys are contextual).
        KeyBinding("⇧I", "toggle-colours"),
        // ⇧R is in v8 twice (Filters "No Reply" entry 35, Pop Out Compose "Pop Out Reply" entry 107): one chord, one meaning per place.
        // popout-reply answers where a reply exists (thread open, quick reply up, composer); from the plain list MacShellLinks.shiftR
        // (MacShellCommands.swift) resolves it to the row key below, filter-no-reply.
        KeyBinding("Escape", "clear-selection"), KeyBinding("⇧⌘A", "attach"), KeyBinding("Tab", "cycle-links"), KeyBinding("Tab", "indent-list"),
        KeyBinding("Shift+Tab", "cycle-links-back"), KeyBinding("Shift+Tab", "outdent-list"), KeyBinding("⌘K", "hyperlink"), KeyBinding("⌘O", "color"), KeyBinding("⌘U", "underline"),
        KeyBinding("⇧R", "popout-reply"), KeyBinding("P", "cal-previous"), KeyBinding("N", "cal-next"),
    ],
    sequences: [
        KeySequence("G", "I", "go-inbox"), KeySequence("G", "O", "go-other"), KeySequence("G", "S", "go-starred"),
        KeySequence("G", "D", "go-drafts"), KeySequence("G", "T", "go-sent"), KeySequence("G", "E", "go-done"),
        KeySequence("G", "H", "go-reminders"), KeySequence("G", "M", "go-muted"), KeySequence("G", ";", "go-snippets"),
        KeySequence("G", "⇧!", "go-spam"), KeySequence("G", "⇧#", "go-trash"), KeySequence("G", "A", "go-all-mail"),
        KeySequence("G", "L", "go-label"), KeySequence("0", "0", "cal-week"),
    ])

    private static let superhumanRows: [KeyBinding] = [
        // Actions
        KeyBinding("⌘K", "search"), KeyBinding("Z", "undo"), KeyBinding("X", "select-conversation"),
        KeyBinding("⇧J", "extend-selection-down"), KeyBinding("⇧K", "extend-selection-up"), KeyBinding("Escape", "back"),
        KeyBinding("⌘A", "select-from-here"), KeyBinding("⇧⌘A", "select-all"),
        // Conversations
        KeyBinding("E", "archive"), KeyBinding("⇧E", "mark-not-done"), KeyBinding("H", "snooze"), KeyBinding("S", "star"),
        KeyBinding("U", "unread"), KeyBinding("⇧#", "delete"), KeyBinding("⇧!", "junk"), KeyBinding("⇧M", "mute"),
        KeyBinding("⌘U", "unsubscribe"),
        // Messages
        // Enter: Superhuman's list says "open" in the list and "reply all" in a thread; the owner's word overrides the list view only
        // (2026-11-10, A102): from the plain list Enter does what R does (opens the thread and the quick reply). CommandRegistry.enter picks.
        KeyBinding("C", "compose"), KeyBinding("Enter", "enter"), KeyBinding("R", "reply"), KeyBinding("F", "forward"),
        KeyBinding("⌘O", "open-links"), KeyBinding("O", "expand-message"), KeyBinding("⇧O", "expand-all-messages"),
        // ⇧H: Superhuman's "Expand / Collapse Message Header" (its in-app tooltip in the owner's screenshots, 2026-11-08
        // 22:00; SPEC A70). The v8 list has no row for it; its nearest row is "Expand Message" (O, entry 24).
        KeyBinding("⇧H", "toggle-message-header"),
        // Labels
        KeyBinding("L", "label"), KeyBinding("Y", "remove-label"), KeyBinding("[", "remove-label-next"),
        KeyBinding("]", "remove-label-previous"), KeyBinding("⇧Y", "remove-all-labels"), KeyBinding("V", "move"),
        // Filters
        KeyBinding("⇧U", "filter-unread"), KeyBinding("⇧S", "filter-starred"), KeyBinding("⇧I", "filter-important"),
        KeyBinding("⇧R", "filter-no-reply"),
        // Calendar (coming soon)
        KeyBinding("0", "cal-day"), KeyBinding("T", "cal-today"), KeyBinding("-", "cal-previous"), KeyBinding("=", "cal-next"),
        KeyBinding("B", "cal-create-event"),
        // Format
        KeyBinding("⌘B", "bold"), KeyBinding("⌘I", "italic"), KeyBinding("⇧⌘X", "strikethrough"), KeyBinding("⇧⌘7", "numbered-list"),
        KeyBinding("⇧⌘8", "bullet-list"), KeyBinding("⇧⌘9", "quote"), KeyBinding("⌘]", "increase-indent"), KeyBinding("⌘[", "decrease-indent"),
        // Compose
        KeyBinding("⇧⌘O", "compose-to"), KeyBinding("⇧⌘C", "compose-cc"), KeyBinding("⇧⌘B", "compose-bcc"), KeyBinding("⇧⌘F", "compose-from"),
        KeyBinding("⇧⌘S", "edit-subject"), KeyBinding("⇧⌘M", "edit-message"), KeyBinding("⇧⌘,", "discard-draft"),
        KeyBinding("⇧⌘I", "move-contacts-to-bcc"), KeyBinding("⇧⌘H", "remind-me"), KeyBinding("⇧⌘L", "send-later"),
        KeyBinding(";", "snippet-inline"), KeyBinding("⌘;", "use-snippet"), KeyBinding("⌘Enter", "send"),
        KeyBinding("⇧⌘Enter", "send-and-done"), KeyBinding("⇧⌘Z", "instant-send"),
        // Navigation
        KeyBinding("Left", "focus-left"), KeyBinding("Right", "focus-right"), KeyBinding("Tab", "next-split"),
        KeyBinding("J", "next"), KeyBinding("K", "previous"), KeyBinding("N", "next-message"), KeyBinding("P", "previous-message"),
        KeyBinding("Space", "scroll-down"), KeyBinding("⇧Space", "scroll-up"), KeyBinding("⌘Up", "jump-top"), KeyBinding("⌘Down", "jump-bottom"),
        KeyBinding("⌘N", "new-window"), KeyBinding("⌘T", "new-tab"), KeyBinding("⇧⌘]", "next-tab"), KeyBinding("⇧⌘[", "previous-tab"),
        KeyBinding("⌘W", "close-tab"), KeyBinding("⌘=", "font-bigger"), KeyBinding("⌘-", "font-smaller"), KeyBinding("⌘0", "font-reset"),
        KeyBinding("⌃/", "copy-page-link"),
        // Pop out compose
        KeyBinding("⇧⌘P", "popout-draft"), KeyBinding("⇧Enter", "popout-reply-all"), KeyBinding("⇧C", "popout-new"),
        KeyBinding("⇧F", "popout-forward"), KeyBinding("⌘/", "popout-draft-search"), KeyBinding("⌘D", "switch-draft"),
        // Whoosh's own, not in v8: the palette and the pin that gave up P. The key list is reached from the palette.
        KeyBinding("⌘P", "palette"), KeyBinding("⇧P", "pin"), KeyBinding("⌘\\", "sidebar"), KeyBinding("⌘J", "ai-reply"),
    ]

    /// GotEmail: ⌘K opens search and ⇧⌘P opens the palette (docs/JOURNEYS.md, J1.9 and J2.1). A copied key, the
    /// allowed exception to "one modifier, one key". "/" is not a second way into the search here.
    public static let gotEmail = superhuman.rebinding(["palette": "⇧⌘P", "search": "⌘K"], id: "gotemail", name: "GotEmail")

    /// Gmail's letters where they differ from Superhuman: B snooze, V move, ⇧! spam, ⇧U unread. No pin key.
    public static let gmail = KeyPreset(id: "gmail", name: "Gmail", bindings: [
        KeyBinding("J", "next"), KeyBinding("K", "previous"), KeyBinding("Enter", "open"),
        KeyBinding("C", "compose"), KeyBinding("R", "reply"), KeyBinding("A", "reply-all"), KeyBinding("F", "forward"),
        KeyBinding("E", "archive"), KeyBinding("⇧#", "delete"), KeyBinding("⇧!", "junk"),
        KeyBinding("B", "snooze"), KeyBinding("L", "label"), KeyBinding("V", "move"),
        KeyBinding("S", "star"), KeyBinding("⇧U", "unread"), KeyBinding("Z", "undo"),
        KeyBinding("⌘K", "search"), KeyBinding("⌘P", "palette"), KeyBinding("⌘J", "accounts"), KeyBinding("⌘\\", "sidebar"),
        KeyBinding("Tab", "next-split"), KeyBinding("⇧?", "keys"),
    ], alsoKeys: [KeyBinding("/", "search")],
    // ⌘J writes the AI reply while a thread is open (reply-thread); elsewhere the ai-reply handler says "not here" and ⌘J opens accounts.
    contextKeys: [KeyBinding("⌘J", "ai-reply")])

    /// Apple Mail's own menu keys. Mail has no pin, snooze or label key, so those stay unbound here.
    public static let appleMail = KeyPreset(id: "applemail", name: "Apple Mail", bindings: [
        KeyBinding("⌘]", "next"), KeyBinding("⌘[", "previous"), KeyBinding("⌃⌘A", "archive"),
        KeyBinding("⌘N", "compose"), KeyBinding("Enter", "open"), KeyBinding("⌘R", "reply"),
        KeyBinding("⌘Z", "undo"), KeyBinding("⌥⌘F", "search"), KeyBinding("⌃Tab", "next-split"),
        KeyBinding("⌘/", "keys"), KeyBinding("⌘P", "palette"), KeyBinding("⌘J", "accounts"),
    ], alsoKeys: [KeyBinding("⌘K", "search")], contextKeys: [KeyBinding("⌘J", "ai-reply")])   // as in Gmail: ⌘J writes in an open thread

    /// The four built-ins; "your own" is derived from the owner's changes (ShortcutEngine.customPreset).
    public static let builtIn: [KeyPreset] = [superhuman, gmail, appleMail, gotEmail]

    /// Keys that work the same in every preset. They are not part of a preset's row list.
    public static let fixed: [KeyBinding] = [
        KeyBinding("Shift+Tab", "previous-split"), KeyBinding("Down", "next"), KeyBinding("Up", "previous"),
    ]

    public static func named(_ id: String) -> KeyPreset? { builtIn.first { $0.id == id } }

    /// Every key a person can press with the modifiers ChordKit writes (⌃ ⌥ ⇧ ⌘ order, never the reverse). The feature
    /// registers them all so the engine hears a key no preset binds: a sequence waiting for its second key must end on it
    /// (G then Q drops the G), and an unbound key still gets refused.
    public static let pressable: Set<String> = {
        let keys = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789`-=[]\\;',./".map(String.init)
            + ["Enter", "Escape", "Tab", "Space", "Left", "Right", "Up", "Down", "Backspace"]
        let mods = ["", "⇧", "⌘", "⌃", "⌥", "⇧⌘", "⌥⌘", "⌃⌘", "⌃⇧", "⌥⇧"]
        return Set(mods.flatMap { m in keys.map { m + $0 } })
    }()
}
