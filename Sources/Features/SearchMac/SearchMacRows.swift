import Foundation

/// What the socket verb `rows` reads on the search screen (ax-ids under `mac-search.`).
@MainActor
enum SearchMacRows {
    static var s: SearchMacModel { .shared }

    static var folderRows: [ControlRow] {
        s.syncFolders()
        return s.folders.enumerated().map { i, f in
            ["ax": "mac-search.sidebar.smart-folders.\(i)", "id": f.id, "title": f.name, "count": f.count, "rule": f.rule, "threads": f.threads, "says": f.says,
             "edit": "mac-search.sidebar.smart.\(i).edit"]
        }
    }

    /// Whether each result's subject and attachment name is cut, by ax id (`mac-search.results.<i>.subject|attachment`): read from the sizes the hidden
    /// copies of SearchResultsList took, through the `labels` verb's own reader and switch (FrameProbe), only when a client asks for the rows.
    /// A text that was not judged (no copy mounted, not on screen) has no entry, and its row serves null, never a made-up false.
    static func cutFacts() -> [String: Bool] {
        #if os(macOS)
        guard !s.results.isEmpty, let w = WindowBridge.mainWindow() else { return [:] }
        FrameProbe.switchLabelsOn()
        var out: [String: Bool] = [:]
        for r in FrameProbe.labels(in: w)["labels"] as? [ControlRow] ?? [] {
            guard let ax = r["ax"] as? String, ax.hasPrefix("mac-search.results."), r["text"] is String,
                  let need = r["need_w"] as? CGFloat, let box = r["box_w"] as? CGFloat else { continue }
            out[ax] = isCut(need: need, box: box)
        }
        return out
        #else
        return [:]
        #endif
    }

    /// The string the evidence's bold ranges point into.
    static func evidenceSource(_ r: SearchResult) -> String {
        switch r.evidence.place {
        case .subject: return r.row.subject
        case .sender: return r.row.senderName
        default: return r.evidence.line
        }
    }

    /// One result's fact from `cutFacts`: true or false, or null when that text was not judged.
    /// `shown` false (a row with no attachment): nothing is there to cut, so false.
    static func cutValue(_ cut: [String: Bool], _ i: Int, _ part: String, shown: Bool = true) -> Any {
        if !shown { return false }
        if let b = cut["mac-search.results.\(i).\(part)"] { return b }
        return NSNull()
    }

    /// The text needs more width than the box it got (half a point is the pixel grid's rounding, as FrameProbe.wrapTolPt).
    static func isCut(need: CGFloat, box: CGFloat) -> Bool { need > box + 0.5 }

