import Foundation

/// The zero state: "All done", the next snooze return time from the store, and the next-tab hint.
@MainActor
struct EmptyStateWords {
    let title: String
    let line: String
    let keyHint: String

    static func make(_ m: AppModel) -> EmptyStateWords? {
        guard m.rows.isEmpty else { return nil }
        if let syncing = FirstSyncWords.make(m) { return syncing }
        if let label = InboxListState.shared.labelFilter { return LabelFilter.emptyWords(label) }   // a label with no message: its words, not "All done"
        let name = PinnedTabs.name(m.tab, m)
        var line = Words.text("Nothing in %@.", name)
        if let next = nextSnoozeReturn(m) { line += " " + Words.text("Next snooze returns at %@.", next) }
        let ids = m.tabIds
        var hint = ""
        if let i = ids.firstIndex(of: m.tab), ids.count > 1 {
            let nextId = ids[(i + 1) % ids.count]
            hint = Words.text("Tab for %@", PinnedTabs.name(nextId, m))
        }
        return EmptyStateWords(title: Words.text("All done"), line: line, keyHint: hint)
    }

    /// The earliest return among snoozed messages, read from the core (`snooze/<id>/return_local_time`).
    /// The earliest return among the snoozed messages, from the core's own snooze read (`snoozedMessages()`, every
    /// snooze carries its `returnAtMs`). The old read asked the inbox for a TAB named "snoozed", which holds no rows, so
    /// the line never named a time (J1.12, tests r4). Today: "15:00"; a later day: "Thursday 15:00" (the app's clock).
    static func nextSnoozeReturn(_ m: AppModel) -> String? {
        guard let ms = m.core.snoozedMessages().map(\.returnAtMs).min() else { return nil }
        let at = Date(timeIntervalSince1970: Double(ms) / 1000)
        let now = Date(timeIntervalSince1970: Double(m.host.clock.nowMs()) / 1000)
        return (Calendar.current.isDate(at, inSameDayAs: now) ? SnoozeWordsFormat.clock : SnoozeWordsFormat.dayClock).string(from: at)
    }
}

/// "Why?" on a row: what placed it in its tab, read from `InboxRow.decidedBy` (the typed DecidedBy, delta 6) and said
/// in one voice: a rule or a model, by name. None means the default placement. The words come from the enum case.
@MainActor
enum WhyWords {
    /// The case in the fixture's words ("Laya", "Jev", "Owner", "Rule: <id>", "Default"); nil when the row has no category.
    static func decidedBy(_ r: InboxRow) -> String? {
        switch r.decidedBy {
        case nil: return nil
        case .laya?: return "Laya"
        case .jev?: return "Jev"
        case .apple?: return "Apple Intelligence"
        case .model(let model)?: return DecisionModelCatalogue.entry(id: model).name   // SPEC A133: an on-device model other than Laya, by its catalogue name
        case .owner?: return "Owner"
        case .rule(let id)?: return "Rule: \(id)"
        case .default?: return "Default"
        }
    }

    static func line(_ r: InboxRow, in m: AppModel) -> String {
        let tab = PinnedTabs.name(m.tab, m)      // the tab's translated name, as the tab row draws it
        switch r.decidedBy {
        case .laya?: return Words.text("Placed in %@ by Laya", tab)
        case .jev?: return Words.text("Placed in %@ by Jev", tab)
        case .apple?: return Words.text("Placed in %@ by Apple Intelligence", tab)
        case .model(let model)?: return Words.text("Placed in %1$@ by %2$@", tab, DecisionModelCatalogue.entry(id: model).name)
        case .owner?: return Words.text("Placed in %@ by you", tab)
        // a rule shows its id until a rule-name read exists (contract delta 7)
        case .rule(let id)?: return Words.text("Placed in %@ by the rule %@", tab, id)
        case .default?, nil: return Words.text("Placed in %@ by default", tab)
        }
    }
}

