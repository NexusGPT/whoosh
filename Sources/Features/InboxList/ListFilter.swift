import SwiftUI

/// The four Superhuman filters (docs/reference/superhuman-shortcuts-v8.md, Filters): ⇧U unread, ⇧S starred,
/// ⇧I important, ⇧R no reply. One at a time; the same key again, a click on the chip or Esc clears it.
enum ListFilter: String, CaseIterable, Sendable {
    case unread, starred, important, noReply = "no-reply"

    /// the shortcuts command that opens it ("filter-unread")
    var command: String { "filter-" + rawValue }
    var fallbackKey: String {
        switch self {
        case .unread: "⇧U"
        case .starred: "⇧S"
        case .important: "⇧I"
        case .noReply: "⇧R"
        }
    }
    var title: String {
        switch self {
        case .unread: Words.text("Unread")
        case .starred: Words.text("Starred")
        case .important: Words.text("Important")
        case .noReply: Words.text("No reply")
        }
    }

    /// The core's filter (contract delta 10): the core decides which rows pass, the list never re-derives it.
    var core: InboxFilter {
        switch self {
        case .unread: .unread
        case .starred: .starred
        case .important: .important
        case .noReply: .noReply
        }
    }
}

/// What the filter does to the list: which rows show, where the focus goes, and the words when none match.
@MainActor
enum ListFilters {
    static var state: InboxListState { .shared }

    /// The rows on screen with their place in the model (the focus index and the click ids count the model's rows).
    /// Under a filter, the ids that pass come from ONE core read per change of the rows or the filter (`passing`).
    static func shown(_ m: AppModel) -> [(Int, InboxRow)] {
        let out = Array(m.rows.enumerated()).map { ($0.offset, $0.element) }   // under a label filter the rows are the core's, already the label's
        guard let f = state.filter else { return out }
        let keep = passing(f, m)
        return out.filter { keep.contains($0.1.messageId) }
    }

    /// The message ids the core keeps under the filter on the current list (InboxQuery.filters, delta 10), cached on
    /// the filter, the row ids WITH their unread marks and the starred ids, so a render never reads the core twice for the same list, and a
    /// message read, unread or starred without its row leaving is read again (found 2026-11-10: the stamp held ids only, so after "mark all read"
    /// under ⇧U the empty answer stayed for the same ids even when the mail was unread again; par-inboxlist J1.51).
    static func passing(_ f: ListFilter, _ m: AppModel) -> Set<String> {
        let stamp = FilterStamp(filter: f, tab: m.tab, account: m.accountFilter, mailbox: m.mailbox, smartFolder: m.smartFolderId,
                              ids: m.rows.map { $0.unread ? $0.messageId + "\u{1}u" : $0.messageId }, starred: state.starred)
        if let c = cache, c.stamp == stamp { return c.ids }
        let q = InboxQuery(tabId: m.tab, accountId: m.accountFilter, labelId: nil, mailbox: m.mailbox, accountIds: [], filters: [f.core], smartFolderId: m.smartFolderId)
        let ids = Set(m.core.inbox(query: q).map(\.messageId))
        cache = (stamp, ids)
        return ids
    }

    struct FilterStamp: Equatable { let filter: ListFilter; let tab: String; let account: String?; let mailbox: Mailbox?; let smartFolder: String?; let ids: [String]; let starred: Set<String> }
    static var cache: (stamp: FilterStamp, ids: Set<String>)?

    /// The starred ids among the rows, read from the core once per change of the rows, never once per render.
    static func refreshStarred(_ m: AppModel) {
        // perf-archive r4: written only when it changes. This runs one frame after EVERY change of the rows (rowsChanged), and every
        // row's key reads `starred`: an equal write re-evaluated the whole list one frame into the archive's exit animation.
        let fresh = Set(m.rows.map(\.messageId).filter { m.core.state(path: "message/\($0)/starred") == "true" })
        if fresh != state.starred { state.starred = fresh }
    }

