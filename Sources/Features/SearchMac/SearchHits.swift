import Foundation

/// The core's answer to plain words (Command.search, event SearchResults), turned into the rows the list draws.
/// Chips are the screen's own filters and apply on top; the order is the core's: the exact matches, then every other mail by closeness of meaning.
extension SearchEngine {
    /// The words the core is asked for: plain words only (chips filter locally; a quoted figure stays on the local exact path).
    static func coreWords(_ q: SearchQuery) -> String? {
        guard q.phrase == nil else { return nil }
        let words = q.words.filter { $0.rangeOfCharacter(from: .alphanumerics) != nil }
        return words.isEmpty ? nil : words.joined(separator: " ")
    }

    /// What the engine is asked (SPEC A143): the pills in their colon forms, then the remaining words (`from:"Claire Martin" before:2026-06-01 revised price`).
    /// nil for a quoted figure, which is not an engine search by design, and for an empty query.
    static func engineQuery(_ q: SearchQuery) -> String? {
        guard q.phrase == nil else { return nil }
        let words = q.words.filter { $0.rangeOfCharacter(from: .alphanumerics) != nil }
        let parts = q.chips.map(\.canonical) + words
        return parts.isEmpty ? nil : parts.joined(separator: " ")
    }

    /// The rows of the hits, from ONE core read in the engine's order: a hit's mail can sit in any folder (Sent, Archive, beyond the inbox's first
    /// page), so the inbox rows never stand for it. A hit the core does not return is dropped. `read` is the seam a test counts the calls through.
    @MainActor static func rowsOfHits(_ hits: [SearchHit], core: WhooshCore, read: (([String]) -> [InboxRow])?) -> [InboxRow] {
        var seen = Set<String>()
        let ids = hits.map(\.messageId).filter { seen.insert($0).inserted }
        guard !ids.isEmpty else { return [] }
        return read.map { $0(ids) } ?? core.rowsForMessages(messageIds: ids)
    }

    /// `heads`: the row each conversation shows in the inbox, by thread id. A hit whose mail is another mail of that conversation (an earlier message
    /// of the thread holds the passage) is listed as the conversation's row, and its third line says whose mail the passage is from (SPEC A152).
    static func fromHits(_ q: SearchQuery, hits: [SearchHit], rows: [InboxRow], beforeMs: (Int) -> Int64, names: (InboxRow) -> [String],
                         engineFiltered: Bool = false, heads: [String: InboxRow] = [:]) -> Outcome {
        let byId = Dictionary(rows.map { ($0.messageId, $0) }, uniquingKeysWith: { a, _ in a })
        var out = Outcome()
        let typed = q.words.map { $0.lowercased() } + q.chips.filter { $0.kind == .attname }.map(\.value)
        let stems = SearchLetters.words(q.words.map { stem($0.lowercased()) })
        let letters = SearchLetters.words(q.words)   // the typed letters, folded once per answer; the evidence of every row is made here, never while a row is drawn
        for h in hits {
            // `engineFiltered`: the pills went to the engine in the query, which kept exactly the mails that pass them; the phone filters here instead
            guard let r = byId[h.messageId], engineFiltered || passes(q.chips, r, beforeMs: beforeMs, names: names) else { continue }
            var subjectMarks: [Range<String.Index>] = []
            var label = "meaning"
            var part = SearchPart.match
            // An if-chain, not a switch over every case: a case the engine adds later (the sender, the text) counts as a match in words ("text") and
            // keeps compiling; `.meaningOnly` is "all other mail" and `.related` is a mail close to the search with none of its letters.
            if case .subjectWords(let ranges) = h.matchedIn {
                label = "subject"
                subjectMarks = ranges.compactMap { span(r.subject, $0) }
            } else if case .attachmentName = h.matchedIn { label = "attachment-name"
            } else if case .attachmentContent = h.matchedIn { label = "attachment-content"
            } else if case .meaningOnly = h.matchedIn { label = "meaning"; part = .rest
            } else if case .related = h.matchedIn { label = "related"; part = .related
            } else { label = "text" }
            // the conversation row when the passage sits in another mail of the thread (the row stays one per conversation)
            let head = heads[r.threadId].flatMap { $0.messageId == r.messageId ? nil : $0 }
            let shownRow = head ?? r
            // the core names one place; a word typed that is also in an attachment's name still shows it (the board's first row)
            let name = h.attachmentName ?? names(shownRow).first { n in typed.contains { !wordRanges(stem($0), in: n).isEmpty } }
            var nameMarks: [Range<String.Index>] = []
            if let n = name {
                // the typed word itself where the name holds it ("redlines" in "term-sheet-v2-redlines.docx"), its stem only where the name holds
                // another form of it (lead, 13 Nov, picture read-135: "redlin" alone was tinted while the subject showed the whole word)
                nameMarks = label == "attachment-content" ? [n.startIndex..<n.endIndex] : typed.flatMap { w -> [Range<String.Index>] in
                    let whole = wordRanges(w, in: n)
                    return whole.isEmpty ? wordRanges(stem(w), in: n) : whole
                }
            }
            // why it matched: a row found by meaning alone has no typed letters in it, so it keeps its normal row and no bold
            var evidence = MatchEvidence(), senderMarks: [Range<String.Index>] = []
            if part == .match {
                // the engine's own subject ranges and the typed letters found here (inside longer words too) are both the subject's bold
                let engineMarks = head == nil ? subjectMarks : []   // a range of the hit mail's subject says nothing about the conversation row's subject
                func why(_ ws: [String]) -> MatchEvidence {
                    let inSubject = SearchLetters.merge(engineMarks + SearchLetters.ranges(ws, in: shownRow.subject))
                    if !inSubject.isEmpty { return MatchEvidence(place: .subject, marks: inSubject) }
                    return SearchLetters.evidence(words: ws, subject: shownRow.subject, senderName: shownRow.senderName, senderAddress: shownRow.senderAddress,
                                                  snippet: h.snippet, inAttachment: label.hasPrefix("attachment"))
                }
                evidence = why(letters)
                if evidence.place == .none, stems != letters { evidence = why(stems) }   // the engine also finds "Supply" for "supplier"
                if evidence.place == .subject { subjectMarks = evidence.marks } else if head != nil { subjectMarks = [] }
                if evidence.place == .sender { senderMarks = evidence.marks }
            }
            var result = SearchResult(row: shownRow, score: Int(h.score * 10), passage: h.snippet, highlights: [], marks: [], subjectMarks: subjectMarks,
                                      attachmentName: name, attachmentMarks: nameMarks, matchedIn: label,
                                      attachmentSays: name == nil ? nil : attachmentSays(h, label), senderMarks: senderMarks, evidence: evidence)
            result.part = part
            result.hitMessageId = h.messageId
            result.found = found(h, part: part, label: label, evidence: evidence, letters: letters, stems: stems, hitRow: r, thread: head != nil)
            out.results.append(result)
        }
        return out
    }

