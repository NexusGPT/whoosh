import Foundation

/// The matched-words mark of a phone result: which words of the search are tinted in the subject, the passage and the attachment's
/// name. The words are the Mac's reading (plain words and a quoted figure's words, the small words of a question set aside while a
/// content word is typed; a word matches at a word start through the shared stem), so "supplier" marks "Supply" on both devices.
enum SearchPhoneWords {
    /// The passage line under the subject: the core's snippet, or the row's preview when the engine's passage is the subject itself.
    static func passage(_ r: SearchResult) -> String {
        r.passage.hasPrefix(r.row.subject) || r.passage.isEmpty ? r.row.preview : r.passage
    }

    /// The words to mark, lower case, in the order typed.
    static func terms(_ q: SearchQuery) -> [String] {
        var seen = Set<String>()
        let all = (q.words + (q.phrase?.split(whereSeparator: \.isWhitespace).map(String.init) ?? []))
            .map { $0.lowercased() }
            .filter { $0.rangeOfCharacter(from: .alphanumerics) != nil && seen.insert($0).inserted }
        let content = all.filter { !SearchEngine.stopWords.contains($0.trimmingCharacters(in: .punctuationCharacters)) }
        return content.isEmpty ? all : content
    }

    static func spans(_ terms: [String], in s: String) -> [Range<String.Index>] {
        terms.flatMap { SearchEngine.wordRanges(SearchEngine.stem($0), in: s) }
    }

    /// The marked spans of the subject: the core's own spans when it gave them, else the typed words found in it.
    static func subjectSpans(_ r: SearchResult, _ terms: [String]) -> [Range<String.Index>] {
        r.subjectMarks.isEmpty ? spans(terms, in: r.row.subject) : r.subjectMarks
    }

    /// What the screen marks in one result, as the words themselves (the socket row serves them so a check can read the mark).
    static func markedWords(_ r: SearchResult, in q: SearchQuery) -> [String] {
        let t = terms(q)
        let subject = subjectSpans(r, t).map { String(r.row.subject[$0]) }
        let p = passage(r)
        let body = spans(t, in: p).map { String(p[$0]) }
        var seen = Set<String>()
        return (subject + body).filter { seen.insert($0.lowercased()).inserted }
    }
}