    /// The side reads that follow a change of the rows (the star marks, the why tags), moved OFF the key's frame.
    /// Found 2026-11-10 (list r7, item A105): on the real core each `core.state(path:)` is a store read that waits on the
    /// store while the write-back worker holds it after an archive, and `.onChange(of: model.rows)` ran N of them in the same
    /// main-thread turn that commits the collapse, so the first drawn frame came after ROW_COLLAPSE_MS and the row simply
    /// vanished. The ids that left drop from the marks at once (no read); the reads run one frame later, once, however many
    /// changes of the rows came in between. The fixture answers the reads in no time, which is why only the real core jumped.
    private static var sideReads: Task<Void, Never>?
    static func rowsChanged(_ m: AppModel) {
        let live = Set(m.rows.map(\.messageId))
        if !state.starred.isSubset(of: live) { state.starred.formIntersection(live) }
        RowMarks.prune(live)
        sideReads?.cancel()
        sideReads = Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(WhooshConstants.FRAME_MS))
            guard !Task.isCancelled else { return }
            refreshStarred(m)
            WhyTags.refresh(m)
            ThreadFacts.refresh(m)
            RowMarks.refresh(m)
        }
    }

    /// A filter key: opens the filter, or closes it when it is already the open one.
    /// The model changes on the key's frame; the rows that leave or return glide inside `RowCollapse.run` (transform and
    /// opacity, ROW_COLLAPSE_MS, no glide under Reduced Motion or on a rapid second key), so the filter never waits.
    static func toggle(_ f: ListFilter, _ m: AppModel) -> Bool {
        RowCollapse.run {
            if state.filter == f { clear(m); return true }
            refreshStarred(m)
            state.filter = f
            if let first = shown(m).first, !shown(m).contains(where: { $0.0 == m.focusIndex }) { m.focusIndex = first.0 }
            return true
        }
    }

    static func clear(_ m: AppModel) {
        state.filter = nil
        state.prune(visible: m.rows.map(\.messageId))
    }

    /// J and K under a filter step over the rows it hides. Without a filter the key passes to the shell's own move.
    static func move(_ m: AppModel, _ delta: Int) -> Bool {
        guard state.filter != nil || state.labelFilter != nil else { return false }
        let at = shown(m).map(\.0)
        guard !at.isEmpty else { return false }
        if let next = delta > 0 ? at.first(where: { $0 > m.focusIndex }) : at.last(where: { $0 < m.focusIndex }) { m.focusIndex = next }
        else if !at.contains(m.focusIndex) { m.focusIndex = at[0] }
        return true
    }

    /// The zero state of a filter that keeps no row.
    static func emptyWords(_ f: ListFilter) -> EmptyStateWords {
        EmptyStateWords(title: Words.text("Nothing matches"), line: Words.text("No row is %@ in this tab.", f.title.lowercased()), keyHint: Words.text("Esc clears the filter"))
    }
}

/// The chip above the rows: the filter's name and a cross; a click on it clears the filter (Esc does too).
struct FilterChip: View {
    let filter: ListFilter
    let model: AppModel
    var body: some View {
        Button { ListFilters.clear(model) } label: {
            HStack(spacing: WhooshSpacing.s6) {
                Image(systemName: "line.3.horizontal.decrease").font(WhooshTypography.caption.font(emphasized: true))
                Text(filter.title).font(WhooshTypography.subhead.font(emphasized: true))
                Image(systemName: "xmark").font(WhooshTypography.caption.font(emphasized: false)).foregroundStyle(WhooshPalette.secondaryLabel)
                KeyCap("Esc")
            }
            .foregroundStyle(WhooshPalette.label)
            .padding(.horizontal, WhooshSpacing.s12).frame(height: WhooshMetrics.segmentHeight)
            .background(Capsule().fill(WhooshPalette.background))
            .whooshGlass(in: Capsule())
        }
        .buttonStyle(.plain)
        .help(Words.text("Clear the filter · Esc"))
        .padding(.horizontal, WhooshMetrics.listRowInset + WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s4)
        .frame(maxWidth: .infinity, alignment: .leading)
        .whooshAx("mac-main.list.filter")
    }
}
