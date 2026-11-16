import Foundation
import Observation
import SwiftUI

/// The Mac search screen's state. The field's text is `AppModel.typed` (one home: the real keyboard and the socket
/// `type` verb both write it); everything else here is what the screen alone remembers. Three phases, one screen:
/// typing (the dropdown under the field), searching (skeleton rows in the list pane), results (the matching emails).
@MainActor @Observable
public final class SearchMacModel {
    public static let shared = SearchMacModel()

    /// count: the core's row count for the folder's query (as the main sidebar shows it); threads: the field's reading of the rule.
    public struct Folder: Equatable { public let id: String; public let name: String; public let rule: String; public let threads: [String]; public let count: Int
        /// the engine refused the rule's words: the sentence in the folder's place ("" when it did not)
        public var says = "" }

    public var recents: [String] = []
    public var folders: [Folder] = []
    public var naming = false
    public var savedQuery = ""
    public var fieldSays = ""
    /// One plain sentence the results pane shows about the last act (a result opened that has gone); cleared by the next search.
    public var notice = ""
    public var selected = 0
    /// SPEC A150: the closed line "All other mail · N" is open (the rest is listed under it). Closed at every new search.
    public var restOpen = false
    /// The selection is on the closed line (Down from the last related row reaches it); `selected` keeps the result the reader shows.
    public var onMore = false
    public var returnToSearch = false
    /// J5.6: the conversations ⌘A selected (thread ids), every listed result; empty = the one `selected` result. A new search, Esc or a click clears it.
    public var picked: Set<String> = []
    /// The caret is in the field: ⌘K, `/` or a click on it put it there; the return from an opened result (Esc) leaves the field holding the keys
    /// without it, so ⌘A then selects the results instead of the field's words.
    @ObservationIgnored var caret = false
    /// What the field and the selection held when a result opened: Escape from that thread brings them back.
    var returnQuery = ""
    var returnSelected = 0
    var returnLaunched = false
    public var openedMessageId: String?
    public var dark = false
    /// the search is open in the main window: its field is live, its dropdown and results draw in the list pane
    public var active = false
    /// the field holds the keyboard focus (read by the `mac-search.bar` row)
    public var fieldFocused = false
    /// The field's focus intent (SearchFocus.swift): true from ⌘K, `/` or a click on the field until Esc, a click elsewhere or another surface takes the keys.
    public var owns = false
    /// The field's text while it does not hold the shared typed buffer; kept in step with the buffer while it does.
    public var held = ""
    @ObservationIgnored var stash = ""
    /// the list row that had the focus when the field took the keys: a row change to this same row is not a click-away
    @ObservationIgnored var claimedAt = 0
    /// the list's row count when the field took the keys: a focus change with another count is the list's own data moving
    @ObservationIgnored var claimedRows = 0
    /// The mail the list had focused when the field took the keys (SearchFocus.focusMoved compares the mail, not only its index).
    @ObservationIgnored var claimedId: String?
    /// When the field last took the keys, and why it last let them go (served on the `mac-search.bar` row as `released_by`).
    @ObservationIgnored var claimedTime = Date.distantPast
    @ObservationIgnored var releasedBy = ""
    /// A focus loss this soon after a claim is the window settling, not the owner leaving (a check bound on SwiftUI's focus hand-over, not a product number).
    static let claimSettleSeconds: TimeInterval = 0.5
    @ObservationIgnored var fieldFrame = CGRect.zero
    @ObservationIgnored var dropdownFrame = CGRect.zero
    var corpus: [InboxRow] = []

