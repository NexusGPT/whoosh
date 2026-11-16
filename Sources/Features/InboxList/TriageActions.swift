import Foundation

/// What the list's keys do. Every change is a core Command through AppModel.perform (the one write path);
/// this file only decides WHICH command and on WHICH rows.
@MainActor
public enum TriageActions {
    static var state: InboxListState { .shared }

    /// The rows an action applies to: the mail shown while search results show (the inbox selection is not on screen then), else the
    /// selection when there is one, else the focused row.
    static func targets(_ m: AppModel) -> [String] {
        if m.resultsShow { return m.shownMessageId.map { [$0] } ?? [] }
        // perf-archive: an empty selection has nothing to prune, so the common key (no X selection) no longer maps every row's id
        // into an array and a set on the key's path; the result is the same (an empty selection stays empty).
        if !state.selection.isEmpty {
            let t0 = InteractionProbe.phaseStart()
            state.prune(visible: m.rows.map(\.messageId))
            InteractionProbe.phaseEnd(.triageTargets, t0)
        }
        if !state.selection.isEmpty { return state.selection }
        return m.shownMessageId.map { [$0] } ?? []
    }

    static func archive(_ m: AppModel) -> Bool {
        let ids = targets(m)
        guard !ids.isEmpty else { return false }
        let ok = RowCollapse.run { m.perform(.archive(messageIds: ids)) }
        let t0 = InteractionProbe.phaseStart()
        if ok { state.clearSelection(); state.say(ids.count == 1 ? Said("Archived") : Said("Archived %lld", ids.count)) }
        InteractionProbe.phaseEnd(.listSay, t0)
        return ok
    }

    /// The open picker's action. An action whose picker is no longer on the window (another screen took its place) is dropped here: a picker nobody
    /// sees never takes a key or an Enter (lead, 13 Nov 2026, key trace on main156: Esc in the label field closed the picker's view and left its
    /// action, which then took every typed letter and would have labelled the mail on Enter).
    static func live(_ m: AppModel) -> InboxListState.Pending? {
        guard let p = state.pending else { return nil }
        let own: String
        switch p {
        case .snooze: own = Overlay.snooze
        case .label: own = Overlay.label
        }
        if m.overlay == own { return p }
        state.pending = nil
        m.typed = ""
        TriagePickState.shared.reset()
        return nil
    }

    /// H and L open their picker; the typed words plus Enter finish it (`finish`).
    static func openPicker(_ m: AppModel, _ make: ([String]) -> InboxListState.Pending, overlay: String) -> Bool {
        let ids = targets(m)
        guard !ids.isEmpty else { return false }
        #if os(macOS)
        TriagePick.installOnce()   // the key monitor's hook is in place before the first letter (it was installed with the list's key table, which the first keys never reached)
        #endif
        state.pending = make(ids)
        m.typed = ""
        TriagePickState.shared.reset()
        // the first line that can be taken is the highlighted one ("Later today" is refused late in the evening)
        if case .snooze? = state.pending { TriagePickState.shared.highlight = TriagePick.snoozeOptions().firstIndex { $0.refusal == nil } ?? 0 }
        m.overlay = overlay
        return true
    }

    /// Enter: completes an open picker, else opens the focused thread. The picker's line is the one the typed words name; with nothing typed, the
    /// highlighted one (TriagePick). "Pick a date…" opens the calendar step and keeps the picker up.
    static func enter(_ m: AppModel) -> Bool {
        guard let p = live(m) else { return open(m) }
        let words = m.typed.trimmingCharacters(in: .whitespacesAndNewlines)
        let command: Command
        let said: String
        switch p {
        case .snooze(let ids):
            let options = TriagePick.snoozeOptions()
            guard let i = TriagePick.snoozeIndex(m, options) else { return false }
            let o = options[i]
            if o.kind == .pickDate { TriagePickState.shared.pickingDate = true; return true }
            guard o.refusal == nil, let when = TriagePick.when(o.kind) else { return false }
            command = .snooze(messageIds: ids, when: when)
            said = words.isEmpty ? Words.text(o.title) : words
        case .label(let ids):
            let lines = TriagePick.labels(m)
            let pick = min(TriagePickState.shared.highlight, lines.count - 1)
            let name = lines.indices.contains(pick) ? lines[pick].label.name : words   // words that name no listed label go to the engine as typed, as before
            guard !name.isEmpty else { return false }
            command = .label(messageIds: ids, labelName: name, createIfMissing: false)
            said = name
        }
        return finish(m, p, command, said: said)
    }

