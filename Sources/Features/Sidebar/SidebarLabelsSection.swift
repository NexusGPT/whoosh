import SwiftUI

/// The Labels section as a tree (owner: "client>Provanta … up to like 3-5 levels"). A folded parent shows the unread of
/// its whole branch; an open one shows its own, its children show theirs (Mail's convention, so nothing counts twice).
/// Right expands, left collapses; a click on a label puts the arrow keys on it. Round 6: the pin glyph on pinned labels (pin and
/// unpin on hover and in the menu), the account mark on an account's own labels (shown only under that account's filter), and
/// drag to a new position or parent (LabelDrag.swift).
struct SidebarLabelsSection: View {
    @Bindable var state: SidebarState
    let model: AppModel
    let ns: Namespace.ID
    @Environment(\.accessibilityReduceMotion) private var reduce
    @FocusState private var focused: Bool

    var body: some View {
        let _ = state.revision
        let _ = state.labelsRevision    // the mail changed and has been quiet: the unread figures are read again (SidebarState.mailChanged)
        let all = SidebarData.labels()
        let accounts = SidebarData.accounts(core: model.core)
        let collapsed = state.isCollapsed("labels")
        VStack(alignment: .leading, spacing: 0) {
            SidebarHeader(title: "Labels", section: "labels", collapsed: collapsed,
                          onCollapse: { withAnimation(SidebarMotion.disclosure(reduce: reduce)) { _ = state.toggleSection("labels") } },
                          onAdd: { _ = LabelInspector.openNew(parent: nil) }, forceHover: state.hoverHeader == "labels") { EmptyView() }
                .background(RoundedRectangle(cornerRadius: WhooshRadius.control).strokeBorder(WhooshPalette.accent, lineWidth: 2)
                    .opacity(state.drag?.target?.rowId == LabelDropTarget.topId ? 1 : 0).allowsHitTesting(false))
                .reportLabelFrame(LabelDropTarget.topId)
                .contextMenu { SidebarMenuContent(subject: .labelsHeader, model: model, state: state) }
                .help(Words.text("Drop a label here to move it to the top level"))
            if !collapsed {
                ForEach(all.filter { state.labelVisible($0, in: all) }) { l in
                    row(l, all: all, accounts: accounts)
                    if let n = LabelInspector.newRow, n.parentId == l.id, state.openLabels.contains(l.id) { NewLabelRow(name: n.name, depth: l.depth + 1) }
                }
                if let n = LabelInspector.newRow, n.parentId == nil { NewLabelRow(name: n.name, depth: 0) }
            }
        }
        .coordinateSpace(name: LabelDrag.space)
        .onPreferenceChange(LabelRowFrames.self) { state.labelFrames = $0 }
        .overlay(alignment: .topLeading) { ghost(all) }
        .focusable()
        .focusEffectDisabled()
        .focused($focused)
        .onKeyPress(.rightArrow) { state.labelRight(reduce: reduce) ? .handled : .ignored }
        .onKeyPress(.leftArrow) { state.labelLeft(reduce: reduce) ? .handled : .ignored }
    }

    private func row(_ l: SidebarLabelRow, all: [SidebarLabelRow], accounts: [SidebarAccount]) -> some View {
        let open = state.openLabels.contains(l.id)
        let count = l.hasChildren && !open ? l.total : l.own
        state.drawnLabelCounts[l.id] = count
        let owner: Account? = { if case .account(let id) = l.label.scope { return accounts.first { $0.id == id }?.account }; return nil }()
        let dragged = state.drag?.id == l.id
        return SidebarRow(symbol: "tag", title: SidebarData.title(l.label), count: count > 0 ? count : nil,
                          indent: CGFloat(l.depth) * WhooshSpacing.s12 + (l.hasChildren ? 0 : WhooshSpacing.s16),
                          chevron: l.hasChildren ? open : nil,
                          selected: state.selection.id == "label.\(l.id)", selectionNS: ns, selectionEpoch: state.selection.epoch,
                          pinned: l.label.pinned, onPin: { state.togglePin(l.id, model: model) },
                          pinAx: "mac-main.sidebar.label.\(l.id).pin", forceHover: state.hoverLabel == l.id, scopeAccount: owner,
                          onChevron: { withAnimation(SidebarMotion.disclosure(reduce: reduce)) { state.toggleLabel(l.id) } }) {
            state.focusLabel(l.id, reduce: reduce)
            focused = true
        }
        .help(l.label.path)
        .contextMenu { SidebarMenuContent(subject: .label(l.id), model: model, state: state) }
        .transition(.opacity)
        .opacity(dragged ? 0.4 : 1)
        .overlay { if LabelInspector.editingLabelId == l.id { EditRing() } }   // the label being modified wears the edit ring
        .overlay { LabelDropMark(zone: state.drag?.target?.rowId == l.id ? state.drag?.target?.zone : nil) }
        .overlay(alignment: .top) { Color.clear.frame(height: WhooshSpacing.s4).allowsHitTesting(false).whooshAx("mac-main.sidebar.label.\(l.id).drop-before") }
        .overlay(alignment: .bottom) { Color.clear.frame(height: WhooshSpacing.s4).allowsHitTesting(false).whooshAx("mac-main.sidebar.label.\(l.id).drop-after") }
        .reportLabelFrame(l.id)
        .whooshAx("mac-main.sidebar.label.\(l.id)")
        .simultaneousGesture(dragGesture(l, all: all))
    }

