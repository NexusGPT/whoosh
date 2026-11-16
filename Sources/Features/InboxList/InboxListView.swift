import SwiftUI

/// The mac-main list pane: the glass split control on top, then the rows (or a state), the undo capsule
/// while an action can be undone. Reads AppModel (the shell's core-backed rows) and InboxListState only.
public struct InboxListView: View {
    @State private var model = AppModel.shared
    @State private var list = InboxListState.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @AppStorage(ListSettings.showEmblemsKey) private var showEmblems = true
    @Namespace private var selectionSpace
    @State private var focusMark = FocusMark()
    @State private var settleTask: Task<Void, Never>?

    public init() {}

    /// the command field sits under the tab strip, the header under the field, the rows under the header
    static let fieldTop = WhooshMetrics.sidebarTop - WhooshSpacing.s4
    static let listTop = fieldTop + WhooshMetrics.glassPillHeight + WhooshSpacing.s4 + WhooshMetrics.listHeaderHeight + WhooshSpacing.s2

    public var body: some View {
        let _ = LaunchClock.once("list-body")   // perf-archive r4: a launch phase
        ZStack(alignment: .top) {
            Rectangle().fill(WhooshPalette.background).ignoresSafeArea()
            // the foot band (selection bar, undo capsule, the one suggestion) is a bottom INSET of the content, never an
            // overlay: the scroll view stops its last row and the end-of-list line above it, at every window size
            // the drawing P1's "Typing a domain" (search-r3-parts/designs.js `typing`): the rows behind the open dropdown at opacity .35
            // (the drawing's own number), so the suggestions read first; round 19, d1ffe36c clause 1
            let dimmed = SearchMacModel.shared.active && SearchMacModel.shared.dropdownVisible
            // The search's results carry their own count line ("2 matches · 3 related"); the inbox's line ("All accounts · 31 unread") is about
            // the inbox and is not in the picked drawing (search-r3-parts P1), so it leaves while results show and the results start in its place
            // (lead, 13 Nov, owner A162: two header lines stood above the results, one of them about another list).
            let results = SearchMacModel.shared.active && SearchMacModel.shared.launched && !LabelInspector.showsProof
            content.padding(.top, results ? Self.listTop - WhooshMetrics.listHeaderHeight : Self.listTop)
                .opacity(dimmed ? 0.35 : 1)
                .animation(.easeOut(duration: Double(WhooshConstants.SEARCH_DROPDOWN_ENTER_MS) / 1000), value: dimmed)
                .safeAreaInset(edge: .bottom, spacing: 0) {
                    // the capsule rides above the window's foot strip (FOOT_STRIP_PX from its hairline to the window
                    // foot, one foot line with the reader's key strip); the strip's height never changes with it
                    VStack(spacing: 0) { ListChrome(model: model, list: list); ListFootStrip() }
                }
            SplitTabsBar(model: model).padding(.top, WhooshMetrics.toolbarTop)
            CommandField(model: model).padding(.top, Self.fieldTop)
            // labels r2 hook: while the label inspector is open the line under the tab row is the live match count
            if !results {
                Group { if LabelInspector.showsProof { LabelProofLine() } else { ListHeader(model: model) } }
                    .padding(.top, Self.fieldTop + WhooshMetrics.glassPillHeight + WhooshSpacing.s4)
            }
            // the search's dropdown, under the field; kept in the tree so the first key only fades it in. perf-archive r6: mounted from
            // the turn after the first frame (firstFrameMs set): at launch it is hidden (opacity 0, no hit testing, hidden from
            // accessibility) and no key can reach it before the socket and the window take keys, so the first frame shows the same
            // pixels without building it
            if model.firstFrameMs != nil {
                SearchDropdownHost(model: SearchMacModel.shared, app: model).padding(.top, Self.fieldTop + WhooshMetrics.glassPillHeight + WhooshSpacing.s6)
            }
            if let f = model.lastFailure { FailureLine(failure: f).padding(.top, WhooshMetrics.sidebarTop) }
            ListOverlayHost(model: model).padding(.top, Self.listTop)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .clipped()
        .onAppear { BodyWebGate.shared.warmUp(); ListPerf.install(); ListFilters.refreshStarred(model); WhyTags.refresh(model); ThreadFacts.refresh(model); RowMarks.refresh(model) }
        .onChange(of: model.rows) { _, _ in ListFilters.rowsChanged(model) }   // off the key's frame: see rowsChanged
        .onChange(of: WhyTags.switchOn) { _, _ in WhyTags.refresh(model) }   // the inspector's « Pourquoi ici ? »: one read per row, once
        // the pane id "mac-main.list" is the shell's; this one proves the list itself is mounted and has a frame
        .whooshAx("mac-main.list.view")
    }

    @ViewBuilder private var content: some View {
        if LabelInspector.showsProof {
            LabelProofRows()   // labels r2 hook: the rows the draft rules match, tagged; join/leave/stay bars on a re-run preview
        } else if SearchMacModel.shared.active && SearchMacModel.shared.launched {
            SearchResultsPane(model: SearchMacModel.shared, app: model, compact: false)   // Enter: skeleton rows, then the results, in this pane
        } else if model.rows.isEmpty, let loading = FirstSyncWords.make(model) {
            LoadingList(words: loading)
        } else if let empty = EmptyStateWords.make(model) {
            ListEmptyView(words: empty)
        } else {
            VStack(spacing: 0) {
                if let l = list.labelFilter { LabelFilterChip(label: l) }
                if let f = list.filter, f != .unread { FilterChip(filter: f, model: model) }   // Unread is the count in the count line (UnreadCount)
                rowsList
            }
        }
    }

    @ViewBuilder private var rowsList: some View {
        let shown = ListFilters.shown(model)
        if let f = list.filter, shown.isEmpty {
            ListEmptyView(words: ListFilters.emptyWords(f))
        } else if let l = list.labelFilter, shown.isEmpty {
            ListEmptyView(words: LabelFilter.emptyWords(l))
        } else {
            let book = RowAccount.cachedDirectory(model)
            let key = focusMark.key(to: model.focusIndex)
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(GroupedRows.sections(shown, by: list.grouping, in: model), id: \.key) { section in
                            if !section.key.isEmpty { GroupHeader(title: section.key, count: section.rows.count) }
                            ForEach(section.rows, id: \.1.messageId) { i, r in
                                row(i, r, account: RowAccount.of(r.accountId, in: book), key: key)
                                    .id(r.messageId).transition(RowMotion.transition(reduceMotion: reduceMotion, height: list.compact ? InboxRowView.compactHeight : WhooshMetrics.listRow))
                                    .onAppear { if model.nearListEnd(i) { model.wantMoreRows() } }   // scrolled near the end of what the list holds: the next page
                            }
                        }
                        ListEndLine(model: model, shown: shown.count).onAppear { model.wantMoreRows() }
                    }
                }
                .onChange(of: model.focusIndex) { _, i in
                    list.focusMotion = focusMark.motion(to: i)
                    list.focusDistance = abs(i - focusMark.last)
                    list.focusRapid = focusMark.rapid()
                    focusMark.settle(at: i)
                    if model.nearListEnd(i) { model.wantMoreRows() }   // the keys reach the end of what the list holds: the next page
                    if model.rows.indices.contains(i) { proxy.scrollTo(model.rows[i].messageId) }
                    holdStrip()
                }
            }
        }
    }

    private func row(_ i: Int, _ r: InboxRow, account: RowAccount, key: Int) -> some View {
        let anim = focusMark.animation(to: model.focusIndex, reduceMotion: reduceMotion)
        let why = list.whyOpen == r.messageId ? WhyWords.line(r, in: model) : nil
        let strip = list.stripSettled || i != model.focusIndex
        let whyTag = WhyTags.tag(r.messageId, model)
        // The focus fill and its animation are drawn by two rows only: the one taking the focus and the one giving it (`focusMark.last` until the move settles).
        // Every other row's key holds neither, so a move redraws those two rows and not every row on screen: the animation kind changes three times in one move
        // (resting, moving, inside FOCUS_SLIDE_MS) and was in every row's key (main180, twenty paced moves: 33 row bodies per move; the main thread's sample:
        // two thirds of a move's work in SwiftUI's update).
        let involved = i == model.focusIndex || i == focusMark.last
        let muted = RowMarks.muted(r), vip = RowMarks.vip(r), snooze = RowMarks.snoozeWords(r), rested = list.rested == r.messageId   // board states 13, 14, 10, 7
        let rowKey = RowKey(row: r, index: i, account: account, focused: i == model.focusIndex, selected: list.selection.contains(r.messageId), starred: list.starred.contains(r.messageId), repliedLast: list.repliedLast.contains(r.messageId),
                            compact: list.compact, showEmblem: showEmblems, highlightKey: involved ? key : 0, animation: involved ? anim : nil, why: why, whyTag: whyTag, stripAllowed: strip,
                            muted: muted, vip: vip, snoozeWords: snooze, pointerRested: rested)
        return InboxRowView(row: r, account: account, focused: i == model.focusIndex,
                     selected: list.selection.contains(r.messageId), starred: list.starred.contains(r.messageId), repliedLast: list.repliedLast.contains(r.messageId), compact: list.compact, showEmblem: showEmblems,
                     highlight: FocusHighlight(ns: selectionSpace, key: key), why: why, whyTag: whyTag, highlightAnimation: anim, stripAllowed: strip,
                     muted: muted, vip: vip, snoozeWords: snooze, pointerRested: rested,
                     onHover: { h in list.hovered = h ? r.messageId : (list.hovered == r.messageId ? nil : list.hovered) },
                     onRest: { h in list.rested = h ? r.messageId : (list.rested == r.messageId ? nil : list.rested) },
                     onAction: { action in
                        // by the row's message, not its place: the equatable row keeps this closure across a shift of the places (perf-archive r7)
                        if let j = model.rows.firstIndex(where: { $0.messageId == r.messageId }) { act(action, at: j) }
                     }, key: rowKey)
            .equatable()
            .accessibilityIdentifier("mac-main.list.row.\(i)")   // the place id, outside the equatable row (perf-archive r7)
            .onTapGesture { model.focusIndex = i }
            .onTapGesture(count: 2) { model.focusIndex = i; _ = model.press("Enter") }
            .contextMenu { Button(RuleFromMessage.title) { _ = RuleFromMessage.run(model, row: i) } }   // J5.9: the same door as `mac-main.list.row.<n>.create-rule`
    }

    /// The focused row's hover strip appears once the keys settle, not on every move (a held J draws none).
    private func holdStrip() {
        if list.stripSettled { list.stripSettled = false }
        settleTask?.cancel()
        settleTask = Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(WhooshConstants.FOCUS_SLIDE_MS))
            if !Task.isCancelled { list.stripSettled = true }
        }
    }

    private func act(_ action: String, at i: Int) {
        // a strip on a row outside the selection acts on that row alone
        if !list.selection.isEmpty, !list.selection.contains(model.rows[i].messageId) { list.clearSelection() }
        model.focusIndex = i
        if action == "why" { _ = Registrations.click("mac-main.list.row.\(i).why")?(model); return }
        _ = TrainerFeature.mouseUsed(action, model)   // counts the mouse use, shows the key hint
    }
}