    /// the words the results on screen were searched for; the list shows them until Enter launches the field's new text
    public var launched = false
    var launchedText = ""
    /// the words and filters the list shows: what Enter launched, until a chip typed or removed after it narrows or widens the list at once
    var shownText = ""
    /// skeleton rows show while true: from Enter until the answer is in
    public var loading = false
    /// the skeleton stays until released: the shot verb's hold, never set by a key
    public var holdSkeleton = false
    /// Esc closed the dropdown; typing opens it again
    public var dismissed = false
    /// The dropdown row Enter and Tab act on. It belongs to the text it was set for: a key that changes the text puts it back on the first row
    /// at once, in the same read, not when the field's view later notices the change (an Enter that lands before that, as the socket's `type`
    /// then `Enter` does, would accept a row the old text had highlighted: main54, J2.8 launched a recent search in place of its words).
    public var highlight: Int {
        get { highlightFor == text ? rawHighlight : 0 }
        set { rawHighlight = newValue; highlightFor = text; rawKeyed = false }
    }
    private var rawHighlight = 0
    private var highlightFor = ""
    /// The owner moved to the highlighted row with the arrow keys (or clicked it) for THIS text: only then does Return take that row. Otherwise Return runs
    /// the typed words, wherever the highlight sits. Like `highlight`, it belongs to the text: a typed letter ends it in the same read.
    public var keyed: Bool { highlightFor == text && rawKeyed }
    private var rawKeyed = false
    /// Sets the highlight as the owner's own choice (an arrow key, a click on a row): Return will take it.
    func choose(_ i: Int) { highlight = i; rawKeyed = true }
    /// The row the pointer rests on: its own tint, nothing else. It never moves `highlight`, so it never changes what Return or Tab does.
    public var hovered: Int?
    /// bumped by ⌘K or `/` while the screen is open: the field takes focus again
    public var focusTick = 0
    var outcome = SearchEngine.Outcome()
    /// The engine refused the launched words (SPEC A141): nil when it did not, else its own words ("" when it gave none). The pane then says
    /// so in one sentence with these words under it and lists nothing; no other engine answers in its place.
    /// The name to draw on a person's pill: the pill's value is the address the engine is asked for ("claire.martin@…"), lower case, and the owner reads the name.
    public var pillNames: [String: String] = [:]
    public internal(set) var refusal: String?
    /// How the engine made the answer on screen (`search/last/stage`, read once after each answer): "words" the exact matches first and the other mails in date
    /// order, a second answer will rank them; "ranked" final; "no_model" or "model_failed" the first answer is final; "filtered" pills and no words; "" not known.
    public internal(set) var stage = ""
    /// the engine's reason when no meaning order came (`search/last/reason`, when it serves one)
    public internal(set) var stageReason = ""
    /// reads the stage; nil = the core's `search/last/stage`. Tests put their own here.
    @ObservationIgnored var stageRead: (() -> String?)?
    /// How a search reaches the engine; nil = the core's `Command.search`. Tests put an engine here that refuses.
    @ObservationIgnored var engineAsk: ((String) throws -> Void)?
    @ObservationIgnored var hits: [SearchHit] = []
    @ObservationIgnored var hitsFor = ""
    @ObservationIgnored var awaiting = ""
    @ObservationIgnored var searchSub: UInt64?
    /// every answer of the Search stream, by the words asked: a smart folder reads its rule's threads from the same answer the field got
    @ObservationIgnored var hitCache: [String: [SearchHit]] = [:]
    /// the rows of each cached answer's hits, read from the core in one call per answer (`rowsOfHits`); dropped wherever `hitCache` is
    @ObservationIgnored var hitRowsCache: [String: [InboxRow]] = [:]
    /// How the hits' rows are read; nil = the core's `rowsForMessages`. Tests put a counting wrapper here.
    @ObservationIgnored var hitRowsRead: (([String]) -> [InboxRow])?
    @ObservationIgnored var folderWordsPending: Set<String> = []
    /// the engine's words for each rule's plain words it refused (a smart folder asks the engine like any search)
    @ObservationIgnored var folderRefusals: [String: String] = [:]
    /// the meaning index's growth re-asks the launched words (SearchMeaningRefresh.swift)
    @ObservationIgnored var meaningListening = false
    @ObservationIgnored var reaskPending = false
    public internal(set) var reaskCount = 0
    private var nameCache: [String: [String]] = [:]

