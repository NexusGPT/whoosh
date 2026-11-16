import Foundation

/// Contracts delta 13: `fixture load full-history-<n>`, the large fixture the memory budget is judged on
/// (perf/memory-mac.py, fixtures/tests-j1.json "perf"): the fixture's first n accounts each get
/// FIXTURE_FULL_HISTORY_PER_ACCOUNT messages, a mailbox's whole history spread over the years before now, in threads of
/// four. Accounts, labels and settings stay; the mail and every per-message state start over, as `fixture corpus` does.
/// The meaning index is not warmed: the memory budget without the model (MEM_MAC_MB) is read on this load, and a
/// meaning search loads the model on demand (RSS_MODEL_MAX_MB).
extension FixtureCore {
    public func loadFullHistory(accounts n: Int) throws {
        let per = WhooshConstants.FIXTURE_FULL_HISTORY_PER_ACCOUNT
        let sync: [(StreamKind, EventSink)] = try lock.withLock {
            guard n >= 1, n <= fx.accounts.count else {
                throw CoreError.Invalid(field: "accounts", reasonWords: "the fixture has \(fx.accounts.count) accounts: pick from 1 to \(fx.accounts.count)")
            }
            let made = Self.history(accounts: Array(fx.accounts.prefix(n)).map { (id: $0.id, address: $0.address) }, perAccount: per, nowMs: clock.nowMs())
            return installGeneratedLocked(made)
        }
        announceGenerated(sync)
    }

    /// The most mails `fixture seed-corpus` makes: past it the verb refuses in words (a typo must not fill the Mac's memory).
    public static let seedCorpusMax = 2_000_000

    /// `fixture seed-corpus <N>` (perf/search.py, item 8149bdd7): the fixture holds exactly N generated mails spread over all its accounts (the same
    /// rows `loadFullHistory` makes, cut to N), so `index/message_count` reads N and a search runs over N mails. Accounts, labels and settings stay.
    public func seedCorpus(count n: Int) throws {
        guard n >= 1, n <= Self.seedCorpusMax else {
            throw CoreError.Invalid(field: "count", reasonWords: "seed-corpus takes a number of mails from 1 to \(Self.seedCorpusMax)")
        }
        let sync: [(StreamKind, EventSink)] = try lock.withLock {
            let accounts = fx.accounts.map { (id: $0.id, address: $0.address) }
            guard !accounts.isEmpty else { throw CoreError.Invalid(field: "count", reasonWords: "the fixture has no account to hold the mails") }
            var made = Self.history(accounts: accounts, perAccount: (n + accounts.count - 1) / accounts.count, nowMs: clock.nowMs())
            if made.rows.count > n {
                for dropped in made.rows[n...] { made.receivedAt[dropped.id] = nil }
                made.rows = Array(made.rows[..<n])
            }
            return installGeneratedLocked(made)
        }
        announceGenerated(sync)
    }

    /// Under the lock: the generated mail replaces the fixture's mail and every per-message state starts over. Returns the sync-status sinks to tell.
    private func installGeneratedLocked(_ made: (rows: [FixtureFile.Message], receivedAt: [String: Int64])) -> [(StreamKind, EventSink)] {
        messages = made.rows
        resetMessages(receivedAt: made.receivedAt)   // launch-r3: the times the rows were made from, not 300,000 parses of their text
        undo = []; consumedLast = false; entries = []
        agents.reset()
        return sinks.values.filter { $0.0 == .syncStatus }
    }

    private func announceGenerated(_ sync: [(StreamKind, EventSink)]) {
        emit()
        let states = syncStates()
        Self.deliver(sync.flatMap { _, sink in states.map { (sink, CoreEvent.syncStateChanged(state: $0)) } })
    }

