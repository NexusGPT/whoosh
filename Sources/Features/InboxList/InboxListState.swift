import Foundation
import Observation

/// The list's own UI state (the Blast Test: only the list and its probes read it). Server data stays in
/// AppModel and the core; nothing here holds a second copy of a row or a count.
@MainActor @Observable
public final class InboxListState {
    public static let shared = InboxListState()

    /// How the rows are grouped on screen (par-inboxlist: group by sender, by domain).
    public enum Grouping: String, CaseIterable, Sendable { case none, sender, domain }

    /// What the next typed words and Enter answer: a picker the list opened.
    public enum Pending: Equatable, Sendable {
        case snooze(messageIds: [String])
        case label(messageIds: [String])

        /// the capsule's words as a catalogue key and its argument, translated when drawn
        func doneWords(_ typed: String) -> Said {
            switch self {
            case .snooze: return Said("Snoozed until %@", typed.lowercased())
            case .label: return Said("Labelled %@", typed)
            }
        }
    }

    /// message ids chosen with X / Shift+J; empty means "the focused row only"
    public private(set) var selection: [String] = []
    public var grouping: Grouping = .none
    public var compact = false
    public var pending: Pending?
    /// the open filter (⇧U, ⇧S, ⇧I, ⇧R): the rows it keeps show, the chip names it
    var filter: ListFilter?
    /// the unpinned label that filters the list (a sidebar click, `LabelFilter`); nil = none. A pinned label is a tab, not this.
    var labelFilter: String?
    /// the tab the list was on when the label filter opened; it comes back when the filter ends
    var labelReturnTab: String?
    /// the starred ids among the rows, refreshed when the rows change (the star mark and the starred filter read it)
    var starred: Set<String> = []
    /// the message ids of the thread rows whose newest message is the owner's own (ThreadFacts): the reply arrow's rows
    var repliedLast: Set<String> = []
    /// the why tag's reads (WhyTags): message id to the core's answer for the view the list shows; empty while the inspector's switch is off
    var whyReads: [String: WhyRead] = [:]
    var whyScope: String?
    /// how many core reads the tags cost so far and how many loads made them: the probe proves they follow loads, never scrolls
    var whyCoreReads = 0
    var whyLoads = 0
    /// the row whose "Why?" line is open (hover on screen, click on the socket)
    public var whyOpen: String?
    public var hovered: String?
    /// the row the pointer rests on (board message-card-r1 state 7): the pointer stayed HOVER_REVEAL_MS on it, or the rig's `fixture pointer-rest
    /// mac-main.list.row.<n>` put it there (ListPointerRest); the row draws the hover fill and the action strip. nil = no row
    public var rested: String?
    /// the board's marks the contract's row does not carry (states 10, 13, 14), read from the core once per change of the rows (RowMarks):
    /// the muted messages among the rows (`message/<id>/muted`), the VIP senders' addresses (the VIPs view's rows), the snoozed mails' return time
    /// (`snooze/<id>/return_local_time`)
    var muted: Set<String> = []
    var vipSenders: Set<String> = []
    var snoozeTimes: [String: String] = [:]
    /// The undo capsule's state, ONE observed value (perf-archive r4: main97 measured 9.2 ms in say's four separate observed writes;
    /// one value is one change for the views that read it). Read through the four names below, as before.
    struct UndoCapsule: Equatable {
        var said: Said?
        var available = true
        var stamp: ContinuousClock.Instant?
        var gen = 0
    }
    private var capsule = UndoCapsule()
    /// the undo capsule's words while the last action can be undone ("Archived", "Snoozed until tomorrow morning"), kept as
    /// the catalogue key and its arguments so a language switch after the action redraws them in the new language
    public var undoSaid: Said? { capsule.said }
    public var undoWords: String? { undoSaid?.text }
    /// false while the capsule's words say the action has no undo entry (a delete): the Undo button is hidden
    public var undoAvailable: Bool { capsule.available }
    /// how the focus highlight moved last (docs/MOTION.md rule 11), for the `mac-main.list.state` probe
    var focusMotion = SelectionMotion.none
    var focusDistance = 0
    /// false from a focus move until FOCUS_SLIDE_MS after the last one: the focused row's action strip waits for it
    var stripSettled = true
    /// the last focus move landed inside FOCUS_SLIDE_MS of the one before: it jumped
    var focusRapid = false

    private init() {}

