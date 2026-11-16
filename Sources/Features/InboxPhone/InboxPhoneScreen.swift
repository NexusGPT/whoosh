import SwiftUI

/// The iPhone inbox (layout mail-26, style glass-light): paper list, glass only on what floats.
/// Loading, empty and error each draw their own state; the rows already shown stay through a refresh.
public struct InboxPhoneScreen: View {
    var onBack: () -> Void = {}
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.colorScheme) private var scheme
    private let m = AppModel.shared
    private let state = InboxPhoneState.shared
    private let searchModel = SearchPhoneModel.shared

    public init(onBack: @escaping () -> Void = {}) { self.onBack = onBack }

    public var body: some View {
        let rows = state.rows(m)
        ZStack(alignment: .bottom) {
            PhoneGround.list.ignoresSafeArea()
            ScrollView {
                VStack(spacing: 0) {
                    PhoneHeader(title: state.title(m), syncWords: syncWords, failing: failingWords, tabs: tabs, current: m.tab, onTab: open)
                        .padding(.top, PhoneMetrics.titleTop)
                    content(rows)
                }
                .padding(.bottom, PhoneMetrics.listBottomRoom)
            }
            .scrollIndicators(.hidden)
            .refreshable { state.refresh(m) }
            .accessibilityIdentifier("iphone-inbox.list")

            fade
            VStack(spacing: WhooshSpacing.s12) {
                if state.failure != nil || (state.undo != nil && !state.selecting) {
                    PhoneCapsule(state: state) { state.undoLast(m) }
                        .transition(reduceMotion ? .opacity : .offset(y: WhooshMotion.undoRise).combined(with: .opacity))
                }
                if state.selecting { selectionBar } else {
                    PhoneBottomBar(state: state, tabs: tabs, onTab: open, onCompose: { PhoneRouter.shared.compose() })
                }
            }
            .animation(reduceMotion ? nil : WhooshMotion.newRow, value: state.undo)
            .animation(reduceMotion ? nil : WhooshMotion.newRow, value: state.failure)
        }
        .overlay(alignment: .top) { PhoneTopButtons(state: state, onBack: onBack) }
        // Search opens in place: the screen is drawn over this one, which stays alive under it (and under a thread pushed from a result).
        .overlay { if searchModel.active { SearchPhoneScreen().transition(.opacity) } }
        .animation(reduceMotion ? nil : WhooshMotion.newRow, value: searchModel.active)
        .onAppear { let want = searchModel.active ? "iphone-search" : "iphone-inbox"; if m.surface != want { m.surface = want } }
    }

    @ViewBuilder private func content(_ rows: [InboxRow]) -> some View {
        if m.host.kind == .unavailable {
            PhoneEmpty(symbol: "exclamationmark.triangle", title: "Mail is not available", line: "The local engine did not start. Your mail stays on the server.")
        } else if rows.isEmpty {
            PhoneEmpty(symbol: "tray", title: "\(tabTitle(m.tab)) is clear", line: "Nothing waiting here. New mail lands at the top.")
        } else {
            LazyVStack(spacing: 0) {
                ForEach(Array(rows.enumerated()), id: \.element.messageId) { i, r in
                    row(r, index: i).transition(.opacity)
                }
            }
            .animation(reduceMotion ? nil : WhooshMotion.archiveCollapse, value: rows.map(\.messageId))
        }
    }

    private func row(_ r: InboxRow, index i: Int) -> some View {
        let a = PhoneAccounts.of(r.accountId, in: m)
        return SwipeRow(leading: state.swipes.leading, trailing: state.swipes.trailing,
                        onCommit: { state.act($0, row: i, in: m) }) {
            PhoneRow(row: r, account: a, selecting: state.selecting, selected: state.selected.contains(r.messageId))
        }
        .onTapGesture { if state.selecting { state.toggle(r.messageId) } else { PhoneRouter.shared.openThread(row: i) } }
        .contextMenu {
            Button("Reply", systemImage: "arrowshape.turn.up.left") { m.focus(messageId: r.messageId); PhoneRouter.shared.reply(messageId: r.messageId) }
            Button("Archive", systemImage: "archivebox") { state.act(.archive, row: i, in: m) }
            Button("Snooze", systemImage: "clock") { state.act(.snooze, row: i, in: m) }
            Button(r.pinned ? "Unpin" : "Pin", systemImage: "pin") { state.act(.pin, row: i, in: m) }
            Button("Label…", systemImage: "tag") { m.focus(messageId: r.messageId); m.overlay = "iphone-label-picker" }
        }
        .accessibilityIdentifier("iphone-inbox.list.row.\(i)")
    }

    private var selectionBar: some View {
        HStack(spacing: WhooshSpacing.s24) {
            ForEach([SwipeAction.archive, .snooze], id: \.self) { a in
                Button { state.actOnSelection(a, in: m) } label: { Label(a.title, systemImage: a.symbol).phoneText(PhoneType.headline) }
                    .buttonStyle(PressStyle()).disabled(state.selected.isEmpty)
            }
        }
        .foregroundStyle(WhooshPalette.label)
        .frame(maxWidth: .infinity).frame(height: PhoneMetrics.barButton)
        .whooshGlass(in: Capsule()).padding(.horizontal, PhoneMetrics.gutter)
        .transition(.opacity)
    }

    /// `.edgeB`: the preview text fades to paper before it reaches the glass.
    private var fade: some View {
        LinearGradient(stops: [.init(color: PhoneGround.list(scheme).opacity(0), location: 0), .init(color: PhoneGround.list(scheme), location: 0.72)],
                       startPoint: .top, endPoint: .bottom)
            .frame(height: PhoneMetrics.edgeFade).frame(maxHeight: .infinity, alignment: .bottom)
            .ignoresSafeArea(edges: .bottom).allowsHitTesting(false)
    }

    private var tabs: [(id: String, title: String, unread: Int)] {
        m.tabIds.map { id in (id, tabTitle(id), Int(m.counts.tabs.first { $0.tabId == id }?.unread ?? 0)) }
    }
    private func tabTitle(_ id: String) -> String { m.host.fixture?.tabName(id) ?? id }
    private func open(_ id: String) { m.tab = id; m.focusIndex = 0; m.refresh() }

    private var failingWords: String? {
        m.core.syncStates().first { $0.lastError != nil }.map { s in
            "\(PhoneAccounts.of(s.accountId, in: m).name) needs a new sign-in"
        }
    }
    private var syncWords: String {
        let mins = Int(Date().timeIntervalSince(state.refreshedAt) / 60)
        return mins < 1 ? "Updated just now" : "Updated \(mins) min ago"
    }
}

/// The empty and error states: a quiet glyph, one title, one line.
struct PhoneEmpty: View {
    let symbol: String
    let title: String
    let line: String
    var body: some View {
        VStack(spacing: WhooshSpacing.s6) {
            Image(systemName: symbol).font(.system(size: PhoneMetrics.roundButton)).foregroundStyle(WhooshPalette.tertiaryLabel)
                .padding(.bottom, WhooshSpacing.s8)
            Text(title).phoneText(PhoneType.title3).foregroundStyle(WhooshPalette.label)
            Text(line).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel).multilineTextAlignment(.center)
        }
        .padding(.horizontal, WhooshSpacing.s32).padding(.top, PhoneMetrics.edgeFade / 2)
        .accessibilityElement(children: .combine)
    }
}
