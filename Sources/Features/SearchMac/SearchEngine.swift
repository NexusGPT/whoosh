import Foundation

/// The three parts of the list, in the order the engine answers them (SPEC A150).
public enum SearchPart: String, Sendable { case match, related, rest }

/// The third line of a result row (docs/design/boards/search-r3-parts, P1): where the passage is from, the passage, and what stands before it.
public struct SearchFound: Equatable, Sendable {
    public enum Source: String, Sendable { case text, attachment, thread }
    public let source: Source
    /// the passage, one line; for a match the typed letters are in `marks`, for a related mail it is drawn in quotes
    public let passage: String
    public let marks: [Range<String.Index>]
    /// a related mail's passage is quoted; a match's has its letters in bold
    public let quoted: Bool
    /// `.attachment`: the file's name (and the engine's note, "page 2")
    public let fileName: String?
    /// `.thread`: the sender and the date of the mail that holds the passage ("Marc Petit · 12 Oct"), and the sender alone
    public let who: String?
    public let sender: String?
    public init(source: Source, passage: String, marks: [Range<String.Index>] = [], quoted: Bool, fileName: String? = nil, who: String? = nil, sender: String? = nil) {
        self.source = source; self.passage = passage; self.marks = marks; self.quoted = quoted; self.fileName = fileName; self.who = who; self.sender = sender
    }
    /// what stands before the passage, as plain text (the file's name, or the earlier mail's sender and date); "" for a text passage
    public var lead: String { fileName ?? who ?? "" }
}

/// One message that matched, as the results list draws it.
public struct SearchResult: Equatable, Sendable {
    public let row: InboxRow
    public let score: Int
    /// the passage shown under the subject, around the first match
    public let passage: String
    /// exact-phrase spans in `passage` (the figure the owner quoted); empty when nothing matched exactly
    public let highlights: [Range<String.Index>]
    /// plain-word spans in `passage`, drawn softer than an exact match
    public let marks: [Range<String.Index>]
    /// plain-word and exact spans inside the subject, for the tint the result row draws
    public let subjectMarks: [Range<String.Index>]
    /// the attachment whose name carries the match (nil when the match is not in an attachment's name) and the spans in it
    public let attachmentName: String?
    public let attachmentMarks: [Range<String.Index>]
    /// where the match sits: the core's `matched_in` word (subject, attachment-name, attachment-content, meaning); "body" only on the local engine's path
    public let matchedIn: String
    /// the words beside the attachment's name: "in the attachment" for a match read out of the file, the core's note for a file
    /// it could not read (delta 10, J2.8); nil when the row names no attachment or the match is in the name itself
    public var attachmentSays: String? = nil
    /// Which of the list's three parts the result belongs to (SPEC A150): a match (typed letters are in it), a related mail (none of them, clearly above the
    /// search's average closeness), or the rest (behind the closed line "All other mail"). The engine's order is matches, related, rest; nothing re-orders it.
    public var part: SearchPart = .match
    /// not a match: found by closeness of meaning alone (a related mail or one of the rest). The matches come first.
    public var close: Bool { part != .match }
    /// The third line of the row (SPEC A152, P1): the passage that made the mail a result when it is in the text, an attachment or an earlier mail of the
    /// thread; nil for a result found in its subject or sender (two lines) and for a mail of the rest. Made once when the answer arrives.
    public var found: SearchFound? = nil
    /// the mail that holds the passage (the engine's `hit.messageId`); the row is the conversation's own row, which is another mail when the passage is earlier in the thread
    public var hitMessageId = ""
    /// the typed letters inside the sender's name, for the bold the row draws in its first line (SPEC A149)
    public var senderMarks: [Range<String.Index>] = []
    /// why the result matched: where the typed letters are and the line to draw (SearchLetters.evidence), kept from the moment the answer arrived
    public var evidence = MatchEvidence()
}

