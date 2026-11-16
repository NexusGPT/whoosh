import Foundation

/// Where the typed letters were found in a matching result (SPEC A149: every match SHOWS why).
public enum EvidencePlace: String, Sendable {
    case subject, sender, address, attachment, text
    /// a row found by meaning alone (no typed letters in it), or a search with no typed words
    case none
}

/// Why a result matched, worked out once when the answer arrives and kept in the `SearchResult` (nothing is computed while a row is drawn).
/// `marks` are character ranges in the string the place names: the subject (`.subject`), the sender's name (`.sender`), or `line` (`.address`, `.text`).
public struct MatchEvidence: Equatable, Sendable {
    public var place: EvidencePlace = .none
    /// the one line a text or address match draws where a preview line would stand; "" for the other places
    public var line: String = ""
    public var marks: [Range<String.Index>] = []

    public init(place: EvidencePlace = .none, line: String = "", marks: [Range<String.Index>] = []) {
        self.place = place; self.line = line; self.marks = marks
    }

    /// The letters in bold, as plain text (what a check reads without a picture): the marked pieces of `source`, the string the place names.
    public func bold(in source: String) -> [String] { marks.map { String(source[$0]) } }
}

/// The typed letters as ranges in a text: case and accent blind, also inside a longer word. Pure; no regular expression.
enum SearchLetters {
    /// The typed words that can match: trimmed, with a letter or a digit in them, each once (case and accent folded).
    static func words(_ typed: [String]) -> [String] {
        var seen = Set<String>(), out: [String] = []
        for w in typed {
            let t = w.trimmingCharacters(in: .whitespacesAndNewlines)
            guard t.rangeOfCharacter(from: .alphanumerics) != nil, seen.insert(fold(t)).inserted else { continue }
            out.append(t)
        }
        return out
    }

    static func fold(_ s: String) -> String { s.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: nil) }

    /// Every occurrence of every word in `text` ("consultant" in "brightconsultants.example", "e" in "é"), in text order, overlaps joined.
    static func ranges(_ words: [String], in text: String) -> [Range<String.Index>] {
        guard !text.isEmpty else { return [] }
        var found: [Range<String.Index>] = []
        for w in words where !w.isEmpty {
            var from = text.startIndex
            while from < text.endIndex, let r = text.range(of: w, options: [.caseInsensitive, .diacriticInsensitive], range: from..<text.endIndex), !r.isEmpty {
                found.append(r); from = r.upperBound
            }
        }
        return merge(found)
    }

    /// Sorted, with ranges that overlap joined into one.
    static func merge(_ rs: [Range<String.Index>]) -> [Range<String.Index>] {
        var out: [Range<String.Index>] = []
        for r in rs.sorted(by: { $0.lowerBound < $1.lowerBound }) {
            if let last = out.last, r.lowerBound < last.upperBound { out[out.count - 1] = last.lowerBound..<max(last.upperBound, r.upperBound) }
            else { out.append(r) }
        }
        return out
    }

    /// Whitespace runs (line breaks too) become one space: a passage is ONE line.
    static func oneLine(_ s: String) -> String {
        s.split(whereSeparator: \.isWhitespace).joined(separator: " ")
    }

    /// One line of the text around the first occurrence, with the letters marked: a little before it (so the start of a narrow row still shows it),
    /// more after it, "…" where the text goes on. nil when the text holds none of the letters.
    static func passage(_ text: String, words: [String], before: Int = 20, after: Int = 90) -> (line: String, marks: [Range<String.Index>])? {
        let flat = oneLine(text)
        guard let first = ranges(words, in: flat).first else { return nil }
        let lo = flat.index(first.lowerBound, offsetBy: -before, limitedBy: flat.startIndex) ?? flat.startIndex
        let hi = max(flat.index(first.lowerBound, offsetBy: after, limitedBy: flat.endIndex) ?? flat.endIndex, first.upperBound)
        let line = (lo > flat.startIndex ? "…" : "") + String(flat[lo..<hi]) + (hi < flat.endIndex ? "…" : "")
        return (line, ranges(words, in: line))
    }

    /// Why this row matched, in the order of SPEC A149: the subject, else the sender's name, else the sender's address (only when the row shows it; the
    /// look S1 does not, so `showsAddress` is false there), else (an attachment's name or text is what the row's own attachment line shows), else ONE
    /// line of the text around the letters (the hit's snippet). A mail whose letters are only in its address, with a snippet that holds none, shows the
    /// address itself on that line, with the letters marked. `.none` when no typed letters are found, whatever the engine said.
    static func evidence(words: [String], subject: String, senderName: String, senderAddress: String, snippet: String,
                         inAttachment: Bool = false, showsAddress: Bool = false) -> MatchEvidence {
        guard !words.isEmpty else { return MatchEvidence() }
        let inSubject = ranges(words, in: subject)
        if !inSubject.isEmpty { return MatchEvidence(place: .subject, marks: inSubject) }
        let inName = ranges(words, in: senderName)
        if !inName.isEmpty { return MatchEvidence(place: .sender, marks: inName) }
        let inAddress = ranges(words, in: senderAddress)
        if showsAddress, !inAddress.isEmpty { return MatchEvidence(place: .address, line: senderAddress, marks: inAddress) }
        if inAttachment { return MatchEvidence(place: .attachment) }
        if let p = passage(snippet, words: words) { return MatchEvidence(place: .text, line: p.line, marks: p.marks) }
        if !inAddress.isEmpty { return MatchEvidence(place: .address, line: senderAddress, marks: inAddress) }
        return MatchEvidence()
    }
}