    var app: AppModel { AppModel.shared }
    public var barText: String { naming ? savedQuery : text }
    /// the field's own reading of its text: the chips drawn before it
    public var query: SearchQuery { SearchQuery.typed(barText) }
    var launchedQuery: SearchQuery { SearchQuery.typed(shownText) }
    /// the field's text differs from what the list shows: Enter launches, the dropdown is open
    public var fieldMode: Bool { !launched || text != launchedText }

    /// The dropdown's rows. Built once per (field text, recents, folders): a key's render asks for them several times.
    public var suggestions: [SearchSuggestion] {
        guard !naming else { return [] }
        let key = text + "\u{1}" + recents.joined(separator: "\u{2}") + "\u{3}\(folders.count)"
        if let c = suggestCache, c.key == key { return c.rows }
        let chips = SearchQuery.typed(text).chips
        let draft = SearchQuery.draft(of: text)
        let rows = SearchSuggest.build(draft: draft, chips: chips, source: suggestSource(for: draft), recentMax: WhooshConstants.SEARCH_RECENT_MAX)
        suggestCache = (key, rows)
        return rows
    }
    @ObservationIgnored private var suggestCache: (key: String, rows: [SearchSuggestion])?
    /// read when the screen opens (the labels do not change while it is open)
    @ObservationIgnored var labelNames: [String] = []

    /// the list pane draws the results (not the skeleton, not the name field): the header, sections and footer rows exist then
    var paneShowsResults: Bool { active && launched && !loading && !naming }

    public var dropdownVisible: Bool { holds && fieldMode && !dismissed && !naming && !suggestions.isEmpty }

    /// The people are asked of the engine by the last typed word (`PeopleRead`); a subject is one of the mails on screen that holds the typed words.
    private func suggestSource(for draft: String) -> SearchSuggest.Source {
        let last = draft.split(whereSeparator: \.isWhitespace).last.map(String.init) ?? ""
        let rows = corpus
        var src = SearchSuggest.Source(people: PeopleRead.people(core: app.core, prefix: last), labels: labelNames,
                                       folders: folders.map { ($0.name, $0.rule) }, recents: recents)
        src.domains = DomainRead.domains(core: app.core, prefix: last)
        src.subject = { typed in
            let t = typed.trimmingCharacters(in: .whitespaces)
            guard t.count >= 2 else { return nil }
            return rows.first { r in r.subject.range(of: t, options: [.caseInsensitive, .diacriticInsensitive]) != nil }?.subject
        }
        return src
    }

    /// The labels and the recent searches, read when the screen opens.
    func loadSuggestSources() {
        labelNames = app.core.labels().map(\.name)
        reloadRecents()
        suggestCache = nil
    }

    /// Recent searches: the core's `search/recent` state, newest first, already capped at SEARCH_RECENT_MAX.
    func reloadRecents() {
        guard let json = app.core.state(path: "search/recent"), let d = json.data(using: .utf8),
              let list = try? JSONDecoder().decode([String].self, from: d) else { recents = []; return }
        recents = list
    }

    /// A message's attachment names, read from the core once and kept for the screen's life.
    func attachmentNames(_ r: InboxRow) -> [String] {
        guard r.attachmentCount > 0 else { return [] }
        if let n = nameCache[r.messageId] { return n }
        let n = app.core.message(messageId: r.messageId)?.attachmentNames ?? []
        nameCache[r.messageId] = n
        return n
    }

    /// The launched query's result list. The pills in their colon forms and the plain words go to the engine as one query (`engineQuery`): every mail
    /// that passes the pills, the exact matches first, then the rest by closeness of meaning. A quoted figure is not an engine search by design and
    /// runs on the local engine. A core that refuses the query is said on the screen (`refusal`) and nothing else answers: the list stays empty.
    func runSearch() {
        let q = launchedQuery
        guard let words = SearchEngine.engineQuery(q) else {
            refusal = nil
            outcome = SearchEngine.run(q, over: corpus, beforeMs: Self.monthStartMs, names: attachmentNames)
            return
        }
        if hitsFor != words { askCore(words) }
        if refusal != nil { outcome = SearchEngine.Outcome(); return }
        if hitsFor == words {
            outcome = SearchEngine.fromHits(q, hits: hits, rows: hitRows(words, hits), beforeMs: Self.monthStartMs, names: attachmentNames, engineFiltered: true, heads: conversationRows())
            settleLine()
        }
    }