/// A section title when the list is grouped by sender or by domain.
struct GroupHeader: View {
    let title: String
    let count: Int
    var body: some View {
        HStack {
            Text(title).font(WhooshTypography.caption.font(emphasized: true)).foregroundStyle(WhooshPalette.secondaryLabel)
            Text("\(count)").font(WhooshTypography.caption.font(emphasized: false)).foregroundStyle(WhooshPalette.tertiaryLabel)
            Spacer()
        }
        .padding(.horizontal, WhooshMetrics.listRowInset + WhooshSpacing.s12)
        .frame(height: WhooshMetrics.listHeaderHeight)
    }
}

/// A refused action, in words: the account and the reason; the row has already come back.
struct FailureLine: View {
    let failure: ActionFailed
    var body: some View {
        HStack(spacing: WhooshSpacing.s6) {
            Circle().fill(WhooshPalette.warning).frame(width: WhooshMetrics.warningDot, height: WhooshMetrics.warningDot)
            Text("\(failure.accountName): \(failure.reasonWords)").font(WhooshTypography.subhead.font(emphasized: false))
                .foregroundStyle(WhooshPalette.label).lineLimit(2)
        }
        .padding(.horizontal, WhooshSpacing.s14).padding(.vertical, WhooshSpacing.s6)
        .background(Capsule().fill(WhooshPalette.background))
        .whooshGlass(in: Capsule())
        .accessibilityIdentifier("mac-main.list.failure")
    }
}