    static var all: [String: (AppModel) throws -> [ControlRow]] { [
        "mac-search.bar": { _ in [["ax": "mac-search.bar", "text": s.barText, "says": s.says, "naming": s.naming, "launched": s.launched, "loading": s.loading, "reasks": s.reaskCount, "active": s.active, "focused": s.holds, "cursor": s.fieldFocused,
                                   "dropdown": s.dropdownVisible, "draft": SearchQuery.draft(of: s.text), "owner": s.holds ? "search" : "other", "released_by": s.releasedBy]] },
        "mac-search.dropdown": { _ in
            s.dropdownVisible ? s.suggestions.enumerated().map { i, r in
                ["ax": "mac-search.dropdown.\(i)", "kind": r.kind.rawValue, "title": r.title, "detail": r.detail, "account": r.accountId ?? NSNull(),
                 "highlighted": i == s.highlight, "hovered": i == s.hovered, "keyed": s.keyed && i == s.highlight, "chip": r.chip?.kind.rawValue ?? NSNull(), "group": r.group]
            } : []
        },
        // the engine refused the search: the sentence the pane says and the engine's own words under it; absent when it did not
        "mac-search.refused": { _ in
            guard s.paneShowsResults, let words = s.refusal else { return [] }
            return [["ax": "mac-search.refused", "text": SearchMacModel.refusedSentence, "words": words]]
        },
        "mac-search.skeleton": { _ in [["ax": "mac-search.skeleton", "loading": s.loading, "held": s.holdSkeleton]] },
        "mac-search.chips": { _ in
            s.naming ? [] : s.query.chips.enumerated().map { i, c in
                ["ax": "mac-search.chips.\(i)", "kind": c.kind.rawValue, "value": c.value, "title": c.title, "remove": "mac-search.chips.\(i).remove",
                 "resolves_to": c.when.map { ISO8601DateFormatter().string(from: $0) } ?? NSNull()]
            }
        },
        "mac-search.results": { _ in
            let book = AccountBook.shared.accounts()
            let cut = cutFacts()
            func acc(_ id: String) -> AccountBook.Entry? { book.first { $0.id == id } }
            // built in typed parts: one literal of this size is more than the compiler type-checks in reasonable time
            return s.results.enumerated().map { (i: Int, r: SearchResult) -> [String: Any] in
                func orNull(_ v: Any?) -> Any { v ?? NSNull() }
                let mark = AccountBook.mark(of: r.row.accountId, in: book)
                let highlights: [String] = r.highlights.isEmpty ? r.subjectMarks.map { String(r.row.subject[$0]) } : r.highlights.map { String(r.passage[$0]) }
                var row: [String: Any] = ["ax": "mac-search.results.\(i)", "id": r.row.messageId, "thread": r.row.threadId, "account": r.row.accountId]
                row["account_name"] = acc(r.row.accountId)?.name ?? r.row.accountId
                row["colour"] = mark.colour
                row["account_icon"] = mark.iconValue
                row["sender"] = r.row.senderName
                row["subject"] = r.row.subject
                row["passage"] = r.passage
                row["attachment"] = orNull(r.attachmentName)
                row["attachment_says"] = orNull(r.attachmentSays)
                row["matched_in"] = r.matchedIn
                row["highlights"] = highlights
                row["marks"] = r.marks.count
                row["selected"] = (s.picked.isEmpty ? i == s.selected : s.picked.contains(r.row.threadId)) && !s.onMore   // J5.6: ⌘A selects every result
                row["chips"] = r.row.labelNames
                // why the row matched (SPEC A149): where the typed letters are (subject, sender, address, attachment, text, none for a row found by meaning),
                // the line a text or address match draws, and the letters in bold as plain text (in the subject, the sender's name or that line)
                row["evidence_in"] = r.evidence.place.rawValue
                row["evidence_line"] = r.evidence.line.isEmpty ? NSNull() as Any : r.evidence.line as Any
                row["evidence_bold"] = r.evidence.bold(in: evidenceSource(r))
                row["close"] = r.close
                // the list's part (SPEC A150: match, related, rest), the third line's passage and where it is from (SPEC A152: text, attachment with the file's
                // name, thread with the earlier mail's sender), the passage's bold letters (a match) and whether it is quoted (related); null when the row has no third line
                row["part"] = r.part.rawValue
                var sourceName: String? = nil
                var passageBold: [String]? = nil
                if let f = r.found {
                    if f.source == .attachment { sourceName = f.fileName } else if f.source == .thread { sourceName = f.sender }
                    passageBold = f.marks.map { String(f.passage[$0]) }
                }
                row["passage_line"] = orNull(r.found?.passage)
                row["source"] = orNull(r.found?.source.rawValue)
                row["source_name"] = orNull(sourceName)
                row["source_who"] = orNull(r.found?.who)
                row["passage_bold"] = orNull(passageBold)
                row["passage_quoted"] = orNull(r.found?.quoted)
                row["hit_id"] = r.hitMessageId
                // cut: the subject / the attachment's name does not fit its box (a row with no attachment cuts none); fade_ms: the rows crossfade in over this long
                row["subject_truncated"] = cutValue(cut, i, "subject")
                row["attachment_truncated"] = cutValue(cut, i, "attachment", shown: r.attachmentName != nil)
                row["fade_ms"] = WhooshConstants.SEARCH_SKELETON_FADE_MS
                return row
            }
        },
        // what the results pane draws around the rows (SearchResultsPane, SearchResultsList): the count line, the line before the close-in-meaning group, the index note, the footer
        "mac-search.header": { _ in
            s.paneShowsResults ? [["ax": "mac-search.header", "title": Words.text("Results"), "count": SearchMacModel.countLine(matches: s.matchedConversations, related: s.relatedCount), "matches": s.matchedConversations,
                       "related": s.relatedCount, "rest": s.restCount, "stage": s.stage, "tail_pending": s.tailPending]] : []
        },
        "mac-search.sections": { _ in
            guard s.paneShowsResults, !s.allResults.isEmpty || s.tailPending else { return [] }
            let titles = SearchResultsList.titles(related: s.relatedCount, rest: s.restCount, stage: s.stage, tailPending: s.tailPending)
            return titles.enumerated().map { i, t in ["ax": "mac-search.sections.\(i)", "title": t] }
        },
        // the closed line "All other mail · N" (SPEC A150): its count, whether it is open, whether the selection is on it (it then shows the Return key and the bottom bar
        // says "Show all other mail" or "Hide all other mail"); absent while the rest is not known (stage "words") or empty
        "mac-search.more": { _ in
            guard s.paneShowsResults, s.showsMore else { return [] }
            return [["ax": "mac-search.more", "title": SearchResultsList.moreTitle, "count": s.restCount, "open": s.restOpen, "selected": s.onMore, "key": s.onMore ? "Enter" : NSNull(),
                     "listed": s.results.count - s.upperCount]]
        },
        // while the meaning index is still being built: how far it is, above the rows (absent when it is not building)
        "mac-search.index-note": { _ in
            guard s.paneShowsResults, !s.results.isEmpty, let line = SearchResultsList.indexLine(IndexProgressModel.shared.status) else { return [] }
            return [["ax": "mac-search.index-note", "text": line]]
        },
        "mac-search.footer": { _ in
            s.paneShowsResults && !s.results.isEmpty ? [["ax": "mac-search.footer", "text": SearchResultsList.footerLine]] : []
        },
        "mac-search.loading-accounts": { _ in
            s.launched ? s.loadingAccounts.enumerated().map { i, a in ["ax": "mac-search.loading-accounts.\(i)", "id": a.id, "name": a.name] } : []
        },
        "mac-search.recent": { _ in s.recents.enumerated().map { i, q in ["ax": "mac-search.recent.\(i)", "text": q] } },
        "mac-search.sidebar.smart-folders": { _ in folderRows },
        "mac-search.sidebar.smart": { _ in folderRows },
        "mac-search.save-as-smart-folder.field": { m in
            [["ax": "mac-search.save-as-smart-folder.field", "open": s.naming, "text": s.naming ? m.typed : "", "says": s.fieldSays]]
        },
        // the one plain sentence the results pane shows about the last act (a result opened that has gone); absent when there is none
        "mac-search.notice": { _ in s.notice.isEmpty ? [] : [["ax": "mac-search.notice", "text": s.notice]] },
        "mac-search.passage": { _ in s.openedMessageId.map { [["ax": "mac-search.passage", "id": $0]] } ?? [] },
        // J5.6: the list's label picker (overlay `mac-main.label`) opened on the selected results: the typed name, and whether it offers to create it
        "mac-search.label-field": { m in
            guard !s.picked.isEmpty, m.overlay == Overlay.label, case .label(let ids)? = InboxListState.shared.pending else { return [] }
            return [["ax": "mac-search.label-field", "text": m.typed, "offers_create": s.offersCreate(m.typed), "messages": ids.count]]
        },
        // the list's undo capsule (ListChrome, `mac-main.list.undo`) as the search screen shows it over its results: its words and whether Z undoes
        "mac-search.undo.bar": { _ in
            guard s.paneShowsResults, let words = InboxListState.shared.undoWords else { return [] }
            return [["ax": "mac-search.undo.bar", "text": words, "undoable": InboxListState.shared.undoAvailable]]
        },
    ] }
}
