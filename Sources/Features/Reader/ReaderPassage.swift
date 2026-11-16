import Foundation

/// What the selected search result hands the reader (docs/SPEC.md A152, owner 2026-11-11: "it's subject+body+attachment and they should be all related";
/// "+thread too btw"; card answer "P1 The passage on the row", whose drawing shows the reader opened at the passage, marked): the mail that holds the
/// passage (the hit's own mail, which can be an EARLIER mail of the conversation than the one the list row shows), the passage and the typed letters, and where
/// it is from (the text, or an attachment's file name). One small value; it is made from the result when the reader settles on it (`ReaderFocus.passage`) and
/// is nil when the search is closed or the mail shown is not a result.
struct ReaderPassage: Equatable, Sendable {
    enum Source: String, Sendable { case text, attachment, thread }

    /// The mail that holds the passage: the result's `hitMessageId` (the row's own mail when the engine named none).
    let messageId: String
    /// The conversation the result stands for (the row's thread).
    let threadId: String
    /// The passage as the row shows it: one line, at most about 120 characters, cut with "…" where the mail goes on.
    let text: String
    /// The typed letters of a match, as the row's bold shows them; none for a related mail (the whole passage is then the marked part).
    let letters: [String]
    let source: Source
    /// The attachment's file name for `.attachment`.
    let fileName: String?

    /// A passage is marked inside the mail's text unless it is an attachment's (that one rings the file's tile and marks no text).
    var marksText: Bool { source != .attachment && !text.isEmpty }

    /// The result's hand-over; nil for a result with no third line (found in its subject or its sender, or a mail of the rest): nothing is marked then.
    static func of(_ r: SearchResult) -> ReaderPassage? {
        guard let f = r.found else { return inText(r) }
        let source: Source
        switch f.source {
        case .text: source = .text
        case .attachment: source = .attachment
        case .thread: source = .thread
        }
        // the letters are the row's own bold, read back from the passage: the same pieces, found again by `SearchLetters.ranges` (the one ranges rule)
        let letters = SearchLetters.words(f.marks.map { String(f.passage[$0]) })
        let file = source == .attachment ? (r.attachmentName ?? f.fileName) : nil
        return ReaderPassage(messageId: r.hitMessageId.isEmpty ? r.row.messageId : r.hitMessageId, threadId: r.row.threadId, text: f.passage,
                             letters: letters, source: source, fileName: file)
    }

    /// A match found in its subject or its sender (two lines on the row, no third line) still opens the reader at the typed letters in the mail's text
    /// when the text holds them: the drawing P1's "After Enter" (RD_M0: a subject match, its text marked "revised price") and SPEC A152 ("opened at the
    /// mail and the passage that matched"). The passage is one line of the hit's text around the letters (SearchLetters.passage, the row's own rule);
    /// nil when the text holds none of them (nothing to mark; the row's bold in the subject says why). Round 19, d1ffe36c clause 2.
    static func inText(_ r: SearchResult) -> ReaderPassage? {
        guard r.part == .match else { return nil }
        let typed = piece(r.row.subject, r.subjectMarks) + piece(r.row.senderName, r.senderMarks)
        let letters = SearchLetters.words(typed)
        guard !letters.isEmpty, let p = SearchLetters.passage(r.passage, words: letters) else { return nil }
        return ReaderPassage(messageId: r.hitMessageId.isEmpty ? r.row.messageId : r.hitMessageId, threadId: r.row.threadId, text: p.line,
                             letters: letters, source: .text, fileName: nil)
    }

    /// The marked pieces of `s`; a range that is not a valid range of `s` is skipped (never a trap).
    private static func piece(_ s: String, _ ranges: [Range<String.Index>]) -> [String] {
        ranges.compactMap { r in
            guard r.lowerBound >= s.startIndex, r.upperBound <= s.endIndex, r.lowerBound.samePosition(in: s.unicodeScalars) != nil,
                  r.upperBound.samePosition(in: s.unicodeScalars) != nil else { return nil }
            return String(s[r])
        }
    }