    /// The third line of a result (SPEC A152, P1), made once per answer. A mail of the rest has none. A match found in its subject or its sender's name
    /// has none (two lines, the letters in bold there). Otherwise: a file (paperclip and name, then the passage), an earlier mail of the thread (arrow,
    /// its sender and date, then the passage), or the text (the passage alone). A match's passage holds the typed letters in bold; a related mail's is quoted.
    static func found(_ h: SearchHit, part: SearchPart, label: String, evidence: MatchEvidence, letters: [String], stems: [String], hitRow: InboxRow, thread: Bool) -> SearchFound? {
        guard part != .rest else { return nil }
        let quoted = part == .related
        if part == .match, evidence.place == .subject || evidence.place == .sender { return nil }
        var passage = "", marks: [Range<String.Index>] = []
        if part == .match, evidence.place == .text || evidence.place == .address {
            passage = evidence.line; marks = evidence.marks
        } else {
            let flat = SearchLetters.oneLine(h.snippet)
            if part == .match, let p = SearchLetters.passage(h.snippet, words: letters) ?? SearchLetters.passage(h.snippet, words: stems) {
                passage = p.line; marks = p.marks
            } else if flat.count > foundMax {
                passage = String(flat.prefix(foundMax)) + "…"
            } else { passage = flat }
        }
        let note = h.attachmentNote.flatMap { $0.isEmpty ? nil : $0 }
        if let file = h.attachmentName {
            // an attachment-name hit's snippet is the engine's note on the file ("could not be read"), so it is the passage only when it is a note
            return SearchFound(source: .attachment, passage: passage, marks: marks, quoted: quoted && !passage.isEmpty,
                               fileName: part == .related ? [file, note].compactMap { $0 }.joined(separator: " · ") : file)
        }
        guard !passage.isEmpty else { return nil }
        if thread { return SearchFound(source: .thread, passage: passage, marks: marks, quoted: quoted, who: threadWho(hitRow), sender: hitRow.senderName) }
        return SearchFound(source: .text, passage: passage, marks: marks, quoted: quoted)
    }

    /// the longest passage a related mail's line keeps (the row cuts it to its width; this keeps the stored line short)
    static let foundMax = 160

    /// "Marc Petit · 12 Oct": the sender and the date of the mail that holds the passage, as the row's own time reads.
    static func threadWho(_ r: InboxRow) -> String { r.senderName + " · " + RowFacts.time(r.receivedAtMs) }

    /// Beside the file's name: where in the file the words were found, or why only its name was searched.
    static func attachmentSays(_ h: SearchHit, _ label: String) -> String? {
        if let note = h.attachmentNote, !note.isEmpty { return note }
        return label == "attachment-content" ? Words.text("in the attachment") : nil
    }

    /// A core range is UTF-8 byte offsets into the subject.
    static func span(_ s: String, _ r: TextRange) -> Range<String.Index>? {
        let u = s.utf8
        guard r.start < r.end, Int(r.end) <= u.count else { return nil }
        let a = u.index(u.startIndex, offsetBy: Int(r.start)), b = u.index(u.startIndex, offsetBy: Int(r.end))
        return a.samePosition(in: s).flatMap { lo in b.samePosition(in: s).map { lo..<$0 } }
    }
}

/// Receives the Search stream: the hits of the words the screen asked for.
final class SearchHitSink: EventSink, @unchecked Sendable {
    private let handle: @MainActor (String, [SearchHit]) -> Void
    init(_ handle: @escaping @MainActor (String, [SearchHit]) -> Void) { self.handle = handle }
    func onEvent(event: CoreEvent) {
        guard case .searchResults(let query, let hits) = event else { return }
        if Thread.isMainThread { MainActor.assumeIsolated { handle(query, hits) } }
        else { Task { @MainActor in handle(query, hits) } }
    }
}
