import Foundation

/// What a search asks of the core, and what it shows. The same reading as the Mac's field: plain words go to the core
/// (`Command.search`, the hits come back on the Search stream), filters alone and a quoted figure run on the local engine, and a core
/// that refuses falls back to it. Typing never waits: the rows on screen stay until the new answer replaces them.
@MainActor
extension SearchPhoneModel {
    func subscribe() {
        listenForMeaning()
        if resetToken == nil {
            resetToken = NotificationCenter.default.addObserver(forName: Notification.Name("whoosh.fixture.reset"), object: nil, queue: .main) { _ in
                MainActor.assumeIsolated { if SearchPhoneModel.shared.active { _ = SearchPhoneModel.shared.close() } }
            }
        }
        guard searchSub == nil else { return }
        searchSub = app.core.subscribe(stream: .search, sink: SearchHitSink { [weak self] q, h in self?.receive(q, h) })
    }

    /// The Search stream's answer: kept by the words asked for, and the rows are built from it when it is the answer awaited.
    func receive(_ query: String, _ found: [SearchHit]) {
        hitCache[query] = found; hitRowsCache[query] = nil
        guard query == awaiting else { return }
        hits = found; hitsFor = query; awaiting = ""
        if active { build() }
    }

    /// One run of the search for the field and filters as they stand.
    func refresh() {
        guard active else { return }
        let q = engineQuery(includingSender: false)
        SearchQueryVector.typed(engineQuery(includingSender: true))      // the meaning query's vector, made while the words are typed
        readDomainOffers()                                                // "From domain <domain>" for the word being typed (SPEC A153)
        if holdSkeleton, !isIdleInput { results = []; everyResult = []; phase = .searching; return }   // the screen's own hold: the first answer stays unlisted
        if isIdleInput {
            results = []; everyResult = []; restOpen = false; candidates = []; notes = []; exactMissed = false; phase = .idle; awaiting = ""
            return
        }
        restOpen = false                                                  // every new search closes "All other mail" (SPEC A150)
        guard let words = SearchEngine.coreWords(q) else { awaiting = ""; build(); return }
        if hitCache[words] != nil { build(); return }
        awaiting = words
        do { _ = try app.core.run(command: .search(query: words)) } catch { awaiting = ""; build(); return }
        // the fixture core answers inline: `receive` already built the rows. A real core answers later: the rows shown stay, the phase says why.
        if awaiting == words { phase = .searching }
    }

    /// The meaning index grows in the background: the launched words are asked again so meaning hits join, with no skeleton on a re-ask.
    func listenForMeaning() {
        guard !meaningListening else { return }
        meaningListening = true
        MeaningSearch.onUpdate = { Task { @MainActor in SearchPhoneModel.shared.meaningGrew() } }
    }

    func meaningGrew() {
        guard active, !reaskPending else { return }
        reaskPending = true
        Task { @MainActor in
            await Task.yield()
            reaskPending = false
            hitCache = [:]; hitRowsCache = [:]
            guard !isIdleInput, let words = SearchEngine.coreWords(engineQuery(includingSender: false)) else { return }
            awaiting = words
            do { _ = try app.core.run(command: .search(query: words)) } catch { awaiting = "" }
        }
    }

    /// The rows of one answer's hits: one core read per answer, kept until the answer is replaced.
    func hitRows(_ words: String, _ hits: [SearchHit]) -> [InboxRow] {
        if let r = hitRowsCache[words] { return r }
        let r = SearchEngine.rowsOfHits(hits, core: app.core, read: hitRowsRead)
        hitRowsCache[words] = r
        return r
    }

    /// Builds the rows from the hits in hand (plain words) or from the local engine, then applies the account and sender filters.
    func build() {
        guard !holdSkeleton else { return }
        let q = engineQuery(includingSender: false)
        var outcome = SearchEngine.Outcome()
        if q.isEmpty {
            outcome.results = corpus.map(Self.wrapped)                    // an account or a sender alone: the filter is the search
        } else if let words = SearchEngine.coreWords(q), let found = hitCache[words] {
            // the conversation's row is its newest mail, and a passage in an earlier mail says whose it is (SPEC A152, as the Mac: `heads`)
            let heads = Dictionary(corpus.map { ($0.threadId, $0) }, uniquingKeysWith: { a, b in a.receivedAtMs >= b.receivedAtMs ? a : b })
            outcome = SearchEngine.fromHits(q, hits: found, rows: hitRows(words, found), beforeMs: { SearchMacModel.monthStartMs($0) }, names: { self.attachmentNames($0) },
                                            heads: heads)
        } else {
            outcome = SearchEngine.run(q, over: corpus, beforeMs: { SearchMacModel.monthStartMs($0) }, names: { self.attachmentNames($0) })
        }
        exactMissed = outcome.exactMissed
        let byAccount = outcome.results.filter { filters.accountId == nil || $0.row.accountId == filters.accountId }
        var seen = Set<String>()
        candidates = byAccount.filter { seen.insert($0.row.threadId).inserted }
        stage = SearchMacModel.decoded(app.core.state(path: "search/last/stage"))
        let listed = filters.sender.map { s in candidates.filter { Self.sender($0.row, contains: s) } } ?? candidates
        applyParts(listed)
        notes = notesFor(q)
        phase = everyResult.isEmpty ? .none : .results
    }

    /// The lines that change how to read the results: a filter word not understood, an exact figure not found, accounts still loading.
    private func notesFor(_ q: SearchQuery) -> [String] {
        var parts = q.notUnderstood.map { Words.text("“%@” not understood, searched as words", $0) }
        if exactMissed, let p = q.phrase { parts.append(Words.text("no exact match for “%@”, showing close meaning", p)) }
        if let line = SearchMacModel.shared.loadingLine { parts.append(line) }
        return parts
    }

    /// A message's attachment names, read from the core once and kept for the screen's life.
    func attachmentNames(_ r: InboxRow) -> [String] {
        guard r.attachmentCount > 0 else { return [] }
        if let n = nameCache[r.messageId] { return n }
        let n = app.core.message(messageId: r.messageId)?.attachmentNames ?? []
        nameCache[r.messageId] = n
        return n
    }

    /// A row with no words behind it (a filter alone): its preview is the passage.
    static func wrapped(_ r: InboxRow) -> SearchResult {
        SearchResult(row: r, score: 0, passage: r.preview, highlights: [], marks: [], subjectMarks: [], attachmentName: nil,
                     attachmentMarks: [], matchedIn: "filter")
    }

    /// The Mac's rule for a sender filter: the name or the address contains the value, case and accents ignored.
    static func sender(_ r: InboxRow, contains v: String) -> Bool {
        let has = { (hay: String) in hay.range(of: v, options: [.caseInsensitive, .diacriticInsensitive]) != nil }
        return has(r.senderName) || has(r.senderAddress)
    }
}
