import Foundation

/// Search in the fixture core (delta 8, group 2): the recent searches, a small deterministic ranker over the loaded messages
/// and a table of what was read out of attachments. The ranker is a fixture rule, not a model: it exists so the search screen
/// has results with the right shape (where each hit matched, which attachment) on the sample inbox and on the J2 corpus.
final class FixtureSearchBook {
    /// What was read out of an attachment after download (docs/DATA-MAP.md "One search", attachment-content). Invented for the fixture.
    struct AttachmentRead: Sendable { let messageId: String; let name: String; let text: String }
    static let attachmentReads: [AttachmentRead] = [
        // S1 board: a photo on Linea Supply's invoice message, OCR of the delivery note on the pallets
        AttachmentRead(messageId: "msg-s02", name: "warehouse-photo.jpg", text: "Delivery note · Linea Supply · Q1 brochure · 12,000 · 0.84 EUR · pallets in the warehouse"),
        // J2.8: a phrase inside a PDF
        AttachmentRead(messageId: "msg-01", name: "quote-Q1-print-run.pdf", text: "Quotation for the Q1 print run: 12,000 units at 0.84 EUR per unit, valid until 30 June"),
    ]

    /// The attachment texts of the corpus a check loaded (`fixture corpus <path>`: its `attachment_content` entries that were read and hold text): pieces
    /// for the meaning search beside the table above. Before 13 Nov only the table was scored, so a corpus's own file could never be the reason a mail is
    /// related (item bad881fa: the related-by-attachment case judged nothing on every build).
    nonisolated(unsafe) private static var corpusReadsStore: [AttachmentRead] = []
    private static let corpusReadsLock = NSLock()
    static var corpusReads: [AttachmentRead] {
        get { corpusReadsLock.withLock { corpusReadsStore } }
        set { corpusReadsLock.withLock { corpusReadsStore = newValue } }
    }
    /// The pieces the meaning search scores: the table, then the loaded corpus's reads (a file the table already holds is not scored twice).
    static var meaningReads: [AttachmentRead] {
        let own = corpusReads
        return attachmentReads + own.filter { r in !attachmentReads.contains { $0.messageId == r.messageId && $0.name == r.name } }
    }

    /// the two searches the J2 journey leaves behind: the exact query (J2.3, newest) and the filter text (J2.2)
    private(set) var recent: [String] = ["\"0.84 EUR\"", "from Linea Supply before June has attachment"]
    private(set) var last: (query: String, hits: [SearchHit])?
    /// How the last answer was made (the engine's `search/last/stage`): "filtered" pills and no words, "ranked" the meaning order is in, "no_model" none was available
    private(set) var lastStage = ""
    /// round 23 (item bad881fa): the last answer's closeness of each mail found by meaning alone (the model's, the best piece of the mail) and the cut-off a
    /// related mail stands above (`relatedCut`): `search/last/closeness/<message id>` and `search/last/related_cut`, so a check that misses a related mail
    /// says by how much. Numbers only; empty without a model.
    private(set) var lastCloseness: [String: Double] = [:]
    private(set) var lastCut: Double?
    /// `fixture search-hold on`: a search is answered in two steps and the second waits for `off` (the first holds the exact matches, then every other mail by date; stage "words")
    var hold = false
    /// `fixture search-refuse <words>`: every search is refused with these words until `off`
    var refusal: String?
    /// the held second answer, and the stage the first one is in while it waits
    var held: (query: String, hits: [SearchHit])?
    var stageOverride: String?

    /// Enter on a search: newest first, one row per distinct words (case-blind), at most SEARCH_RECENT_MAX.
    func record(_ query: String) throws {
        let q = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !q.isEmpty else { throw CoreError.Invalid(field: "query", reasonWords: "type something to search for") }
        recent.removeAll { $0.caseInsensitiveCompare(q) == .orderedSame }
        recent.insert(q, at: 0)
        if recent.count > WhooshConstants.SEARCH_RECENT_MAX { recent = Array(recent.prefix(WhooshConstants.SEARCH_RECENT_MAX)) }
    }

    /// `from`: the sender's address; `to`: the recipients, each as "name address" (a pill `to:"Claire"` matches either); `labels`: the label names on it.
    /// The three are empty on rows that do not carry them (the smart-folder rules' rows), and a pill that needs one then matches nothing.
    struct Row { let id: String; let subject: String; let body: String; let sender: String; let attachments: [String]; let receivedMs: Int64
        var from = ""; var to: [String] = []; var labels: [String] = [] }

