import SwiftUI

/// The sidebar of this window: the one label tree (labels are shared by every account), the smart folders shown
/// under the label they are nested in, and the people. The main window's sidebar is the sidebar lane's; this one
/// draws the same data through the same source.
/// Where each draggable row sits in the tree's coordinate space, so a drag can find the row under the pointer.
private struct RowFrames: PreferenceKey {
    static let defaultValue: [String: CGRect] = [:]
    static func reduce(value: inout [String: CGRect], nextValue: () -> [String: CGRect]) { value.merge(nextValue()) { $1 } }
}

struct LabelsSidebar: View {
    let model: LabelsContactsModel
    @State private var dragId: String?
    @State private var dragPoint = CGPoint.zero
    @State private var frames: [String: CGRect] = [:]
    private static let space = "labelTree"
    private static let top = "top"

    /// The row under the pointer while a label is dragged ("top": the Labels header); never the label itself.
    private var target: String? {
        guard let d = dragId else { return nil }
        return frames.first { $0.key != d && $0.value.contains(dragPoint) }?.key
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
                header(LabelsWords.labels)
                    .frame(maxWidth: .infinity, alignment: .leading).contentShape(Rectangle())
                    .background(dropHighlight(target == Self.top))
                    .reportFrame(Self.top)
                    .whooshAx("mac-labels-contacts.labels.top")
                    .help(LabelsWords.dropHere)
                ForEach(model.labels.ordered()) { l in
                    let depth = model.labels.depth(l.id)
                    labelRow(l, depth: depth)
                    ForEach(model.folders.filter { $0.parentId == l.id || ($0.parentId == l.name && depth == 1) }, id: \.id) { f in
                        row(f.name, symbol: "sparkle.magnifyingglass", count: model.savedCount(f),
                            selected: model.pane == .folder && model.folderId == f.id) { _ = model.edit(f.id) }
                            .padding(.leading, WhooshSpacing.s16)
                            .transition(.opacity.combined(with: .move(edge: .top)))
                    }
                }
                HStack {
                    header(LabelsWords.smartFolders)
                    Spacer(minLength: 0)
                    Button { _ = model.startNewFolder() } label: {
                        Image(systemName: "plus").foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                            .frame(width: WhooshMetrics.actionIcon, height: WhooshMetrics.actionIcon).contentShape(Rectangle())
                    }
                    .buttonStyle(.plain).help(LabelsWords.newFolder).accessibilityLabel(LabelsWords.newFolder)
                    .accessibilityIdentifier("mac-labels-contacts.folder.new")
                }
                ForEach(model.folders.filter { $0.parentId == nil || model.labels.item($0.parentId ?? "") == nil }, id: \.id) { f in
                    row(f.name, symbol: "sparkle.magnifyingglass", detail: model.parentName(f.parentId), count: model.savedCount(f),
                        selected: model.pane == .folder && model.folderId == f.id) { _ = model.edit(f.id) }
                        .accessibilityIdentifier("mac-labels-contacts.sidebar.\(f.id)")
                }
                header(LabelsWords.people)
                ForEach(model.people, id: \.id) { p in
                    row(p.name, symbol: "person.crop.circle", selected: model.pane == .contact && model.card?.id == p.id) {
                        _ = model.open(address: p.addresses.first ?? "")
                    }
                    .accessibilityIdentifier("mac-labels-contacts.person.\(p.id)")
                }
            }
            .padding(.horizontal, WhooshSpacing.s10).padding(.bottom, WhooshSpacing.s8)
            .animation(.spring, value: model.folders)
            .animation(.spring, value: model.labels)
            .coordinateSpace(name: Self.space)
            .onPreferenceChange(RowFrames.self) { frames = $0 }
            .overlay(alignment: .topLeading) { ghost }
        }
        .scrollIndicators(.never)
        .padding(.top, WhooshMetrics.sidebarTop)
        .background { Color.clear.whooshSystemMaterial(.regularMaterial, in: Rectangle()).overlay(Rectangle().fill(WhooshSidebarMaterial.tint)) }
        .overlay(alignment: .trailing) { Rectangle().fill(WhooshSidebarMaterial.edge).frame(width: WhooshSidebarMaterial.edgeWidth) }
        .accessibilityIdentifier("mac-labels-contacts.sidebar")
    }

    private func labelRow(_ l: LabelItem, depth: Int) -> some View {
        let leaf = model.labels.children(of: l.id).isEmpty
        let selected = model.pane == .labels && model.selectedLabelId == l.id
        return HStack(spacing: WhooshSpacing.s8) {
            Image(systemName: leaf ? "tag" : "folder").foregroundStyle(WhooshPalette.accent).frame(width: WhooshMetrics.actionIcon)
            // the mark sits on the name's own Text, a leaf the `labels` verb can copy and judge for clipping (on the whole row it read no text: 0 of 21 judged)
            Text(l.name).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label).lineLimit(1).whooshAx("mac-labels-contacts.labels.\(l.id)")
            Spacer(minLength: WhooshSpacing.s4)
        }
        .padding(.horizontal, WhooshSpacing.s8).frame(minHeight: WhooshMetrics.sidebarRow)
        .background(RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous).fill(WhooshPalette.hover).opacity(selected ? 1 : 0))
        .background(dropHighlight(target == l.id, radius: WhooshRadius.row))
        .contentShape(Rectangle())
        .opacity(dragId == l.id ? 0.4 : 1)
        .hoverAck(radius: WhooshRadius.row)
        .animation(WhooshMotion.hover, value: selected)
        .padding(.leading, CGFloat(depth - 1) * WhooshSpacing.s16)
        .reportFrame(l.id)
        .onTapGesture { _ = model.selectLabel(l.id) }
        .gesture(DragGesture(minimumDistance: 6, coordinateSpace: .named(Self.space))
            .onChanged { v in dragId = l.id; dragPoint = v.location }
            .onEnded { v in
                dragPoint = v.location
                let t = target
                withAnimation(.spring) { dragId = nil }
                if let t { _ = model.moveLabel(l.id, into: t == Self.top ? nil : t) }
            })
        .accessibilityAddTraits(.isButton)
        .transition(.opacity)
    }

    /// The target the pointer is over: an accent outline that eases in.
    private func dropHighlight(_ on: Bool, radius: CGFloat = WhooshRadius.row) -> some View {
        RoundedRectangle(cornerRadius: radius, style: .continuous)
            .strokeBorder(WhooshPalette.accent, lineWidth: 2).opacity(on ? 1 : 0)
            .animation(WhooshMotion.hover, value: on)
    }

    /// The name of the label being dragged, riding beside the pointer.
    @ViewBuilder private var ghost: some View {
        if let d = dragId, let name = model.labels.item(d)?.name {
            Text(name).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.onAccent)
                .padding(.horizontal, WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s4)
                .background(Capsule().fill(WhooshPalette.accent))
                .position(x: dragPoint.x + WhooshSpacing.s16, y: dragPoint.y)
                .allowsHitTesting(false).transition(.opacity)
        }
    }

    private func header(_ title: String) -> some View {
        Text(title).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
            .padding(.horizontal, WhooshSpacing.s8).padding(.top, WhooshSpacing.s12).padding(.bottom, WhooshSpacing.s4)
    }

    private func row(_ title: String, symbol: String, detail: String? = nil, count: Int? = nil, selected: Bool,
                     action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: WhooshSpacing.s8) {
                Image(systemName: symbol).foregroundStyle(WhooshPalette.accent).frame(width: WhooshMetrics.actionIcon)
                VStack(alignment: .leading, spacing: 0) {
                    Text(title).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label).lineLimit(1)
                    if let detail { Text(LabelsWords.inFolder(detail)).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.sidebarSecondaryLabel) }
                }
                Spacer(minLength: WhooshSpacing.s4)
                if let count {
                    Text("\(count)").whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                        .contentTransition(.numericText())
                }
            }
            .padding(.horizontal, WhooshSpacing.s8).frame(minHeight: WhooshMetrics.sidebarRow)
            .background(RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous).fill(WhooshPalette.hover).opacity(selected ? 1 : 0))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .hoverAck(radius: WhooshRadius.row)
        .animation(WhooshMotion.hover, value: selected)
    }
}


private extension View {
    func reportFrame(_ key: String) -> some View {
        background(GeometryReader { g in Color.clear.preference(key: RowFrames.self, value: [key: g.frame(in: .named("labelTree"))]) })
    }
}