    /// How long the capsule stays up, in ms. Proposed ledger row UNDO_CHIP_MS = 6000 (docs/CONSTANTS.md; the owner of the ledger adds it):
    /// before this lane the capsule had no clock at all and stayed until Z or the next action. One number, read by the expiry
    /// below and by the line the capsule draws, so the line ends the moment the capsule leaves.
    nonisolated static let undoChipMs = 6000
    /// When the capsule last came up (the one clock: the expiry and the timer line both start here). nil while no capsule shows.
    private var undoStamp: ContinuousClock.Instant? { capsule.stamp }
    /// Counts capsules: a second action restarts the line because the line is keyed on this.
    public var undoGen: Int { capsule.gen }
    @ObservationIgnored private var undoExpiry: Task<Void, Never>?   // no view reads it: not observed

    /// The fraction of the capsule's life left, 1 at the start to 0 at the end; nil when the capsule shows no timer line
    /// (no capsule, or no Undo button). Read from the stamp, never stored per frame; the probe row field `undo_timer`.
    public var undoTimer: Double? {
        guard undoSaid != nil, undoAvailable, let at = undoStamp else { return nil }
        let used = at.duration(to: .now)
        let ms = Double(used.components.seconds) * 1000 + Double(used.components.attoseconds) / 1e15
        return max(0, min(1, 1 - ms / Double(Self.undoChipMs)))
    }

    /// The capsule's words; `undoable` false hides its Undo button (the action has no undo entry). A new capsule restarts the
    /// clock; the chip leaves when it ends (a later `say` cancels this one's expiry, so a newer capsule is never cut short).
    public func say(_ words: Said?, undoable: Bool = true) {
        let t1 = InteractionProbe.phaseStart()
        undoExpiry?.cancel()
        InteractionProbe.phaseEnd(.sayTask, t1)
        // the same four values as before (words, undoable, the stamp and the generation when words come, the stamp cleared when
        // they go), written as one value
        var next = capsule
        next.said = words; next.available = undoable
        if words != nil { next.stamp = .now; next.gen += 1 } else { next.stamp = nil }
        let t0 = InteractionProbe.phaseStart()
        capsule = next
        InteractionProbe.phaseEnd(.sayWrites, t0)
        guard words != nil else { undoExpiry = nil; return }
        let t3 = InteractionProbe.phaseStart()
        defer { InteractionProbe.phaseEnd(.sayTask, t3) }
        undoExpiry = Task { [weak self] in
            try? await Task.sleep(for: .milliseconds(Self.undoChipMs))
            guard !Task.isCancelled, let self else { return }
            var gone = self.capsule
            gone.said = nil; gone.stamp = nil
            self.capsule = gone
        }
    }
    /// Legacy door for a caller outside this folder that still passes translated text (the reader's delete): kept as its own
    /// key, so it does NOT follow a later language switch. Callers move to `Said`.
    public func say(_ words: String, undoable: Bool = true) { say(Said(words), undoable: undoable) }

    /// The messages this list snoozed (the core has no read of the snoozed list yet): id, sender, subject.
    public private(set) var snoozedHere: [(id: String, sender: String, subject: String)] = []
    func noteSnoozed(_ rows: [InboxRow]) {
        snoozedHere += rows.map { ($0.messageId, $0.senderName, $0.subject) }
    }

    public func toggle(_ id: String) {
        if let i = selection.firstIndex(of: id) { selection.remove(at: i) } else { selection.append(id) }
    }

    public func add(_ id: String) { if !selection.contains(id) { selection.append(id) } }

    /// ⌘A and ⌘⇧A: the selection becomes exactly these ids, in list order.
    public func select(_ ids: [String]) { selection = ids }

    /// Written only when it changes (perf-archive): every archive calls this, and a write of an equal empty array would still mark the
    /// selection changed for every view that reads it (each row's key, the glass foot).
    public func clearSelection() { if !selection.isEmpty { selection = [] } }

    /// Drops ids that are no longer on screen (archived, snoozed, moved to another tab). Written only when an id leaves: `removeAll`
    /// on the observed array marks it changed even when it removes nothing.
    public func prune(visible: [String]) {
        let keep = Set(visible)
        if selection.contains(where: { !keep.contains($0) }) { selection.removeAll { !keep.contains($0) } }
    }
}

/// Words to draw later: a catalogue key and its arguments (never the translated text), so the text follows the language.
public struct Said: Equatable, Sendable {
    public let key: String
    public let args: [String]
    public init(_ key: String, _ args: any CustomStringConvertible...) { self.key = key; self.args = args.map { String(describing: $0) } }
    /// the text in the current language (reading Words makes the view depend on the language)
    @MainActor public var text: String { args.isEmpty ? Words.text(key) : WordsFormat.fill(Words.text(key), args) }
}