    /// A pill filters EXACTLY (SPEC A143): the mail holds the pill's words, or the pill's day order, or it has an attachment.
    static func passes(_ c: SearchChip, _ r: Row) -> Bool {
        func has(_ hay: String, _ v: String) -> Bool { hay.range(of: v, options: [.caseInsensitive, .diacriticInsensitive]) != nil }
        switch c.kind {
        case .from: return has(r.sender, c.value) || has(r.from, c.value)
        case .to: return r.to.contains { has($0, c.value) }
        case .subject: return has(r.subject, c.value)
        case .attname: return r.attachments.contains { has($0, c.value) }
        case .label:
            let leaf = c.value.components(separatedBy: CharacterSet(charactersIn: "›>")).last?.trimmingCharacters(in: .whitespaces) ?? c.value
            return r.labels.contains { $0.caseInsensitiveCompare(leaf) == .orderedSame }
        case .domain: return SearchEngine.senderIsAt(domain: c.value, address: r.from)   // SPEC A153: `from:@<domain>` keeps the mails whose sender's address is at that domain
        case .attachment: return !r.attachments.isEmpty
        case .before: return c.when.map { r.receivedMs < Int64($0.timeIntervalSince1970 * 1000) } ?? true
        case .after: return c.when.map { r.receivedMs >= Int64($0.timeIntervalSince1970 * 1000) } ?? true
        }
    }

    private static let stop: Set<String> = ["what", "did", "me", "this", "the", "a", "an", "of", "to", "is", "my", "do", "i", "for", "in", "from", "about", "and"]
    /// meaning without a model: a word stands for these (the fixture's stand-in for the vector index)
    private static let near: [String: [String]] = [
        "supplier": ["linea", "printwerk", "atelier", "quote", "offer", "estimate", "price", "invoice"],
        "quote": ["offer", "estimate", "price", "cost"],
        "price": ["cost", "estimate", "eur", "usd", "cents"],
    ]

    /// SPEC A150, the fixture's rule for "related" without a model (the `near` stand-in, whose closeness comes in steps of 0.5): among the mails found by meaning
    /// alone, those whose closeness is at least this many times the mean closeness of those mails for THIS query. A per-search cut-off, never a fixed score; with
    /// fewer than two such mails none stands out, so none is related. With a model the engine's own lift rule decides (`liftRelated`).
    static let relatedShare = 1.25

    /// SPEC A150 with a model: the mails found by meaning alone whose closeness (the vector's cosine, never the fused rank score, which is near 1/(RRF_K + rank)
    /// for every one of them and so lifts none: main88, J2.1 and the shots' `related = 0`) stands at least RELATED_LIFT_MIN deviations above the mean closeness
    /// of those mails for THIS query, the deviation never under RELATED_DEVIATION_FLOOR, at most RELATED_MAX of them, the closest first. The real engine's rule
    /// (docs/CONSTANTS.md) less its RELATED_MIN_POPULATION, which the test corpora (11 to 14 mails) never reach. Returns the indices of `closeness` that are related.
    /// The closeness a related mail must reach for THIS query (liftRelated's rule as one number): the mean plus RELATED_LIFT_MIN deviations, the deviation
    /// never under RELATED_DEVIATION_FLOOR; nil with fewer than two mails.
    static func relatedCut(_ closeness: [Double]) -> Double? {
        guard closeness.count >= 2 else { return nil }
        let mean = closeness.reduce(0, +) / Double(closeness.count)
        let sd = (closeness.map { ($0 - mean) * ($0 - mean) }.reduce(0, +) / Double(closeness.count)).squareRoot()
        return max(mean + WhooshConstants.RELATED_LIFT_MIN * max(sd, WhooshConstants.RELATED_DEVIATION_FLOOR), WhooshConstants.RELATED_MIN_COSINE)
    }