    /// The calendar step of the snooze picker chose a date.
    static func finishSnooze(_ m: AppModel, when: SnoozeWhen, words: String) -> Bool {
        guard let p = state.pending, case .snooze(let ids) = p else { return false }
        return finish(m, p, .snooze(messageIds: ids, when: when), said: words)
    }

    private static func finish(_ m: AppModel, _ p: InboxListState.Pending, _ command: Command, said: String) -> Bool {
        let taken = m.rows.filter { r in
            if case .snooze(let ids) = p { return ids.contains(r.messageId) }
            return false
        }
        let ok = RowCollapse.run { m.perform(command) }
        if ok { state.noteSnoozed(taken) }
        close(m)
        if ok { state.clearSelection(); state.say(p.doneWords(said)) }
        return ok
    }

    static func close(_ m: AppModel) {
        state.pending = nil
        m.typed = ""
        TriagePickState.shared.reset()
        if m.overlay == Overlay.snooze || m.overlay == Overlay.label { m.overlay = nil }
    }

    static func open(_ m: AppModel) -> Bool {
        guard let i = m.rows.indices.first(where: { $0 == m.focusIndex }) else { return false }
        guard m.perform(.openThread(threadId: m.rows[i].threadId)) else { return false }
        #if os(macOS)
        // the picked three-pane design: the thread opens in the reader pane (its Escape returns to mac-main)
        if m.overlay == "mac-thread" { m.overlay = nil }
        m.surface = "mac-reader"
        #else
        m.overlay = "mac-thread"
        #endif
        return true
    }

    /// Z: the most recent action not yet undone (last in, first out); the capsule leaves with it.
    static func undo(_ m: AppModel) -> Bool {
        guard m.perform(.undo) else { return false }
        let t0 = InteractionProbe.phaseStart()
        state.say(nil)
        InteractionProbe.phaseEnd(.listSay, t0)
        return true
    }

    static func pin(_ m: AppModel) -> Bool {
        guard let row = m.shownRow else { return false }
        let id = row.messageId, pinned = row.pinned
        guard m.perform(.pin(messageId: id, pinned: !pinned)) else { return false }
        state.say(pinned ? Said("Unpinned") : Said("Pinned"))
        if !m.resultsShow { m.focus(messageId: id) }
        return true
    }

    /// U: read becomes unread and back, on the targets (the first target decides the direction).
    static func toggleRead(_ m: AppModel) -> Bool {
        let ids = targets(m)
        guard let first = ids.first, let row = m.resultsShow ? m.shownRow : m.rows.first(where: { $0.messageId == first }) else { return false }
        return m.perform(.markRead(messageIds: ids, read: row.unread))
    }

    /// "Mark all as read": every unread row on the current split.
    static func markAllRead(_ m: AppModel) -> Bool {
        let ids = m.rows.filter(\.unread).map(\.messageId)
        guard !ids.isEmpty else { return false }
        return m.perform(.markRead(messageIds: ids, read: true))
    }
}

/// The overlay names the list owns (uistate.overlay).
public enum Overlay {
    public static let snooze = "mac-main.snooze"
    public static let label = "mac-main.label"
}

/// Words -> a snooze time, from the same option titles the TimePickerInWords control shows.
public enum SnoozeWords {
    public static func when(_ words: String, now: Date = Date()) -> SnoozeWhen? {
        let w = words.lowercased()
        for o in TimeInWords.options(now: now) where o.title.lowercased().hasPrefix(w) || w.hasPrefix(o.title.lowercased()) {
            switch o.kind {
            case .laterToday: return .laterToday
            case .tomorrowMorning: return .tomorrowMorning
            case .nextWeek: return .nextWeek
            case .pickDate: return nil
            }
        }
        return nil
    }
}
