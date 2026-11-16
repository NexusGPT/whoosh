import Foundation

/// The `rows` answers for the list's regions: the same facts the rows draw, in the order they draw them.
@MainActor
enum ListProbe {
    static var state: InboxListState { .shared }

    static func rows(_ m: AppModel) -> [ControlRow] {
        let selected = Set(state.selection)
        let book = RowAccount.cachedDirectory(m)
        let emblem = ListSettings.showEmblems ? "row" : "hidden"
        let shown = ListFilters.shown(m)
        let rings = RingProbe.widths(shown.map(\.1.messageId))
        return shown.map { i, r in
            let a = RowAccount.of(r.accountId, in: book)
            var out: ControlRow = ["ax": "mac-main.list.row.\(i)", "id": r.messageId, "sender": r.senderName, "subject": r.subject,
                    "preview": r.preview, "time": RowFacts.time(r.receivedAtMs), "account": r.accountId,
                    "account_name": a.name, "account_address": a.address, "account_colour": a.colour,
                    "account_icon": a.iconId ?? NSNull(), "account_attention": a.needsAttention, "emblem": emblem,
                    "initials": RowFacts.initials(r.senderName), "unread": r.unread, "pinned": r.pinned, "reminder": r.reminderFired,   // gap-remind: the reply reminder fired (the chip)
                    "chips": r.labelNames, "draft": RowFacts.hasDraft(threadId: r.threadId, in: m), "thread_count": Int(r.threadCount), "count_pill": RowFacts.pillCountProbe(r), "replied_last": state.repliedLast.contains(r.messageId) && r.threadCount > 1, "attachments": Int(r.attachmentCount),
                    "focused": i == m.focusIndex, "selected": selected.contains(r.messageId), "starred": state.starred.contains(r.messageId),
                    "group": GroupedRows.key(r, by: state.grouping, in: m),
                    "decided_by": WhyWords.decidedBy(r) ?? NSNull(), "sender_address": r.senderAddress,
                    "why": WhyWords.line(r, in: m), "hairline": i != m.focusIndex,
                    "ring_px": rings.width[r.messageId].map { Double($0) } ?? rings.fallback.map { Double($0) } ?? NSNull(),
                    "ring_source": rings.width[r.messageId] != nil ? "row" as Any : (rings.fallback != nil ? "list" as Any : NSNull())]
            out["count_pill_fill"] = ThreadFacts.pillCount(r.threadCount) == nil ? NSNull() as Any : CountPill.fillKind(unread: r.unread, focused: i == m.focusIndex).rawValue as Any   // the pill's fill the row draws (null for a single message)
            out["why_tag"] = WhyTags.tag(r.messageId, m)?.line ?? NSNull()      // the tag the row draws (null with the switch off)
            out["why_card"] = state.whyReads[r.messageId]?.card.rawValue ?? NSNull()
            // board message-card-r1 states 13, 14, 10 and 7: the marks the row draws (RowMarks) and the pointer on it (InboxListState.hovered, .rested)
            out["muted"] = RowMarks.muted(r)
            out["vip"] = RowMarks.vip(r)
            out["snoozed"] = RowMarks.snoozeWords(r) ?? NSNull()     // the words drawn in the time's place, null when the mail is not snoozed
            out["hovered"] = state.hovered == r.messageId || state.rested == r.messageId   // the hover fill is drawn
            out["rested"] = state.rested == r.messageId              // the pointer rested: the strip may show (it shows unless the line is too narrow for it)
            return out
        }
    }

    /// The Unread control in the count line (UnreadCount): its id, state, key and the words it draws.
    static func unreadControl(_ m: AppModel) -> ControlRow {
        let on = state.filter == .unread
        let w = UnreadCount.words(on: on, count: ListHeaderWords.unread(m))
        return ["ax": "mac-main.splits.unread", "on": on, "key": LiveKeys.chord(ListFilter.unread.command, ListFilter.unread.fallbackKey),
                "place": "count-line", "count_words": w.count, "show_all": w.tail ?? NSNull(), "count_style": UnreadCount.countStyle(on: on).rawValue]
    }

    /// The messages this list snoozed that the core still holds snoozed: id, sender, subject, `snooze_words` (the core's
    /// `snooze/<id>/return_local_time`) and `snooze_ms` (`snooze/<id>/return_ms`, null until the core serves it).
    /// The core has no read of the whole snoozed list yet, so a snooze made elsewhere is not listed.
    static func snoozed(_ m: AppModel) -> [ControlRow] {
        state.snoozedHere.filter { m.core.state(path: "message/\($0.id)/snoozed") == "true" }.enumerated().map { i, r in
            ["ax": "mac-main.snooze.\(i)", "id": r.id, "sender": r.sender, "subject": r.subject,
             "snooze_words": CoreText.value("snooze/\(r.id)/return_local_time", in: m) ?? NSNull(),
             "snooze_ms": Int64(CoreText.value("snooze/\(r.id)/return_ms", in: m) ?? "") ?? NSNull()]
        }
    }

    /// Every pinned label in the sidebar's order; `shown` is false for the ones folded into the "+N" chip.
    static func splits(_ m: AppModel) -> [ControlRow] {
        let tabs = PinnedTabs.all(m)
        let shown = Set(PinnedTabs.split(tabs).shown.map(\.id))
        return tabs.enumerated().map { i, t in
            ["ax": "mac-main.splits.\(i)", "id": t.id, "title": t.name, "unread": t.unread, "waiting": t.waiting,
             "kind": t.isView ? "view" : "label", "menu": t.menu, "on": m.mailbox == nil && t.id == m.tab, "shown": shown.contains(t.id)]
        }
    }