    static func liftRelated(_ closeness: [Double]) -> [Int] {
        guard closeness.count >= 2 else { return [] }
        let mean = closeness.reduce(0, +) / Double(closeness.count)
        let sd = (closeness.map { ($0 - mean) * ($0 - mean) }.reduce(0, +) / Double(closeness.count)).squareRoot()
        let dev = max(sd, WhooshConstants.RELATED_DEVIATION_FLOOR)
        // the closeness floor beside the lift, as the engine's `related_of`: a search typed with letters no mail is about marks no mail
        return closeness.indices.filter { (closeness[$0] - mean) / dev >= WhooshConstants.RELATED_LIFT_MIN && closeness[$0] >= WhooshConstants.RELATED_MIN_COSINE }
            .sorted { closeness[$0] > closeness[$1] }.prefix(WhooshConstants.RELATED_MAX).map { $0 }
    }

    /// The `near` table's closeness of a mail to the typed terms: 0.5 per near word of a typed word found in its subject, text, sender or attachment reads
    /// (the sum the ranking makes without a model).
    static func nearCloseness(_ r: Row, terms ts: [String]) -> Double {
        let reads = attachmentReads.filter { $0.messageId == r.id }.map(\.text).joined(separator: " ")
        let hay = (r.subject + " " + r.body + " " + r.sender + " " + reads).lowercased()
        return ts.reduce(0.0) { s, t in s + Double((near[t] ?? []).filter { hay.contains($0) }.count) * 0.5 }
    }

    /// SPEC A152: an attachment's text read out of the file is a piece of its mail for the meaning search, titled with the mail's subject and the file's name.
    /// Each read of the mails in `rows` is scored by the same ranker as the bodies (`FixtureCore.meaningRanker`, one call, the query vector cached); the best
    /// piece per mail, by message id. Empty with no model, no words, or no read in the rows.
    static func closestPieces(_ words: String, rows: [Row]) -> [String: (score: Float, name: String, text: String)] {
        let subjects = Dictionary(rows.map { ($0.id, $0.subject) }, uniquingKeysWith: { a, _ in a })
        let reads = meaningReads.filter { subjects[$0.messageId] != nil }
        guard !words.isEmpty, !reads.isEmpty else { return [:] }
        let byPiece = Dictionary(reads.map { ("\($0.messageId)#attachment#\($0.name)", $0) }, uniquingKeysWith: { a, _ in a })
        let titles = Dictionary(reads.map { ("\($0.messageId)#attachment#\($0.name)", (subjects[$0.messageId] ?? "") + " · " + $0.name) }, uniquingKeysWith: { a, _ in a })
        let scored = FixtureCore.meaningRanker(words, Array(byPiece.keys).sorted()) { pid in byPiece[pid].map { (title: titles[pid], body: $0.text) } }
        var best: [String: (score: Float, name: String, text: String)] = [:]
        for s in scored {
            guard let r = byPiece[s.id], s.score > (best[r.messageId]?.score ?? -.infinity) else { continue }
            best[r.messageId] = (s.score, r.name, r.text)
        }
        return best
    }

    private static func stem(_ w: String) -> String { w.count > 3 && w.hasSuffix("s") ? String(w.dropLast()) : w }
    static func terms(_ query: String) -> [String] {
        query.lowercased().split(whereSeparator: { !$0.isLetter && !$0.isNumber }).map(String.init).filter { !stop.contains($0) }.map(stem)
    }

    /// Whether `text` holds the typed word `t` (a term of `terms`), as the real engine's word index does (search-text query.rs `token_tier`, `is_wide`): a word of
    /// SUBSTRING_MIN_CHARS letters or more is held when the text holds its letters (the word itself, a word that starts with it, a word that holds it inside), a
    /// shorter word only as a whole word. Case-blind.
    static func holds(_ text: String, _ t: String) -> Bool {
        let hay = text.lowercased()
        guard t.count < WhooshConstants.SUBSTRING_MIN_CHARS else { return hay.contains(t) }
        return hay.split(whereSeparator: { !$0.isLetter && !$0.isNumber }).contains { $0 == t }
    }

    /// The match rule of the search (item 3bb3d817, search-text query.rs `run_ranked` and `meet`): the mail holds EVERY typed word, each one somewhere in its
    /// subject, its sender's name or address, its text, an attachment's name or a read attachment's text (`meaningReads`). False with no typed word.
    static func holdsEvery(_ ts: [String], _ r: Row) -> Bool {
        guard !ts.isEmpty else { return false }
        let files = meaningReads.filter { $0.messageId == r.id }.map(\.text)
        return ts.allSatisfy { t in
            holds(r.subject, t) || holds(r.sender, t) || holds(r.from, t) || holds(r.body, t) || r.attachments.contains { holds($0, t) } || files.contains { holds($0, t) }
        }
    }