    /// The row each conversation shows in the inbox, by thread id: what a hit from an earlier mail of the thread is listed as (SPEC A152). One pass over the
    /// inbox rows, made when an answer arrives, never while a row is drawn.
    func conversationRows() -> [String: InboxRow] {
        Dictionary(corpus.map { ($0.threadId, $0) }, uniquingKeysWith: { a, _ in a })
    }

    /// The rows of one answer's hits: read once per answer, kept until the answer is replaced.
    func hitRows(_ words: String, _ hits: [SearchHit]) -> [InboxRow] {
        if let r = hitRowsCache[words] { return r }
        let r = SearchEngine.rowsOfHits(hits, core: app.core, read: hitRowsRead)
        hitRowsCache[words] = r
        return r
    }

    /// Command.search; the answers come back on the Search stream (inline on the fixture core, later on a real one).
    private func askCore(_ words: String) {
        awaiting = words; refusal = nil; stage = ""; stageReason = ""
        do {
            if let ask = engineAsk { try ask(words) } else { _ = try app.core.run(command: .search(query: words)) }
        } catch {
            awaiting = ""; hitsFor = ""; hits = []
            refusal = Self.engineWords(error)
        }
    }

    /// The engine's own words for a refusal (the contract's reason words); "" when the error carries none.
    static func engineWords(_ error: Error) -> String {
        guard let e = error as? CoreError else { return "" }
        switch e {
        case .Invalid(_, let words), .Unexpected(let words): return words
        case .Action(let failure): return failure.reasonWords
        case .NotFound, .Throttled: return ""
        }
    }

    /// The stage of the answer just received: one local read.
    private func readStage() {
        stage = Self.decoded(stageRead?() ?? app.core.state(path: "search/last/stage"))
        stageReason = Self.stageSentence(stage) != nil ? Self.decoded(app.core.state(path: "search/last/reason")) : ""
    }

    /// What the screen says when the meaning order is missing, in the app's language, by the stage (never the engine's English): the engine's own words
    /// go under it as the small detail line (`stageReason`).
    static func stageSentence(_ stage: String) -> String? {
        switch stage {
        case "no_model": return Words.text("The search model is not on this Mac. Words are searched; meaning needs the model.")
        case "model_failed": return Words.text("The meaning search could not run this time. Exact matches first, then the other mails by date.")
        case "failed": return Words.text("The search could not run.")
        default: return nil
        }
    }