/// The local search: filters alone and an exact quoted figure run over the spine's rows, because they are not an engine search by design.
/// Plain words go to the core's search (`SearchEngine.fromHits`, SearchHits.swift); a core that refuses them is said on the screen, never
/// answered from here (SPEC A141).
public enum SearchEngine {
    public struct Outcome: Equatable, Sendable {
        public var results: [SearchResult] = []
        public var exactMissed = false
    }

    /// SPEC A153: the sender's address is at this domain, or at a subdomain of it ("lorvan-hotels.example" holds "claire@mail.lorvan-hotels.example", never "claire@notlorvan-hotels.example").
    static func senderIsAt(domain: String, address: String) -> Bool {
        let d = domain.trimmingCharacters(in: CharacterSet(charactersIn: "@ ")).lowercased()
        guard !d.isEmpty, let at = address.lastIndex(of: "@") else { return false }
        let host = address[address.index(after: at)...].lowercased()
        return host == d || host.hasSuffix("." + d)
    }

    /// Every chip holds for the row. The words of the search are not asked here.
    static func passes(_ chips: [SearchChip], _ r: InboxRow, beforeMs: (Int) -> Int64, names: (InboxRow) -> [String]) -> Bool {
        let contains = { (hay: String, v: String) in hay.range(of: v, options: [.caseInsensitive, .diacriticInsensitive]) != nil }
        return chips.allSatisfy { c in
            switch c.kind {
            case .from: return contains(r.senderName, c.value) || contains(r.senderAddress, c.value)
            case .subject: return contains(r.subject, c.value)
            case .attname: return names(r).contains { contains($0, c.value) }
            case .label:
                let leaf = c.value.components(separatedBy: CharacterSet(charactersIn: "›>")).last?.trimmingCharacters(in: .whitespaces) ?? c.value
                return r.labelNames.contains { $0.caseInsensitiveCompare(leaf) == .orderedSame }
            case .domain: return senderIsAt(domain: c.value, address: r.senderAddress)
            case .attachment: return r.attachmentCount > 0
            case .to: return true   // the rows hold no recipients: the engine filters on them, and what it sends is already filtered
            case .before:
                if let when = c.when { return r.receivedAtMs < Int64(when.timeIntervalSince1970 * 1000) }
                return SearchQuery.monthIndex(c.value).map { r.receivedAtMs < beforeMs($0) } ?? true
            case .after:
                if let when = c.when { return r.receivedAtMs >= Int64(when.timeIntervalSince1970 * 1000) }
                return SearchQuery.monthIndex(c.value).map { r.receivedAtMs >= beforeMs($0) } ?? true
            }
        }
    }