    /// The text from a little before the first occurrence of any term (case-blind, inside words too), at most SEARCH_SNIPPET_CHARS long; nil when no term is in it.
    static func passage(_ text: String, terms: [String]) -> String? {
        let first = terms.compactMap { text.range(of: $0, options: .caseInsensitive) }.min { $0.lowerBound < $1.lowerBound }
        guard let f = first else { return nil }
        let lo = text.index(f.lowerBound, offsetBy: -30, limitedBy: text.startIndex) ?? text.startIndex
        return String(text[lo...].prefix(WhooshConstants.SEARCH_SNIPPET_CHARS))
    }

    /// UTF-8 byte ranges of every occurrence of `term` in `text`, case-blind.
    static func ranges(of term: String, in text: String) -> [TextRange] {
        var out: [TextRange] = []
        let hay = text.lowercased()
        var from = hay.startIndex
        while let r = hay.range(of: term, range: from..<hay.endIndex) {
            out.append(TextRange(start: UInt32(hay.utf8.distance(from: hay.utf8.startIndex, to: r.lowerBound)), end: UInt32(hay.utf8.distance(from: hay.utf8.startIndex, to: r.upperBound))))
            from = r.upperBound
        }
        return out
    }

    /// `meaning`: the vector search's messages, closest first (MeaningSearch.rank); empty = no model, the `near` table stands in.
    /// `everyMail`: the answer is EVERY mail, ranked (SPEC A141), the way the real engine answers: the mails with a typed word in them first
    /// (best match first, then newest), then every other mail by closeness of meaning, then the mails with no vector by date. No cut by score,
    /// no count limit. Without it the answer is the matching mails alone, best first, at most SEARCH_DEFAULT_HITS (the smart-folder rules read that).
    func run(_ query: String, rows: [Row], meaning: [(id: String, score: Float)] = [], attachmentHits: [SearchHit] = [], everyMail: Bool = false) throws -> [SearchHit] {
        let q = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !q.isEmpty else { throw CoreError.Invalid(field: "query", reasonWords: "type something to search for") }
        // the pills filter exactly and the remaining words rank what is kept; a query of pills alone lists the kept mails, newest first
        let parsed = SearchQuery.typed(q)
        lastCloseness = [:]; lastCut = nil
        let rows = parsed.chips.isEmpty ? rows : rows.filter { r in parsed.chips.allSatisfy { Self.passes($0, r) } }
        let ts = Self.terms(([parsed.words.joined(separator: " ")] + (parsed.phrase.map { [$0] } ?? [])).joined(separator: " "))
        let pillsOnly = ts.isEmpty && !parsed.chips.isEmpty
        let meaningRank = Dictionary(uniqueKeysWithValues: meaning.enumerated().map { ($1.id, $0 + 1) })
        var scored: [(tier: Int, rank: Double, date: Int64, hit: SearchHit)] = []
        for r in rows {
            let reads = Self.attachmentReads.filter { $0.messageId == r.id }
            // the attachment index's row for this message (core/crates/attachment-index): a word or name hit; a file only close in meaning (`.related`) is no match
            let indexed = attachmentHits.first { $0.messageId == r.id && $0.matchedIn != .related }
            var score = 0.0
            var wordHit = indexed != nil            // a typed word is in the mail (subject, attachment name or content, body): an exact match
            var nearScore = 0.0                     // the `near` table's share: the stand-in for the vector index, never an exact match
            var subjectRanges: [TextRange] = []
            var nameHit: String?
            var contentHit: String?
            // The real engine's rule (item 3bb3d817, search-text query.rs `run_ranked`: "Several typed words: every one must be held"): a mail is a match only
            // when it holds EVERY typed word, each in its subject, its sender's name or address, its text, a file's name or a read file's text (`holdsEvery`).
            // A mail that holds some of them and not all gets no word share: it is no match, and the `near` share below may still make it related.
            let every = indexed != nil || Self.holdsEvery(ts, r)
            for t in ts where every {
                let rs = Self.holds(r.subject, t) ? Self.ranges(of: t, in: r.subject) : []
                if !rs.isEmpty { score += 3; subjectRanges += rs; wordHit = true }
                if Self.holds(r.sender, t) || Self.holds(r.from, t) { score += 1; wordHit = true }   // the sender's name or address: below a word of the subject or the text
                if Self.holds(r.attachments.joined(separator: " "), t), let n = r.attachments.first(where: { $0.lowercased().contains(t) }) { score += 2; nameHit = nameHit ?? n; wordHit = true }
                if let c = reads.first(where: { Self.holds($0.text, t) }) { score += 2; contentHit = contentHit ?? c.name; wordHit = true }
                if Self.holds(r.body, t) { score += 2; wordHit = true }
            }
            if every, !ts.isEmpty { wordHit = true }   // a word held only in a corpus file's read text (`meaningReads`) still makes the match
            for t in ts {
                for n in meaning.isEmpty ? (Self.near[t] ?? []) : [] {
                    let hay = (r.subject + " " + r.body + " " + r.sender + " " + reads.map(\.text).joined(separator: " ")).lowercased()
                    if hay.contains(n) { score += 0.5; nearScore += 0.5 }
                    if contentHit == nil, let c = reads.first(where: { $0.text.lowercased().contains(n) }) { contentHit = c.name; score += 0.5; nearScore += 0.5 }
                }
            }
            if let a = indexed {   // a content hit outranks a name hit; the snippet of a name hit carries the J2.8 note
                if case .attachmentContent = a.matchedIn { score += 2; contentHit = contentHit ?? a.attachmentName } else { score += 1; nameHit = nameHit ?? a.attachmentName }
            }
            // reciprocal-rank fusion of the vector list (RRF_K, the real core's rule): a message only the vector search found is a meaning-only hit
            if let rank = meaningRank[r.id] { score += 1.0 / Double(WhooshConstants.RRF_K + rank) }
            if pillsOnly { score = 1; wordHit = true }
            guard score > 0 || everyMail else { continue }
            let matched: MatchedIn
            var attachment: String?
            if !subjectRanges.isEmpty { matched = .subjectWords(ranges: subjectRanges.sorted { $0.start < $1.start }) }
            else if let n = nameHit { matched = .attachmentName; attachment = n }
            else if let c = contentHit { matched = .attachmentContent; attachment = c }
            else if everyMail, wordHit { matched = .subjectWords(ranges: []) }   // a word of the body: the contract has no body place, and it is no meaning-only hit
            else { matched = .meaningOnly }
            // a mail whose text holds a typed word: the passage around its first occurrence (the real engine's snippet, SPEC A149); any other mail: its first characters
            var snippet = Self.passage(r.body, terms: ts) ?? String(r.body.prefix(WhooshConstants.SEARCH_SNIPPET_CHARS))
            if case .attachmentName = matched, let a = indexed, a.attachmentName == attachment { snippet = a.snippet }
            // a match inside an attachment's text: the passage is the FILE's, around the typed word (SPEC A152, P1: the row's third line is the file's
            // passage with the letters in bold), never the body's first characters (main92 J2.9 par-attachment-result-shows-its-passage-line)
            if case .attachmentContent = matched, let name = attachment {
                let text = reads.first { $0.name == name }?.text ?? (indexed?.attachmentName == name ? indexed?.snippet : nil)
                if let t = text { snippet = Self.passage(t, terms: ts) ?? String(t.prefix(WhooshConstants.SEARCH_SNIPPET_CHARS)) }
            }
            // a name hit on a file the index could not read carries its state words as the note (delta 10, J2.8)
            let note: String? = { if case .attachmentName = matched, let a = indexed, a.attachmentName == attachment { return a.snippet } else { return nil } }()
            let hit = SearchHit(messageId: r.id, score: score, snippet: snippet, matchedIn: matched, attachmentName: attachment, attachmentNote: note)
            if !everyMail { scored.append((0, -score, r.receivedMs, hit)); continue }
            // the ranking: 0 a typed word is in it (best first), 1 closer in meaning (the vector list's order; the `near` stand-in without a model), 2 no vector (newest first)
            // a hit the answer labels as a match (a word, or with no model the `near` stand-in's read of a file, `.attachmentContent`) stands with the
            // matches: listed after the related mail it would split the parts (main97 iPhone: parts match, match, related, match)
            let labelledMatch: Bool = { if case .meaningOnly = matched { return false }; return true }()
            if wordHit || labelledMatch { scored.append((0, -score, r.receivedMs, hit)) }
            else if let rank = meaningRank[r.id] { scored.append((1, Double(rank), r.receivedMs, hit)) }
            else if nearScore > 0 { scored.append((1, -nearScore, r.receivedMs, hit)) }
            else { scored.append((2, 0, r.receivedMs, hit)) }
        }
        if everyMail, !meaning.isEmpty {
            // with a model: a mail's closeness is its best piece, the body's vector or an attachment's (SPEC A152); a related mail found by its attachment
            // names the file and shows the file's passage (the row's third line, P1)
            let bodies = Dictionary(meaning.map { ($0.id, Double($0.score)) }, uniquingKeysWith: { a, _ in a })
            let pieces = Self.closestPieces(parsed.words.joined(separator: " "), rows: rows)
            let close = scored.indices.filter { i in
                guard case .meaningOnly = scored[i].hit.matchedIn, scored[i].tier == 1 else { return false }
                return bodies[scored[i].hit.messageId] != nil || pieces[scored[i].hit.messageId] != nil
            }
            func closeness(_ id: String) -> Double { max(bodies[id] ?? -.infinity, pieces[id].map { Double($0.score) } ?? -.infinity) }
            // related: lifted by the model's closeness, or standing out in the `near` table as without a model (the test corpora hold 11 to 14 mails, where a
            // lift of RELATED_LIFT_MIN deviations is hard to reach; the table keeps the corpus's designed meaning match related on any model)
            let byId = Dictionary(rows.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
            let nears = close.map { byId[scored[$0].hit.messageId].map { Self.nearCloseness($0, terms: ts) } ?? 0 }
            let stand = nears.filter { $0 > 0 }
            let nearCut = stand.count >= 2 ? Self.relatedShare * stand.reduce(0, +) / Double(stand.count) : .infinity
            let lifted = Set(Self.liftRelated(close.map { closeness(scored[$0].hit.messageId) }) + nears.indices.filter { nears[$0] > 0 && nears[$0] >= nearCut })
            lastCloseness = Dictionary(close.map { (scored[$0].hit.messageId, closeness(scored[$0].hit.messageId)) }, uniquingKeysWith: { a, _ in a })
            lastCut = Self.relatedCut(close.map { closeness(scored[$0].hit.messageId) })
            for k in lifted.sorted() {
                let i = close[k], h = scored[i].hit
                var piece: (score: Float, name: String, text: String)? = nil
                if let p = pieces[h.messageId], Double(p.score) > (bodies[h.messageId] ?? -.infinity) { piece = p }
                scored[i].hit = SearchHit(messageId: h.messageId, score: h.score, snippet: piece.map { String($0.text.prefix(WhooshConstants.SEARCH_SNIPPET_CHARS)) } ?? h.snippet,
                                          matchedIn: .related, attachmentName: piece?.name ?? h.attachmentName, attachmentNote: h.attachmentNote)
                // the related part stands above every other mail, closest first: the list shows the run before the first mail of the rest (SearchMacModel.results)
                scored[i].rank = -10 - closeness(h.messageId)
            }
        } else if everyMail {
            let close = scored.indices.filter { i in if case .meaningOnly = scored[i].hit.matchedIn { return scored[i].tier == 1 } else { return false } }
            if close.count >= 2 {
                let mean = close.map { Double(scored[$0].hit.score) }.reduce(0, +) / Double(close.count)
                for i in close where Double(scored[i].hit.score) >= Self.relatedShare * mean {
                    let h = scored[i].hit
                    scored[i].hit = SearchHit(messageId: h.messageId, score: h.score, snippet: h.snippet, matchedIn: .related, attachmentName: h.attachmentName, attachmentNote: h.attachmentNote)
                }
            }
        }
        scored.sort { $0.tier != $1.tier ? $0.tier < $1.tier : $0.rank != $1.rank ? $0.rank < $1.rank : $0.date > $1.date }
        let hits = everyMail ? scored.map(\.hit) : Array(scored.prefix(WhooshConstants.SEARCH_DEFAULT_HITS).map(\.hit))
        last = (q, hits)
        lastStage = pillsOnly ? "filtered" : meaning.isEmpty ? "no_model" : "ranked"
        return hits
    }

    static func matchedWord(_ m: MatchedIn) -> String {
        switch m { case .subjectWords: return "subject"; case .attachmentName: return "attachment_name"; case .attachmentContent: return "attachment_content"; case .meaningOnly: return "meaning"; case .related: return "related"; case .senderWords: return "sender"; case .textWords: return "text" }
    }

    /// `search/last/<field>` and `search/last/hit/<n>/<field>`
    func value(_ p: [String]) -> Any? {
        guard p.count >= 3, let l = last else { return nil }
        if p.count == 3 {
            switch p[2] { case "query": return l.query; case "count": return l.hits.count; case "stage": return stageOverride ?? lastStage
            case "related": return l.hits.filter { if case .related = $0.matchedIn { return true } else { return false } }.count   // SPEC A150: how many of the answer are related mail
            case "related_cut": return lastCut
            default: return nil }
        }
        if p.count == 4, p[2] == "closeness" { return lastCloseness[p[3]] }
        guard p.count == 5, p[2] == "hit", let n = Int(p[3]), l.hits.indices.contains(n) else { return nil }
        let h = l.hits[n]
        switch p[4] {
        case "message_id": return h.messageId
        case "matched_in": return Self.matchedWord(h.matchedIn)
        case "attachment_name": return h.attachmentName
        case "subject_ranges": if case .subjectWords(let rs) = h.matchedIn { return rs.map { [Int($0.start), Int($0.end)] } } else { return nil }
        default: return nil
        }
    }
}

extension FixtureCore {
    /// Everyone in the mail with how many mails came from them (received mail) and how many were sent to them (the recipients of the owner's own sent
    /// mail), the owner's own addresses left out, most mail first (the engine's `people/*` reads, SearchSuggest.swift `PeopleRead`).
    static func everyone(in messages: [FixtureFile.Message], deleted: Set<String> = [], owner: Set<String> = []) -> [PersonSuggestion] {
        var names: [String: String] = [:], fromCount: [String: Int] = [:], toCount: [String: Int] = [:]
        for m in messages where !deleted.contains(m.id) {
            if m.outgoing ?? false {
                for a in (m.to ?? []) { toCount[a.lowercased(), default: 0] += 1 }
            } else {
                let a = m.from.lowercased()
                fromCount[a, default: 0] += 1
                if names[a] == nil, !m.sender.isEmpty { names[a] = m.sender }
            }
        }
        func name(_ a: String) -> String {
            if let n = names[a] { return n }
            let local = a.split(separator: "@").first.map(String.init) ?? a
            return local.split(whereSeparator: { ".-_".contains($0) }).map { $0.prefix(1).uppercased() + $0.dropFirst() }.joined(separator: " ")
        }
        return Set(fromCount.keys).union(toCount.keys).filter { !owner.contains($0) }
            .map { PersonSuggestion(name: name($0), address: $0, fromCount: fromCount[$0] ?? 0, toCount: toCount[$0] ?? 0) }
            .sorted { ($0.fromCount + $0.toCount, $1.name) > ($1.fromCount + $1.toCount, $0.name) }
    }