    /// A state value that is a JSON string, without its quotes ("" when there is none).
    static func decoded(_ raw: String?) -> String {
        guard let raw else { return "" }
        if let d = raw.data(using: .utf8), let s = try? JSONDecoder().decode(String.self, from: d) { return s }
        return raw.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    /// The one sentence the pane says when the engine refused the search.
    static var refusedSentence: String { Words.text("The search could not run.") }

    /// The Search stream's answer. The engine answers the same words more than once (the word hits, then the full ranking): the first answer
    /// for the words asked for, and each later one for the words on screen, replaces the list in place. The selection stays on the same
    /// mail when it is still listed, so the reader does not change under the owner; an answer for other words is dropped.
    func receive(_ query: String, _ found: [SearchHit]) {
        hitCache[query] = found; hitRowsCache[query] = nil          // a new answer for these words: its rows are read again
        if folderWordsPending.remove(query) != nil { reloadFolders() }
        guard query == awaiting || (awaiting.isEmpty && query == hitsFor) else { return }
        let keep = loading ? nil : selectedResultId
        hits = found; hitsFor = query; awaiting = ""; refusal = nil
        readStage()
        guard launched else { return }
        outcome = SearchEngine.fromHits(launchedQuery, hits: hits, rows: hitRows(query, found), beforeMs: Self.monthStartMs, names: attachmentNames, engineFiltered: true, heads: conversationRows())
        settleLine()
        let rs = results
        if let id = keep, let i = rs.firstIndex(where: { $0.row.messageId == id }) { selected = i }
        else { selected = min(selected, max(0, rs.count - 1)) }
        if loading, !holdSkeleton { settleSoon() }
    }

    func subscribeSearch() {
        listenForMeaning()
        guard searchSub == nil else { return }
        searchSub = app.core.subscribe(stream: .search, sink: SearchHitSink { [weak self] q, h in self?.receive(q, h) })
    }

    /// The skeleton gives way one turn after the answer, so a frame of it is always drawn.
    func settleSoon() {
        Task { @MainActor in
            await Task.yield()
            if !holdSkeleton, awaiting.isEmpty { settle() }   // a core that has not answered keeps the skeleton
        }
    }

    /// Every conversation of the answer, one per thread (the thread's best message, the engine's order: matches, then related, then the rest). The
    /// engine's first answer lists the other mails by date, not ranked yet: they wait for the second answer (stage "words").
    var allResults: [SearchResult] {
        guard launched else { return [] }
        var seen = Set<String>()
        return outcome.results.filter { !(stage == "words" && $0.part != .match) && seen.insert($0.row.threadId).inserted }
    }

    /// The rows the list draws, in order: the matches, the related mail, then (only while the closed line is open) the rest under it (SPEC A150).
    public var results: [SearchResult] {
        let all = allResults
        return restOpen ? all : Array(all.prefix { $0.part != .rest })
    }

    /// The first answer lists the other mails in date order and the second ranks them: until then the line "Ranking the other mails by meaning…" stands
    /// in the place of "Related" and the other mails are not listed.
    public var tailPending: Bool { stage == "words" && outcome.results.contains { $0.part != .match } }

    /// How many rows stand above the closed line: the matches and the related mail.
    public var upperCount: Int { allResults.prefix { $0.part != .rest }.count }

    /// How many of the conversations are related mail (the number the count line says after the matches).
    public var relatedCount: Int { allResults.filter { $0.part == .related }.count }

    /// How many conversations stand behind the closed line: the number it shows ("All other mail · N") and the rows opening it lists.
    public var restCount: Int { allResults.filter { $0.part == .rest }.count }

    /// The closed line is on the list: the rest is known (not while the first answer is still waiting for its ranking) and not empty.
    public var showsMore: Bool { stage != "words" && restCount > 0 }

    /// The selection goes to the closed line when nothing stands above it (the "No match" state); a new answer closes the line (SPEC A150).
    func settleLine() {
        onMore = showsMore && upperCount == 0
    }

    /// Enter or a click on the closed line: the rest opens under it, the same key or click closes it. The selected result and the reader stay as they are.
    public func toggleRest() -> Bool {
        guard showsMore else { return false }
        withAnimation(WhooshMotion.newRow) { restOpen.toggle() }
        onMore = true
        if !restOpen, selected >= upperCount { selected = max(0, upperCount - 1) }
        return true
    }

    /// Results are on the list pane (the search is open and launched): the reader then shows the selected result, not the inbox row.
    var showsResults: Bool { active && launched }

    /// The first answer (or a new one) is still loading: the reader keeps what it showed.
    var holdsReader: Bool { showsResults && loading }

    /// The result the reader shows while results show: the selected one, or nil (the reader's empty state) when there is none.
    var selectedResult: SearchResult? {
        if onMore && upperCount == 0 { return nil }   // the "No match" state: no result is selected, the closed line is
        let rs = results
        return rs.indices.contains(selected) ? rs[selected] : nil
    }

    var selectedResultId: String? { selectedResult?.row.messageId }

    /// What an action changed on a result since the search ran (the unread dot, the pin): the result's row is a snapshot, so the
    /// toggles (U, pin) read the row with these on top. Cleared when the screen opens fresh.
    /// Observed: the results list reads it per row, so a result's dot and pin redraw on the key (it was ignored, and the dot kept its old state).
    var edited: [String: (unread: Bool?, pinned: Bool?)] = [:]

    /// The selected result's own row, with what actions changed on it: the row "the mail the main window is about" is read from.
    var selectedRow: InboxRow? {
        guard var r = selectedResult?.row else { return nil }
        if let e = edited[r.messageId] { r.unread = e.unread ?? r.unread; r.pinned = e.pinned ?? r.pinned }
        return r
    }

    /// How many of the listed conversations are matches: the engine lists them first, then the related mail, then the rest.
    public var exactCount: Int { results.prefix { $0.part == .match }.count }

    /// How many of the listed conversations are not matches.
    public var closeCount: Int { results.count - exactCount }

    /// How many conversations match (one per thread): the number the count line says.
    public var matchedConversations: Int { allResults.prefix { $0.part == .match }.count }

    /// The count line of the results header (SPEC A150): "N matches · M related", "N matches" alone when nothing is related, "No match · M related" when
    /// nothing matches; "" when both are 0 (the list then says "No match" in the middle and has no count line). The rest is never counted here.
    static func countLine(matches n: Int, related m: Int) -> String {
        switch (n, m) {
        case (0, 0): return ""
        case (_, 0): return matched(n)
        case (0, 1): return Words.text("No match · 1 related")
        case (0, _): return Words.text("No match · %lld related", m)
        case (1, 1): return Words.text("1 match · 1 related")
        case (1, _): return Words.text("1 match · %lld related", m)
        case (_, 1): return Words.text("%lld matches · 1 related", n)
        default: return Words.text("%lld matches · %lld related", n, m)
        }
    }

    /// The count of matches alone ("N matches", "1 match", "No match"): the line when nothing is related (`countLine(matches:related:)` with M = 0, but "No match" for 0).
    static func countLine(_ n: Int) -> String { n == 0 ? Words.text("No match") : matched(n) }

    /// The count of matches in words.
    static func matched(_ n: Int) -> String { n == 1 ? Words.text("1 match") : Words.text("%lld matches", n) }

    public func matchCount(_ thread: String) -> Int { outcome.results.filter { $0.row.threadId == thread }.count }

    static func counted(_ r: Int, _ t: Int) -> String {
        switch (r == 1, t == 1) {
        case (true, true): return Words.text("1 result in 1 thread")
        case (true, false): return Words.text("1 result in %lld threads", t)
        case (false, true): return Words.text("%lld results in 1 thread", r)
        case (false, false): return Words.text("%lld results in %lld threads", r, t)
        }
    }

    /// What the search understood, in words: filters not understood, no exact match, the count of exact matches; the engine's refusal when it refused.
    public var says: String {
        guard launched else { return "" }
        if refusal != nil { return Self.refusedSentence }
        let q = launchedQuery
        var parts = notes
        if q.isEmpty { return "" }
        let exact = outcome.results.filter { !$0.close }
        parts.append(outcome.results.isEmpty ? Words.text("Nothing found")
                     : exact.isEmpty ? Words.text("No match") : Self.counted(exact.count, Set(exact.map(\.row.threadId)).count))
        return parts.joined(separator: " · ")
    }

    /// The lines that change how to read the results (a filter not understood, an exact figure not found).
    public var notes: [String] {
        guard launched else { return [] }
        let q = launchedQuery
        var parts = notice.isEmpty ? [] : [notice]
        parts += q.notUnderstood.map { Words.text("“%@” not understood, searched as words", $0) }
        if outcome.exactMissed, let p = q.phrase { parts.append(Words.text("no exact match for “%@”, showing close meaning", p)) }
        if let line = loadingLine { parts.append(line) }
        return parts
    }

    static func monthStartMs(_ month: Int) -> Int64 {
        var c = Calendar(identifier: .gregorian).dateComponents([.year], from: Date())
        c.month = month + 1; c.day = 1
        return Int64((Calendar(identifier: .gregorian).date(from: c) ?? Date()).timeIntervalSince1970 * 1000)
    }
}