    /// `mac-main.tabs` (the shell's base region, overridden here): the same pinned labels with their waiting counts.
    static func tabs(_ m: AppModel) -> [ControlRow] {
        PinnedTabs.all(m).enumerated().map { i, t in
            ["ax": "mac-main.tabs.\(i)", "id": t.id, "title": t.name, "unread": t.unread,
             "waiting": Int(m.counts.tabs.first { $0.tabId == t.id }?.waiting ?? 0)]
        }
    }

    static func listState(_ m: AppModel) -> ControlRow {
        var out: ControlRow = ["grouping": state.grouping.rawValue, "compact": state.compact,
                               "why_open": state.whyOpen ?? NSNull(), "why_tags": WhyTags.probe(m),
                               "selected": state.selection.count, "show_emblems": ListSettings.showEmblems, "row_budget": Int(m.rowBudget), "rows": m.rows.count,
                               "field_key": LiveKeys.search,
                               "filter": state.filter?.rawValue ?? NSNull(), "label_filter": state.labelFilter ?? NSNull(), "tab": m.tab, "filter_empty": state.filter != nil && ListFilters.shown(m).isEmpty,
                               "strip_settled": state.stripSettled, "search_active": SearchMacModel.shared.active,
                               "last_failure": m.lastFailure?.reasonWords ?? NSNull(), "filter_words": state.filter?.title ?? NSNull(),
                               "tab_cap": PinnedTabs.cap, "plus_n": PinnedTabs.split(PinnedTabs.all(m)).folded.count,
                               "unread_chip": unreadControl(m),
                               "strip": RowStrip.items.map { ["id": $0.id, "key": LiveKeys.chord($0.id, $0.key), "help": RowStrip.help($0)] },
                               "selection_bar": state.selection.isEmpty ? NSNull() : ["count": state.selection.count, "actions": RowStrip.items.map(\.id)],
                               "selection_motion": state.focusMotion.rawValue, "selection_distance": state.focusDistance, "selection_rapid": state.focusRapid, "collapse_glided": RowCollapse.lastGlided, "collapse_gap_ms": RowCollapse.lastGapMs ?? NSNull(),
                               "slide_max_rows": WhooshConstants.SLIDE_MAX_ROWS,
                               "first_sync": FirstSyncWords.progress(m).map { ["indexed": $0.indexed, "total": $0.total] } ?? NSNull(),
                               "undo_available": state.undoAvailable, "header": ListHeaderWords.line(m),
                               "undo_timer": state.undoTimer ?? NSNull(),   // the fraction of the undo chip's life left, 1 to 0 (undo-timer)
                               "header_filter": ListHeaderWords.filterName(m), "header_unread": ListHeaderWords.unread(m), "undo_words": state.undoWords ?? NSNull(), "undo_key": state.undoSaid?.key ?? NSNull(),
                               "smart_folder": m.smartFolderId ?? NSNull(), "mailbox": m.mailbox.map { MailboxWords.key($0).lowercased() } ?? NSNull(), "header_line": ListHeaderWords.line(m),
                               "loading": m.rows.isEmpty && FirstSyncWords.make(m) != nil,
                               "empty_words": EmptyStateWords.make(m).map { ["title": $0.title, "line": $0.line] as [String: String] } ?? NSNull() as Any,
                               "end_of_list": m.rows.isEmpty ? NSNull() as Any : ListEndWords.make(m, shown: ListFilters.shown(m).count) as Any]
        switch state.pending {
        case .snooze(let ids): out["pending"] = "snooze"; out["pending_ids"] = ids
        case .label(let ids): out["pending"] = "label"; out["pending_ids"] = ids
        case nil: out["pending"] = NSNull()
        }
        out["empty_state"] = InboxListFeature.emptyState(m) ?? NSNull()
        return out
    }
}

/// Group keys for the on-screen sections.
@MainActor
enum GroupedRows {
    static func key(_ r: InboxRow, by g: InboxListState.Grouping, in m: AppModel) -> String {
        switch g {
        case .none: return ""
        case .sender: return r.senderName
        case .domain: return RowFacts.domain(address: r.senderAddress, sender: r.senderName)
        }
    }

    /// Rows in section order: groups keep the order of their first row, rows keep theirs inside a group.
    static func sections(_ rows: [(Int, InboxRow)], by g: InboxListState.Grouping, in m: AppModel) -> [(key: String, rows: [(Int, InboxRow)])] {
        var order: [String] = []
        var buckets: [String: [(Int, InboxRow)]] = [:]
        for (i, r) in rows {
            let k = key(r, by: g, in: m)
            if buckets[k] == nil { order.append(k) }
            buckets[k, default: []].append((i, r))
        }
        return order.map { ($0, buckets[$0] ?? []) }
    }
}

/// The drawn width of each row's account ring, in points, read from the layout through the frame probe
/// (`mac-main.list.row.<id>.emblem`), never from a constant (tests r4 gap: J1.1/row-ring-min-px). A row the lazy list
/// has not drawn (below the fold) carries the width of a drawn row's ring, marked `ring_source: "list"` (every row draws
/// the same emblem tier); with emblems hidden nothing is drawn and the field is null.
@MainActor
enum RingProbe {
    static func widths(_ ids: [String]) -> (width: [String: CGFloat], fallback: CGFloat?) {
        var out: [String: CGFloat] = [:]
        for id in ids {
            if let f = FrameProbe.request("mac-main.list.row.\(id).emblem"), f.width > 0 { out[id] = f.width }
        }
        return (out, ids.lazy.compactMap { out[$0] }.first)
    }
}