    private static let senders = ["Linea Supply", "Nordlicht Freight", "Provanta Tender Desk", "Atelier Verheyden", "Studio Moreau",
                                  "Harbour Bank", "Kestrel Travel", "Orbit Payroll", "Delta Print", "Fjord Analytics"]
    private static let topics = ["Quote for the print run", "Invoice for October", "Delivery window next week", "Contract renewal",
                                 "Meeting notes", "Proposal draft", "Shipping update", "Payroll run", "Travel booking", "Quarterly report"]

    /// The generated mail: deterministic (the same n gives the same rows), newest first by date, with each row's received time.
    /// launch-r3: the load used to format 300,000 dates with a DateFormatter and then `resetMessages` parsed every one back
    /// (FixtureCore.receivedAtMs, iso branch): 1.1 s plus 5.3 s measured on this Mac at load 13 (scratch benchmark,
    /// _captures/launch-r3/histbench.log), past the control socket's 5 s, so `fixture load full-history-30` never answered.
    /// The text is now written from `localtime_r` (the same local time, the same digits: 0 of 300,000 rows differ) and the
    /// time is handed over as it was made, cut to the second as the parse cut it. One difference, on purpose: in the hour a
    /// clock goes back, the parse answered the hour's second pass for a row made in its first pass (120 rows of 300,000,
    /// one hour late); the time handed over is the true one.
    static func history(accounts: [(id: String, address: String)], perAccount: Int, nowMs: Int64) -> (rows: [FixtureFile.Message], receivedAt: [String: Int64]) {
        let stepMs: Int64 = 3 * 3_600_000   // one message every three hours, per account
        let froms = senders.map { $0.lowercased().replacingOccurrences(of: " ", with: ".") + "@example.com" }
        let lowered = topics.map { $0.lowercased() }
        var made: [(row: FixtureFile.Message, ms: Int64)] = []
        made.reserveCapacity(accounts.count * perAccount)
        for (ai, a) in accounts.enumerated() {
            for k in 0..<perAccount {
                let s = (ai * 7 + k) % senders.count, t = (k / 4 + ai) % topics.count
                let sender = senders[s]
                let whenMs = nowMs - Int64(k) * stepMs - Int64(ai) * 60_000
                let subject = "\(topics[t]) #\(k / 4)"
                let body = "Hello, about \(lowered[t]): message \(k) of \(perAccount) for \(a.address). Kind regards, \(sender)."
                made.append((FixtureFile.Message(id: "h-\(a.id)-\(k)", sender: sender, from: froms[s], account: a.id, subject: subject,
                                                 preview: String(body.prefix(160)), time: "", unread: k % 9 == 0, pinned: false,
                                                 attachments: nil, thread_count: 4, trackers_blocked: 0, tab: "primary", body: [body], quoted: nil,
                                                 date: localText(ms: whenMs), thread: "h-\(a.id)-t\(k / 4)"), whenMs))
            }
        }
        made.sort { ($0.row.date ?? "") > ($1.row.date ?? "") }
        var received: [String: Int64] = [:]
        received.reserveCapacity(made.count)
        for m in made { received[m.row.id] = m.ms - ((m.ms % 1000) + 1000) % 1000 }   // to the second, as "yyyy-MM-dd'T'HH:mm:ss" holds it
        return (made.map(\.row), received)
    }

    /// "yyyy-MM-dd'T'HH:mm:ss" in the Mac's time zone, the text the en_US_POSIX DateFormatter wrote, without a formatter.
    static func localText(ms: Int64) -> String {
        var t = time_t(ms >= 0 ? ms / 1000 : (ms - 999) / 1000)
        var parts = tm()
        localtime_r(&t, &parts)
        func two(_ v: Int32) -> String { v < 10 ? "0\(v)" : "\(v)" }
        let y = Int(parts.tm_year) + 1900
        let year = y < 10 ? "000\(y)" : y < 100 ? "00\(y)" : y < 1000 ? "0\(y)" : "\(y)"
        return "\(year)-\(two(parts.tm_mon + 1))-\(two(parts.tm_mday))T\(two(parts.tm_hour)):\(two(parts.tm_min)):\(two(parts.tm_sec))"
    }
}
