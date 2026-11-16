import SwiftUI

/// What the Mac's engine says about why a result stands in the list (`SearchResult.found`, `senderMarks`, `subjectMarks`, made once per answer by
/// `SearchEngine.fromHits`, SPEC A149, A152): read here, never worked out again. The phone's row draws these; the socket serves the same facts.
enum SearchPhoneFound {
    /// "text", "attachment" or "thread": where the passage is from; "" for a result found in its subject or sender (two lines) and for the rest.
    static func source(_ r: SearchResult) -> String { r.found?.source.rawValue ?? "" }

    /// The passage line as plain text: what stands before the passage (the file's name, or the earlier mail's sender and date), then the passage,
    /// in quotes for a related mail. "" when the result has none.
    static func line(_ r: SearchResult) -> String {
        guard let f = r.found else { return "" }
        let passage = f.passage.isEmpty ? "" : (f.quoted ? "\u{201C}\(f.passage)\u{201D}" : f.passage)
        return [f.lead, passage].filter { !$0.isEmpty }.joined(separator: " ")
    }

    /// The typed letters drawn in bold on a match, as words: the sender's name, the subject and the passage (the same marks the row draws). A related mail
    /// and a mail of the rest show none.
    static func boldWords(_ r: SearchResult) -> [String] {
        guard r.part == .match else { return [] }
        let name = r.senderMarks.map { String(r.row.senderName[$0]) }
        let subject = r.subjectMarks.map { String(r.row.subject[$0]) }
        let passage = (r.found.map { f in f.quoted ? [] : f.marks.map { String(f.passage[$0]) } }) ?? []
        return name + subject + passage
    }

    /// The text with the marked spans in bold and softly tinted: the phone's tint (`tint`) and the Mac's bold (`ResultRow.emphasised`).
    @MainActor static func marked(_ s: String, _ ranges: [Range<String.Index>], tint: Color) -> AttributedString {
        var a = ResultRow.emphasised(s, ranges, nil)
        for r in ranges { if let ar = Range(r, in: a) { a[ar].backgroundColor = tint } }
        return a
    }
}