    private func dragGesture(_ l: SidebarLabelRow, all: [SidebarLabelRow]) -> some Gesture {
        DragGesture(minimumDistance: WhooshSpacing.s6, coordinateSpace: .named(LabelDrag.space))
            .onChanged { v in
                state.drag = SidebarLabelDragState(id: l.id, point: v.location,
                                                   target: LabelDrag.target(at: v.location, frames: state.labelFrames, dragging: l.id))
            }
            .onEnded { v in
                let t = LabelDrag.target(at: v.location, frames: state.labelFrames, dragging: l.id)
                withAnimation(WhooshMotion.hover) { state.drag = nil }
                guard let t else { return }
                state.land(t, moving: l.id, model: model)
            }
    }

    /// The name of the label being dragged, riding beside the pointer.
    @ViewBuilder private func ghost(_ all: [SidebarLabelRow]) -> some View {
        if let d = state.drag, let name = all.first(where: { $0.id == d.id })?.label.name {
            Text(name).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.onAccent)
                .padding(.horizontal, WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s4)
                .background(Capsule().fill(WhooshPalette.accent))
                .position(x: d.point.x + WhooshSpacing.s16, y: d.point.y)
                .allowsHitTesting(false).transition(.opacity)
        }
    }
}

extension SidebarState {
    func focusLabel(_ id: String, reduce: Bool = false) {
        labelFocus = id
        MacShellState.shared.focusedPane = .sidebar   // a click on a label gives the sidebar the keys
        select("label.\(id)", reduce: reduce)
    }

    /// A jump to a label (the palette, a key): opens every ancestor, then selects it like a click. Refuses when the label or an ancestor
    /// belongs to an account the filter does not show (the row would not be drawn at any fold state).
    @discardableResult func revealLabel(_ id: String, reduce: Bool = false) -> Bool {
        let all = SidebarData.labels()
        guard let row = all.first(where: { $0.id == id }) else { return false }
        let byId = Dictionary(all.map { ($0.id, $0.label) }, uniquingKeysWith: { a, _ in a })
        let filter = AppModel.shared.accountFilter
        guard Self.scopeShows(row.label, filter: filter) else { return false }
        var chain: [String] = []
        var parent = row.label.parentId
        while let p = parent {
            guard !chain.contains(p) else { return false }
            chain.append(p)
            if let up = byId[p], !Self.scopeShows(up, filter: filter) { return false }
            parent = byId[p]?.parentId
        }
        if !chain.isEmpty { withAnimation(SidebarMotion.disclosure(reduce: reduce)) { openLabels.formUnion(chain) } }
        focusLabel(id, reduce: reduce)
        return true
    }

    /// Right: a folded label that has children opens. Anything else is not the sidebar's (the arrow then moves the pane focus).
    func labelRight(reduce: Bool = false) -> Bool {
        guard let id = labelFocus, let row = SidebarData.labels().first(where: { $0.id == id }), row.hasChildren, !openLabels.contains(id) else { return false }
        withAnimation(SidebarMotion.disclosure(reduce: reduce)) { _ = openLabels.insert(id) }
        return true
    }

    /// Left: an open label folds. Anything else is not the sidebar's.
    func labelLeft(reduce: Bool = false) -> Bool {
        guard let id = labelFocus, openLabels.contains(id) else { return false }
        withAnimation(SidebarMotion.disclosure(reduce: reduce)) { _ = openLabels.remove(id) }
        return true
    }
}
