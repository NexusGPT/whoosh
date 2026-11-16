import Foundation

/// The `rows` answers for the reader's regions: the facts the pane draws, in drawing order. A region whose
/// data the core does not serve yet throws, so a check reading it is "not judged", never green by absence.
@MainActor
enum ReaderProbe {
    static var state: ReaderState { .shared }

    static func thread(_ m: AppModel) throws -> ReaderThread {
        // the pane shows the shown thread at rest until Enter opens it, so its regions answer for that too
        ReaderFocus.shared.catchUp(m)
        guard let t = ReaderState.open(m) ?? ReaderFocus.shared.thread(m, resting: true) else { throw ControlFailure("no conversation is open in the reader") }
        return t
    }

    static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "mac-main.reader.subject": { m in let t = try thread(m); return [["id": t.messageId, "thread_id": t.threadId, "resting": t.resting, "subject": t.subject]] },
            "mac-main.reader.sender": { m in
                let t = try thread(m)
                return [["ax": "mac-main.reader.sender", "id": t.messageId, "name": t.sender, "address": t.open.address,
                         "time": t.time, "contact_id": m.core.contact(address: t.open.address)?.id ?? NSNull(),
                         "contact_messages": ReaderActions.senderMessageCount(m, t)]]
            },
            "mac-main.reader.actions": { m in
                _ = try thread(m)
                return ReaderAction.all.map { ["ax": "mac-main.reader.key.\($0.id)", "key": $0.key, "command": $0.command, "label": $0.label, "where": "header"] }
            },
            "mac-main.reader.strip": { m in
                _ = try thread(m)
                let shown = state.headerHover || MacShellState.shared.focusedPane == .reader
                let quick = QuickReplyModel.shared.isOpen
                return ReaderStripItem.all.map { ["ax": "mac-main.reader.strip.\($0.id)", "label": $0.label, "key": $0.key, "command": $0.command,
                                                  "enabled": $0.enabled(quickOpen: quick), "tooltip": $0.tooltip(quickOpen: quick), "visible": shown,
                                                  "reveal_ms": WhooshConstants.HOVER_REVEAL_MS, "door": $0.door] }
            },
            "mac-main.reader.layout": { m in
                _ = try thread(m)
                let l = ReaderLayout.shared
                guard let pane = l.frames["pane"] else { throw ControlFailure("the reader pane has not been laid out yet") }
                return [["pane_width": Double(pane.width), "summary_gap": l.gap("summary") ?? NSNull(), "message_gap": l.gap("message") ?? NSNull()]]
            },
            "mac-main.reader.summary-callout": { m in
                let t = try thread(m)
                return (ReaderSummaryStore.shared.texts[t.threadId] ?? ReaderSummary.text(m, threadId: t.threadId)).map { [["text": $0]] } ?? []   // the probe reads the indexed value itself when the pane's task has not run yet
            },
            "mac-main.reader.codepill": { m in
                guard let t = ReaderState.open(m), let code = t.open.code else { return [] }
                return [["ax": "mac-main.reader.codepill", "code": code, "copied": state.copiedFor == t.messageId,
                         "clipboard": ReaderActions.clipboardText() ?? NSNull(), "read": m.core.message(messageId: t.messageId)?.read ?? NSNull()]]
            },
            "mac-main.reader.participants": { m in
                let t = try thread(m)
                return [["names": t.participants, "account": t.accountId, "count_words": ReaderWords.count(t.messageCount)]]
            },
            "mac-main.reader.thread": { m in
                let t = try thread(m)
                let shownExpanded = state.expandedShown(t)
                return t.entries(earlierOpen: state.earlierShown(t)).map { e in
                    let open = e.index == t.earlierCount
                    let body: Any = open ? (t.body?.visibleParagraphs(quotedOpen: false) ?? NSNull()) : NSNull()
                    return ["index": e.index, "folded": e.folded, "focused": state.focused(t) == e.index && state.focusedIndex != nil,
                            "expanded": open || shownExpanded.contains(e.index), "from": e.from ?? NSNull(), "time": e.time ?? NSNull(),
                            "sender": e.from ?? NSNull(), "text": open ? (t.body?.plainText ?? t.preview) : (e.fact?.preview ?? NSNull()),
                            "body": body, "body_state": open ? (t.body == nil ? "preview" : "downloaded") : (e.fact?.preview.isEmpty == false ? "preview" : "not-downloaded")]
                }
            },
            "mac-main.reader.passage": { m in
                // the selected search result's passage in the mail the reader shows (A152): whether it is marked, its text and its source; one row, "marked": false when none
                let t = try thread(m)
                return [ReaderPassage.reading(m, t)]
            },
            "mac-main.reader.reply": { m in
                // the closed reply field under the last message: its fill, so a check reads "lifted glass" from the app and not from a picture (A147)
                let t = try thread(m)
                let quick = QuickReplyModel.shared
                // drawn only when nothing else stands for the reply: not while the box is drawn, not with the conversation's draft card (ReaderPane)
                return ReplyBlock.rows(quickOpenHere: (quick.showsBox && quick.threadId == t.threadId) || ThreadCardsRegistry.showsDraftCard(threadId: t.threadId, m))
            },
            "mac-main.reader.attachments": { m in
                let t = try thread(m)
                if t.attachmentNames.isEmpty && t.attachmentCount > 0 { throw ControlFailure("the core counts \(t.attachmentCount) attachment(s) but names none") }
                return t.attachmentNames.enumerated().map { ["ax": "mac-main.reader.attachment.\($0.offset)", "name": $0.element, "kind": ($0.element as NSString).pathExtension.uppercased()] }
            },
            "mac-main.reader.summary": { m in
                let t = try thread(m)
                guard let words = ReaderSummaryStore.shared.texts[t.threadId] ?? ReaderSummary.text(m, threadId: t.threadId) else { throw ControlFailure("the core keeps no summary for this thread yet") }
                return [["text": words]]   // the engine's kept summary (thread/<id>/summary), the words the callout shows
            },
            "mac-main.reader.scroll": { m in
                _ = try thread(m)
                let k = ReaderScroll.shared
                return [["under_text_shown_for": ReaderUnderText.shared.shownFor ?? "",
                         "offset": k.metrics.offset.rounded(), "max_offset": k.maxOffset.rounded(), "viewport": k.metrics.viewport.rounded(), "content": k.metrics.content.rounded()]]
            },
            "mac-main.reader.links": { m in
                _ = try thread(m)
                let l = ReaderLinks.shared
                return l.items.enumerated().map { ["ax": "mac-main.reader.link.\($0.offset)", "index": $0.offset, "kind": $0.element.kind.rawValue, "text": $0.element.text,
                                                   "url": $0.element.url?.absoluteString ?? NSNull(), "ring": l.cursor == $0.offset, "message_id": l.messageId ?? NSNull()] }
            },
            "mac-main.reader.opened": { _ in
                ReaderLinks.shared.opened.map { ["n": $0.n, "kind": $0.kind, "target": $0.target, "message_id": $0.messageId, "done": $0.done] }
            },
            "mac-main.reader.shown": { m in
                let f = ReaderFocus.shared
                f.catchUp(m)
                return [["shown": f.shownId ?? NSNull(), "focused": m.focusedMessageId ?? NSNull(), "pending": f.pending, "moves": f.moves, "settles": f.settles,
                         "settle_ms": WhooshConstants.FOCUS_SLIDE_MS, "crossfade_ms": WhooshConstants.READER_CROSSFADE_MS]]
            },
            "mac-main.reader.perf": { _ in [ReaderPerf.shared.row(frameMs: Double(WhooshConstants.FRAME_MS))] },
            "mac-main.reader.state": { m in
                let t = ReaderState.open(m)
                return [["open": t != nil, "thread_id": t?.threadId ?? NSNull(),
                         "earlier_open": t.map { state.earlierShown($0) } ?? false,
                         "focused_message": t.map { state.focused($0) } ?? NSNull(), "marked": state.focusedIndex != nil,
                         "expanded": t.map { th in (0...th.earlierCount).filter { $0 == th.earlierCount || state.expandedShown(th).contains($0) } } ?? [],
                         "message_count": t?.messageCount ?? 0,
                         "delete_note": state.deletedNote ?? NSNull(), "pending": ["summary", "invites"]]]
            },
        ]
    }
}

/// The reader's words, in one place, through the 14-language catalogue (English is the key).
enum ReaderWords {
    static func count(_ n: Int) -> String { n == 1 ? Words.text("1 message") : Words.text("%@ messages", n) }
    static func earlier(_ n: Int) -> String { n == 1 ? Words.text("1 earlier message") : Words.text("%@ earlier messages", n) }
    static func attachments(_ n: Int) -> String { n == 1 ? Words.text("1 attachment") : Words.text("%@ attachments", n) }
    static func replyTo(_ sender: String) -> String { Words.text("Reply to %@…", sender) }
    static var notDownloaded: String { Words.text("Not downloaded yet") }
    static var previewOnly: String { Words.text("The rest of this message is not downloaded yet.") }
    static var emptyTitle: String { Words.text("No conversation open") }
    static var emptyLine: String { Words.text("Select one in the list, or press Enter on the focused row") }
    static var copy: String { Words.text("Copy") }
    static var copied: String { Words.text("Copied") }
    static var comingWithComposer: String { Words.text("coming with the composer round") }
    static var closeReplyFirst: String { Words.text("close the quick reply first") }
}