    /// The people the field offers for the letters typed: the start of the whole name, of any word of it, or of the address, accents and case folded; at most 8.
    static func people(in messages: [FixtureFile.Message], deleted: Set<String> = [], owner: Set<String> = [], prefix: String) -> [PersonSuggestion] {
        let fold = { (t: String) in t.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: nil) }
        let letters = fold(prefix.trimmingCharacters(in: .whitespaces))
        guard !letters.isEmpty else { return [] }
        let wordStarts = { (n: String) in fold(n).split(whereSeparator: { " .-_@".contains($0) }).contains { $0.hasPrefix(letters) } }
        return everyone(in: messages, deleted: deleted, owner: owner)
            .filter { fold($0.name).hasPrefix(letters) || wordStarts($0.name) || fold($0.address).hasPrefix(letters) }
            .prefix(8).map { $0 }
    }

    private func json(_ any: Any) -> String? {
        guard let d = try? JSONSerialization.data(withJSONObject: any, options: [.fragmentsAllowed]), let s = String(data: d, encoding: .utf8) else { return nil }
        return s
    }
    private func row(_ p: PersonSuggestion) -> [String: Any] { ["name": p.name, "address": p.address, "from_count": p.fromCount, "to_count": p.toCount] }

    /// `people/prefix/<letters>` (an array, at most 8 rows), `people/address/<address>` (one row or null), `people/count`, as the real engine serves them.
    func peopleState(path: String) -> String? {
        let owner = Set(fx.accounts.map { $0.address.lowercased() })
        return lock.withLock {
            let rest = { (p: String) in String(path.dropFirst(p.count)) }
            if path.hasPrefix("people/prefix/") { return json(Self.people(in: messages, deleted: deleted, owner: owner, prefix: rest("people/prefix/")).map(row)) }
            if path.hasPrefix("people/address/") {
                let a = rest("people/address/").lowercased()
                return json(Self.everyone(in: messages, deleted: deleted, owner: owner).first { $0.address == a }.map(row) ?? NSNull())
            }
            if path == "people/count" { return String(Self.everyone(in: messages, deleted: deleted, owner: owner).count) }
            return nil
        }
    }
}

