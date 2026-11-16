import Foundation
import Observation

/// Search in the notch (owner's pick 3A of 14 Nov 2026: "in the search field"). What is typed in the field on the list's first line is asked of
/// the engine's one search (Command.search; the answers come on the Search stream, the door the main window's search uses), and the answer takes
/// the list's place: the mails that hold the typed letters first, then the related mail. The main window's search is not touched: its text, its
/// results and its selection stay as they are. The engine keeps one search current at a time, so the newest words win there; a search is kept
/// as a recent one only when Return opens one of its mails (NotchModel.openRow), as in the main window.
@MainActor @Observable
public final class NotchSearch {
    /// What the field holds.
    public internal(set) var text = ""
    /// The answer for `answeredFor`, in the engine's order, as mails the notch can list and act on (each marked `found`).
    public internal(set) var results: [NotchItem] = []
    /// The third line of each row of the answer, by mail: the passage that made it a result (the engine's, around the first match).
    public internal(set) var gists: [String: String] = [:]
    /// Told when an answer arrives from the engine (the model checks that the row under the keyboard is still listed).
    @ObservationIgnored var onAnswer: @MainActor () -> Void = {}
    /// How many of `results`, from the top, hold the typed letters; the ones after them are related mail.
    public internal(set) var matches = 0
    /// The text the answer on screen is for.
    public internal(set) var answeredFor = ""
    /// The engine refused the words (its own sentence), or nil.
    public internal(set) var refusal: String?
    /// From the last typed letter to its answer in the model, in milliseconds.
    public internal(set) var lastMs: Double?
    @ObservationIgnored private var awaiting = ""
    @ObservationIgnored private var askedAt = Date()
    @ObservationIgnored private var query = SearchQuery()
    @ObservationIgnored private var subscription: UInt64?

    public init() {}

    /// Words are typed: the list gives its place to the answer.
    public var active: Bool { !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    /// The words on screen have no answer yet.
    public var waiting: Bool { active && refusal == nil && answeredFor != text }

    /// The field's text changed: empty words give the list back; else the engine is asked at once (it drops an older search for a newer one).
    func set(_ new: String, _ m: AppModel) {
        guard new != text else { return }
        text = new
        guard active else { clearAnswer(); return }
        let q = SearchQuery.typed(new)
        query = q
        askedAt = Date()
        refusal = nil
        guard let words = SearchEngine.engineQuery(q) else {
            // a quoted figure is not an engine search by design: it runs over the rows the app holds, as in the main window
            awaiting = ""
            show(SearchEngine.run(q, over: m.rows, beforeMs: SearchMacModel.monthStartMs), for: new)
            return
        }
        if subscription == nil {
            subscription = m.core.subscribe(stream: .search, sink: SearchHitSink { [weak self] words, hits in self?.receive(words, hits, m) })
        }
        awaiting = words
        do {
            _ = try m.core.run(command: .search(query: words))
        } catch {
            let said = SearchMacModel.engineWords(error)
            clearAnswer()
            refusal = said.isEmpty ? SearchMacModel.refusedSentence : said
        }
    }

    /// The Search stream's answer. The engine answers the same words more than once (the word hits, then the full ranking): each answer for the
    /// words asked for replaces the one on screen. An answer for other words (the main window's search, an older letter) is not this field's.
    private func receive(_ words: String, _ hits: [SearchHit], _ m: AppModel) {
        guard !awaiting.isEmpty, words == awaiting else { return }
        let rows = SearchEngine.rowsOfHits(hits, core: m.core, read: nil)
        show(SearchEngine.fromHits(query, hits: hits, rows: rows, beforeMs: SearchMacModel.monthStartMs, names: { _ in [] }, engineFiltered: true), for: text)
        onAnswer()
    }

    /// The matches, then the related mail; "all other mail" is the main window's (Open in Whoosh), never listed here.
    private func show(_ outcome: SearchEngine.Outcome, for words: String) {
        var seen = Set<String>()
        let kept = outcome.results.filter { $0.part != .rest && seen.insert($0.row.messageId).inserted }.prefix(NotchMetrics.listMax)
        results = kept.map { r in
            var it = NotchItem(id: r.row.messageId, threadId: r.row.threadId, accountId: r.row.accountId, sender: r.row.senderName, subject: r.row.subject,
                               preview: r.row.preview, code: nil, address: r.row.senderAddress,
                               receivedAt: Date(timeIntervalSince1970: Double(r.row.receivedAtMs) / 1000))
            it.read = !r.row.unread
            it.found = true
            return it
        }
        gists = Dictionary(kept.map { ($0.row.messageId, Self.gist($0.passage.isEmpty ? $0.row.preview : $0.passage)) }, uniquingKeysWith: { first, _ in first })
        matches = kept.prefix { $0.part == .match }.count
        answeredFor = words
        lastMs = Date().timeIntervalSince(askedAt) * 1000
    }

    /// A passage as one tidy line: the engine cuts it around the match, so it can start inside a word ("house. Please find the signed
    /// quote"); a start that is the tail of a word (a small letter first) loses that tail and says it was cut, with an ellipsis.
    static func gist(_ passage: String) -> String {
        let line = passage.split(whereSeparator: \.isNewline).joined(separator: " ").trimmingCharacters(in: .whitespaces)
        guard let first = line.unicodeScalars.first, CharacterSet.lowercaseLetters.contains(first), let gap = line.firstIndex(of: " ") else { return line }
        return "… " + line[line.index(after: gap)...].trimmingCharacters(in: .whitespaces)
    }

    func clear() { text = ""; clearAnswer() }

    private func clearAnswer() { results = []; gists = [:]; matches = 0; answeredFor = ""; awaiting = ""; refusal = nil }

    /// A mail the notch acted on (archived, deleted, snoozed) leaves the answer.
    func drop(id: String) {
        guard let i = results.firstIndex(where: { $0.id == id }) else { return }
        results.remove(at: i)
        if i < matches { matches -= 1 }
    }

    func setRead(id: String, _ read: Bool) {
        if let i = results.firstIndex(where: { $0.id == id }) { results[i].read = read }
    }
}