    /// `names` gives a message's attachment names (the core's message read); an attachment-name match is what the row names.
    public static func run(_ q: SearchQuery, over rows: [InboxRow], beforeMs: (Int) -> Int64, names: (InboxRow) -> [String] = { _ in [] }) -> Outcome {
        guard !q.isEmpty else { return Outcome() }
        var filtered = rows.filter { passes(q.chips, $0, beforeMs: beforeMs, names: names) }
        var out = Outcome()
        var seen = Set<String>()
        var words = q.words.map { $0.lowercased() }.filter { $0.rangeOfCharacter(from: .alphanumerics) != nil && seen.insert($0).inserted }
        let content = words.filter { !stopWords.contains($0.trimmingCharacters(in: .punctuationCharacters)) }
        if !content.isEmpty { words = content }   // a question's filler ("what did ... me this") finds nothing by itself
        if let p = q.phrase {
            let exact = filtered.filter { !exactRanges(p, in: text($0)).isEmpty }
            if exact.isEmpty {
                out.exactMissed = true
                words += p.lowercased().split(whereSeparator: \.isWhitespace).map(String.init)
            } else { filtered = exact }
        }
        let missed = out.exactMissed
        let nameWords = words + q.chips.filter { $0.kind == .attname }.map(\.value)
        out.results = filtered.compactMap { r in
            let hay = text(r)
            let attach = names(r)
            let nameHit = attach.first { n in nameWords.contains { !wordRanges(stem($0.lowercased()), in: n).isEmpty } }
            let marks = words.flatMap { wordRanges(stem($0), in: hay) }
            if !words.isEmpty, marks.isEmpty, nameHit == nil, q.phrase == nil || missed { return nil }
            let exact = missed ? [] : q.phrase.map { exactRanges($0, in: hay) } ?? []
            let score = exact.count * 10 + Set(marks.map { hay[$0].lowercased() }).count + (nameHit == nil ? 0 : 1)
            let subj = r.subject
            let subjectMarks = words.flatMap { wordRanges(stem($0), in: subj) } + (missed ? [] : q.phrase.map { exactRanges($0, in: subj) } ?? [])
            let nameMarks = nameHit.map { n in nameWords.flatMap { wordRanges(stem($0.lowercased()), in: n) } } ?? []
            return SearchResult(row: r, score: score, passage: hay, highlights: exact, marks: marks,
                                subjectMarks: subjectMarks, attachmentName: nameHit, attachmentMarks: nameMarks,
                                // a quoted figure found in the text is a match in words ("body"), never "meaning": its words are the phrase, not `marks` (main88 J2.3)
                                matchedIn: nameHit != nil ? "attachment-name" : !subjectMarks.isEmpty ? "subject" : !marks.isEmpty || !exact.isEmpty ? "body" : "meaning")
        }
        .sorted { $0.score != $1.score ? $0.score > $1.score : $0.row.receivedAtMs > $1.row.receivedAtMs }
        return out
    }

    /// the small words of a question in plain English; they never decide a match while a content word is typed
    static let stopWords: Set<String> = ["what", "which", "who", "when", "where", "how", "did", "does", "do", "is", "are", "was", "were", "me", "my", "i", "you", "we",
                                         "this", "that", "the", "a", "an", "of", "to", "in", "on", "for", "and", "or", "with", "about", "any", "all", "last", "from"]

    static func text(_ r: InboxRow) -> String { r.preview.isEmpty ? r.subject : "\(r.subject) · \(r.preview)" }

    /// "supplier" finds "Supply", "quotes" finds "quote": the shared stem, matched at a word start.
    static func stem(_ w: String) -> String {
        for suffix in ["iers", "ier", "ies", "ing", "es", "ed", "s", "y"] where w.count > suffix.count + 2 && w.hasSuffix(suffix) {
            return String(w.dropLast(suffix.count))
        }
        return w
    }

    static func wordRanges(_ stem: String, in hay: String) -> [Range<String.Index>] {
        var out: [Range<String.Index>] = []
        var from = hay.startIndex
        while let r = hay.range(of: stem, options: [.caseInsensitive, .diacriticInsensitive], range: from..<hay.endIndex) {
            if r.lowerBound == hay.startIndex || !hay[hay.index(before: r.lowerBound)].isLetter && !hay[hay.index(before: r.lowerBound)].isNumber {
                out.append(r)
            }
            from = r.upperBound
        }
        return out
    }

    /// A quoted figure matches only whole: "84 EUR" is not inside "0.84 EUR".
    static func exactRanges(_ phrase: String, in hay: String) -> [Range<String.Index>] {
        var out: [Range<String.Index>] = []
        var from = hay.startIndex
        let joins: Set<Character> = [".", ",", "-", "/"]
        while let r = hay.range(of: phrase, options: .caseInsensitive, range: from..<hay.endIndex) {
            let before = r.lowerBound == hay.startIndex ? nil : hay[hay.index(before: r.lowerBound)]
            let after = r.upperBound == hay.endIndex ? nil : hay[r.upperBound]
            let glued = { (c: Character?) in c.map { $0.isLetter || $0.isNumber || joins.contains($0) } ?? false }
            if !glued(before), !(after.map { $0.isLetter || $0.isNumber } ?? false) { out.append(r) }
            from = r.upperBound
        }
        return out
    }
}