extension FixtureSearchBook {
    /// Sent mail the corpus does not carry, so a person seen only as a recipient exists (the people reads and the `to:` pill): three mails the owner sent to
    /// fay.lund@fixture-example.com, who never wrote. They are `outgoing`, read, with no tab: no inbox row, tab figure, unread figure, mailbox, contact or
    /// smart-folder row (`FixtureCore.listable`). The reads that see them: `search` (they are mail), `search/last/*`, `people/*`, `message(messageId:)`, the thread.
    static func sentSeed(account: String, from address: String) -> [FixtureFile.Message] {
        (1...3).map { n in
            var m = FixtureFile.Message(id: "sent-seed-\(n)", sender: "", from: address, account: account, subject: ["Draft schedule for the workshop", "Re: workshop rooms", "Invoice question"][n - 1],
                                        preview: "Sent mail seeded by the test engine", time: "Yesterday", unread: false, pinned: false,
                                        attachments: nil, thread_count: nil, trackers_blocked: nil, tab: nil, body: ["Hello Fay, here is the plan."], quoted: nil)
            m.to = ["fay.lund@fixture-example.com"]; m.outgoing = true; m.thread = "thr-sent-seed-\(n)"
            return m
        }
    }
}

extension FixtureCore {
    /// Takes the three seeded sent mails out of a running core and nothing else: the rows and the per-message state keyed by their ids. (A second
    /// `resetMessages` would also rebuild what the flow seeding and the arrivals added after the first one, so it is not the same corpus minus the seed.)
    func removeSentSeed() {
        lock.withLock {
            let ids = Set(messages.filter { $0.id.hasPrefix("sent-seed-") }.map(\.id))
            messages.removeAll { ids.contains($0.id) }
            order.removeAll { ids.contains($0) }
            read.subtract(ids)
            for id in ids { receivedAt[id] = nil; account[id] = nil; pinned[id] = nil }
        }
    }
}