    /// The hand-over for what the main window shows now: the selected result while results show, else nil (the search closed, an inbox mail open).
    @MainActor static func current(_ m: AppModel) -> ReaderPassage? {
        guard m.resultsShow, let r = SearchMacModel.shared.selectedResult else { return nil }
        return of(r)
    }

    func isFor(_ t: ReaderThread) -> Bool { threadId == t.threadId }

    /// The passage is in the mail the reader has open (the newest of the conversation).
    func holdsOpenMail(_ t: ReaderThread) -> Bool { isFor(t) && messageId == t.messageId }

    /// The index of the earlier message that holds the passage, in `t.earlier` order; nil when it is the open mail or no earlier message of the thread has that id.
    func earlierIndex(in t: ReaderThread) -> Int? {
        guard isFor(t), messageId != t.messageId else { return nil }
        return t.earlier.firstIndex { $0?.id == messageId }
    }

    /// The words of the mail's attachment list that name the file ("offer-v3.pdf" or "offer-v3.pdf (210 KB)").
    func file(in words: [String]) -> String? {
        guard source == .attachment, let name = fileName, !name.isEmpty else { return nil }
        return words.first { $0 == name || $0.hasPrefix(name + " ") }
    }

    /// The page's passage for the open mail's body; nil when this passage is not in the open mail, is an attachment's, or the body does not hold it.
    @MainActor func page(in t: ReaderThread, content: MessageBodyContent) -> PassageMark.Page? {
        guard marksText, holdsOpenMail(t) else { return nil }
        return PassageMemo.shared.page(id: content.id, passage: text, letters: letters, text: content.plainText)
    }

    /// The marked ranges in the text an earlier message shows.
    @MainActor func ranges(inEarlier id: String, shown: String) -> [Range<String.Index>] {
        guard marksText, id == messageId else { return [] }
        return PassageMemo.shared.ranges(id: id, passage: text, letters: letters, text: shown)
    }

    /// True when this passage's file is the one this tile shows: the reader-owned look reads it (the tile's accent ring), never the attachment store's selection.
    @MainActor static func rings(messageId: String, name: String) -> Bool {
        guard let p = ReaderFocus.shared.passage, p.source == .attachment, p.messageId == messageId, let f = p.fileName else { return false }
        return name == f
    }

    /// The row the checks read (`mac-main.reader.passage`): whether a passage is marked in the mail the reader shows, its text and its source.
    @MainActor static func reading(_ m: AppModel, _ t: ReaderThread) -> [String: Any] {
        guard let p = ReaderFocus.shared.passage, p.isFor(t) else {
            return ["marked": false, "text": "", "source": "", "file": NSNull(), "mail_id": NSNull(), "in_open_mail": false, "earlier_index": NSNull(),
                    "pieces": [String](), "letters": [String](), "ring": NSNull()]
        }
        let open = p.holdsOpenMail(t), earlier = p.earlierIndex(in: t)
        var shown: String?
        var files: [String] = []
        if open { shown = t.shownBody(m)?.plainText; files = t.attachmentNames }
        else if let i = earlier, let fact = t.earlier[i] { shown = EarlierMessageBlock.shownText(fact, expanded: true); files = fact.attachments }
        let pieces = p.marksText ? shown.map { PassageMark.pieces(passage: p.text, letters: p.letters, in: $0) } ?? [] : []
        let ring = p.file(in: files)
        return ["marked": !pieces.isEmpty || ring != nil, "text": p.text, "source": p.source.rawValue, "file": p.fileName ?? NSNull(), "mail_id": p.messageId,
                "in_open_mail": open, "earlier_index": earlier ?? NSNull(), "pieces": pieces, "letters": p.letters, "ring": ring ?? NSNull()]
    }
}