/// "All accounts · 23 unread": the list header line, the account filter's name and the split's unread count.
@MainActor
enum ListHeaderWords {
    static func filterName(_ m: AppModel) -> String {
        guard let id = m.accountFilter else { return Words.text("All accounts") }
        return RowAccount.of(id, in: m).name
    }

    static func unread(_ m: AppModel) -> Int { Int(m.counts.tabs.first { $0.tabId == m.tab }?.unread ?? 0) }

    static func line(_ m: AppModel) -> String {
        if let folder = SmartFolderWords.name(m) { return folder + " · " + Words.text("%lld conversations", m.rows.count) }
        if let box = m.mailbox { return MailboxWords.name(box) + " · " + Words.text("%lld conversations", m.rows.count) }
        return Words.text("%@ · %lld unread", filterName(m), unread(m))
    }
}

/// The first-sync state: while any healthy account still has its history to fetch, the list says how many
/// accounts are indexed of how many the core lists (`WhooshCore.syncStates()`); an account with an error is not
/// "fetching", it is the failure line's.
@MainActor
enum FirstSyncWords {
    struct Progress: Equatable { let indexed: Int; let total: Int }

    static func progress(_ m: AppModel) -> Progress? {
        let states = m.core.syncStates()
        let fetching = states.contains { !$0.historyFetched && $0.lastError == nil }
        guard fetching else { return nil }
        return Progress(indexed: states.filter(\.historyFetched).count, total: states.count)
    }

    static func make(_ m: AppModel) -> EmptyStateWords? {
        guard m.rows.isEmpty, let p = progress(m) else { return nil }
        return EmptyStateWords(title: Words.text("Fetching your history"),
                               line: Words.text("%lld of %lld accounts indexed · you can already search what is here", p.indexed, p.total), keyHint: "")
    }
}

/// Text values from the core's state paths (JSON fragments): a string comes back without its quotes; nil when the
/// core serves no value or an empty one.
@MainActor
enum CoreText {
    static func value(_ path: String, in m: AppModel) -> String? {
        guard let raw = m.core.state(path: path) else { return nil }
        let t = raw.trimmingCharacters(in: CharacterSet(charactersIn: "\""))
        return t.isEmpty || t == "null" ? nil : t
    }

    /// A split tab's name as the core serves it (`split_tab/<id>/name`), else its id.
    static func tabName(_ id: String, in m: AppModel) -> String { value("split_tab/\(id)/name", in: m) ?? id }
}

/// A mailbox's name in the current language: the same catalogue keys the sidebar draws ("Sent", "Archive", "All mail").
@MainActor
enum MailboxWords {
    static func key(_ box: Mailbox) -> String {
        switch box {
        case .outbox: "Outbox"
        case .vips: "VIPs"
        case .pinned: "Pinned"
        case .snoozed: "Snoozed"
        case .drafts: "Drafts"
        case .sent: "Sent"
        case .junk: "Junk"
        case .trash: "Trash"
        case .archive: "Archive"
        case .starred: "Starred"
        case .muted: "Muted"
        case .allMail: "All mail"
        case .reminders: "Reminders"   // delta 18: the view of reply reminders (engine-import)
        }
    }
    static func name(_ box: Mailbox) -> String { Words.text(key(box)) }
}

/// The saved smart folder the sidebar opened (AppModel.smartFolderId): its own name as the owner saved it; nil when none.
/// Its count is the list's rows, the same query the rows come from.
@MainActor
enum SmartFolderWords {
    static func name(_ m: AppModel) -> String? {
        guard let id = m.smartFolderId else { return nil }
        return m.core.smartFolders().first { $0.id == id }?.name ?? id
    }
}

/// The two shapes of a return time on the zero-state line, built once.
@MainActor
enum SnoozeWordsFormat {
    static let clock: DateFormatter = { let f = DateFormatter(); f.locale = Locale(identifier: "en_GB"); f.dateFormat = "HH:mm"; return f }()
    static let dayClock: DateFormatter = { let f = DateFormatter(); f.locale = Locale(identifier: "en_GB"); f.dateFormat = "EEEE HH:mm"; return f }()
}
