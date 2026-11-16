import Foundation
import Observation

/// Pure reads over one row, shared by the view and the `rows` probe so both say the same thing.
public enum RowFacts {
    /// The first letter of the first two words: "Linea Supply" -> "LS", "Hetzner-style host" -> "HH", "GitHub" -> "G".
    public static func initials(_ name: String) -> String {
        let words = name.split(whereSeparator: \.isWhitespace)
        let picked = words.prefix(2).compactMap { $0.first { $0.isLetter || $0.isNumber } }
        return picked.isEmpty ? "?" : String(picked).uppercased()
    }

    /// The sender's domain for grouping: the text after "@" in the address, else the sender name.
    public static func domain(address: String?, sender: String) -> String {
        guard let a = address, let at = a.lastIndex(of: "@") else { return sender }
        return String(a[a.index(after: at)...]).lowercased()
    }

    /// Today: "09:12"; this week: "Tue" ("mar." in French); older: "4 Nov" ("4 nov." in French). Local time, 24-hour like the fixture.
    /// The day and month words follow the language picked in Settings: General (`Words.current`: the stored choice, else the Mac's list), the
    /// same one every other word in the window uses, never the system's region and never English by default.
    public static func time(_ ms: Int64, now: Date = Date(), calendar: Calendar = .current, language: String = Words.current) -> String {
        _ = WordsLanguage.shared.revision   // a view that draws a date depends on the language and redraws when it changes
        let d = Date(timeIntervalSince1970: Double(ms) / 1000)
        let f = formats(language)
        if calendar.isDate(d, inSameDayAs: now) { return f.clock.string(from: d) }
        if let days = calendar.dateComponents([.day], from: d, to: now).day, days < 7 { return f.weekday.string(from: d) }
        return f.dayMonth.string(from: d)
    }

    /// One set of formatters per language, built on first use (perf r2: a DateFormatter per row per render was 4.3% of next's render
    /// samples). A formatter is expensive to make and cheap to reuse; read on the main actor only (the rows' body and the probes).
    private struct Formats { let clock, weekday, dayMonth: DateFormatter }
    nonisolated(unsafe) private static var byLanguage: [String: Formats] = [:]
    private static func formats(_ language: String) -> Formats {
        if let f = byLanguage[language] { return f }
        func make(_ format: String) -> DateFormatter {
            let f = DateFormatter()
            f.locale = Locale(identifier: language)
            f.dateFormat = format
            return f
        }
        let f = Formats(clock: make("HH:mm"), weekday: make("EEE"), dayMonth: make("d MMM"))
        byLanguage[language] = f
        return f
    }

    /// The pill the row draws, as the `rows` probe says it: its number, or null for a single message.
    public static func pillCountProbe(_ r: InboxRow) -> Any { ThreadFacts.pillCount(r.threadCount) ?? NSNull() }

    /// The thread holds a saved draft (contract delta 11 `drafts(threadId:)`, through the quick reply's DraftSource).
    /// Reading `draftsRevision` makes a row redraw its mark when a draft is saved or deleted (quickreply round 4).
    @MainActor
    public static func hasDraft(threadId: String, in m: AppModel) -> Bool {
        let q = QuickReplyModel.shared
        _ = q.draftsRevision
        return !q.drafts.drafts(threadId: threadId, core: m.core).isEmpty
    }
}

/// The receiving account of a row: name, address, colour and icon pick for its emblem, all from the shell's
/// `AccountBook.shared` (the core's `accounts()`). An account the core does not list gets the neutral ring.
public struct RowAccount: Equatable, Sendable {
    public let id: String
    public let name: String
    public let address: String
    public let colour: String
    public let iconId: String?
    public let needsAttention: Bool

    /// One core read per render, keyed by account id: every row of the pass looks its account up here.
    @MainActor
    public static func directory(_ m: AppModel) -> [String: RowAccount] {
        Dictionary(AccountBook.shared.accounts(core: m.core).map { a in
            (a.id, RowAccount(id: a.id, name: a.name, address: a.address, colour: a.colour, iconId: a.iconId,
                              needsAttention: a.health != nil))
        }, uniquingKeysWith: { first, _ in first })
    }

    /// The directory, read from the core only when the counts changed since the last read (an account's unread or
    /// health moves with them). Perf r2: rebuilding it on every render was 8.9% of archive's render samples. The list,
    /// its probe and the reader pane read this one.
    @MainActor
    public static func cachedDirectory(_ m: AppModel) -> [String: RowAccount] {
        // Reading the revision inside a view body subscribes the list to it: an icon or colour pick redraws the rows in the same update.
        let rev = AccountMarkRevision.shared.value
        if let c = DirectoryCache.entry, c.counts == m.counts, c.revision == rev { return c.book }
        let book = directory(m)
        DirectoryCache.entry = (m.counts, rev, book)
        return book
    }

    /// One account by id (the notch's rows call this one); a list pass reads `directory` once instead.
    @MainActor
    public static func of(_ accountId: String, in m: AppModel) -> RowAccount { of(accountId, in: directory(m)) }

    public static func of(_ accountId: String, in book: [String: RowAccount]) -> RowAccount {
        book[accountId] ?? RowAccount(id: accountId, name: accountId, address: "", colour: neutral, iconId: nil, needsAttention: false)
    }

    /// no colour: the emblem draws the separator grey for a value that does not parse
    public static let neutral = ""
}

@MainActor
enum DirectoryCache {
    static var entry: (counts: Counts, revision: Int, book: [String: RowAccount])?
}

/// Counts every write that changes what an account's mark shows (icon, colour, name). The directory above is keyed by the unread
/// counts, which an icon or colour pick does not move: that left the list rows on the old mark until the next count change
/// (the 3 s lag J4.12 waited out), while the sidebar and the reader read the core on each draw. The Accounts session bumps this
/// after the core accepted the write, so the list, the sidebar and the reader read the same core value in the same update.
@MainActor @Observable
public final class AccountMarkRevision {
    public static let shared = AccountMarkRevision()
    public private(set) var value = 0
    public func bump() { value += 1 }
}