extension FixtureCore {
    /// The sender domains of the received mail, for the letters typed (the engine's `senderDomains(prefix:)`, SPEC A153): each domain with how many different
    /// addresses wrote from it and how many mails, the owner's own addresses left out, a domain that starts with the letters (accents and case folded), most mail
    /// first, at most 8.
    static func domainsOfSenders(in messages: [FixtureFile.Message], deleted: Set<String> = [], owner: Set<String> = [], prefix: String) -> [SenderDomain] {
        let fold = { (t: String) in t.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: nil) }
        let letters = fold(prefix.trimmingCharacters(in: CharacterSet(charactersIn: "@ ")))
        guard !letters.isEmpty else { return [] }
        var people: [String: Set<String>] = [:], mails: [String: Int] = [:]
        for m in messages where !deleted.contains(m.id) && !(m.outgoing ?? false) {
            let a = m.from.lowercased()
            guard !owner.contains(a), let at = a.lastIndex(of: "@") else { continue }
            let d = String(a[a.index(after: at)...])
            guard !d.isEmpty, fold(d).hasPrefix(letters) else { continue }
            people[d, default: []].insert(a); mails[d, default: 0] += 1
        }
        return mails.keys.sorted { (mails[$0] ?? 0, $1) > (mails[$1] ?? 0, $0) }.prefix(8)
            .map { SenderDomain(domain: $0, people: UInt32(people[$0]?.count ?? 0), mails: UInt32(mails[$0] ?? 0)) }
    }
}
