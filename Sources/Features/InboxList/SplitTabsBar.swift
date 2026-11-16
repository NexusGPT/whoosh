import SwiftUI

/// The toolbar's glass segmented control (`inbox/tabs`, PinnedTabs): the two views first (Primary, All), a hairline, the
/// pinned labels in the sidebar's order, then a "+N" chip for the labels past TAB_PINNED_MAX. The strip takes the row's
/// full width: the Unread filter is no longer here, it is the count line's control (UnreadCount, board unread-filter-r1 F2). The thumb morphs to the next tab over TAB_MORPH_MS (WhooshMotion.tabThumb). A label tab's
/// context menu unpins it; a view carries no label control at all. There is no editor: the sidebar and the label sheet
/// pin, order and rename.
struct SplitTabsBar: View {
    let model: AppModel
    @Namespace private var thumb
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    /// The strip never sets the pane's width: inside a narrow pane it scrolls sideways and keeps the chosen tab in view. The "+N"
    /// chip sits outside the scroll, so it is always on screen however many tabs the strip holds.
    var body: some View {
        let parts = PinnedTabs.split(PinnedTabs.all(model))
        return HStack(spacing: WhooshSpacing.s4) {
            ScrollViewReader { proxy in
                ScrollView(.horizontal, showsIndicators: false) { strip(parts.shown) }
                    .onChange(of: model.tab) { _, t in withAnimation(WhooshMotion.animation(WhooshMotion.tabThumb, reduceMotion: reduceMotion)) { proxy.scrollTo(t) } }
            }
            // a strip wider than its pane fades out at the trailing edge instead of cutting a word in half; the scroll still reaches the rest
            .mask(alignment: .trailing) {
                HStack(spacing: 0) {
                    Rectangle()
                    LinearGradient(colors: [.black, .clear], startPoint: .leading, endPoint: .trailing).frame(width: WhooshSpacing.s24)
                }
            }
            if !parts.folded.isEmpty {
                chip(parts.folded).padding(WhooshSpacing.s2).whooshGlass(in: Capsule())
            }
        }
        .padding(.horizontal, WhooshMetrics.listRowInset)
        .frame(maxWidth: .infinity)
    }

    private func strip(_ shown: [PinnedTab]) -> some View {
        HStack(spacing: 0) {
            ForEach(Array(shown.enumerated()), id: \.element.id) { i, t in
                segment(i, t).id(t.id)
                // a hairline between the views and the labels: the two kinds of tab read as two groups
                if t.isView, shown.indices.contains(i + 1), !shown[i + 1].isView {
                    Rectangle().fill(WhooshPalette.separator).frame(width: 1, height: WhooshSpacing.s14).padding(.horizontal, WhooshSpacing.s2)
                        .accessibilityHidden(true)
                }
            }
        }
        .background {   // the one thumb, never a source: it takes the current tab's frame (no tab is current while a mailbox shows)
            if model.mailbox == nil, shown.contains(where: { $0.id == model.tab }) {
                Capsule().fill(WhooshPalette.thumb)
                    .shadow(color: WhooshGlass.thumbShadow.color(.light), radius: WhooshGlass.thumbShadowBlur, y: WhooshGlass.thumbShadowY)
                    .matchedGeometryEffect(id: model.tab, in: thumb, isSource: false)
            }
        }
        .padding(WhooshSpacing.s2)
        .whooshGlass(in: Capsule())
        .accessibilityIdentifier("mac-main.splits")
    }

    private func go(_ id: String) {
        withAnimation(WhooshMotion.animation(WhooshMotion.tabThumb, reduceMotion: reduceMotion)) { _ = PinnedTabs.open(id, model) }
    }

    /// `id` nil: the "+N" chip, which sits in its own capsule outside the row of tabs: it draws its own thumb while one of its labels is open.
    private func label(_ title: String, unread: Int, on: Bool, id: String? = nil) -> some View {
        HStack(spacing: WhooshSpacing.s4) {
            Text(title).font(WhooshTypography.subhead.font(emphasized: on))
            if unread > 0 { Text("\(unread)").font(WhooshTypography.caption.font(emphasized: false)).foregroundStyle(WhooshPalette.secondaryLabel) }
        }
        .lineLimit(1).fixedSize()
        .padding(.horizontal, WhooshSpacing.s10)
        .frame(height: WhooshMetrics.segmentHeight)
        .background {
            if id == nil, on {
                Capsule().fill(WhooshPalette.thumb)
                    .shadow(color: WhooshGlass.thumbShadow.color(.light), radius: WhooshGlass.thumbShadowBlur, y: WhooshGlass.thumbShadowY)
            }
        }
        .matchedGeometryEffect(id: id ?? "mac-main.splits.more", in: thumb, isSource: true)   // hid-ai round 4: one matched-geometry source per id (the MatchedGeometryScope trap of main88): each item is the source of its own id, one thumb follows the chosen one
    }

    private func segment(_ i: Int, _ t: PinnedTab) -> some View {
        let on = model.mailbox == nil && t.id == model.tab      // a mailbox shown: no tab is the current one
        return withMenu(t, Button { go(t.id) } label: { label(t.name, unread: t.unread, on: on, id: t.id) }
            .buttonStyle(.plain).foregroundStyle(WhooshPalette.label)
            .help(LiveKeys.tabHelp(t.name, about: t.isView ? Words.text("inbox.view.\(t.id).help", default: t.name) : nil)))
            .whooshAx("mac-main.splits.\(i)")
            .accessibilityAddTraits(on ? .isSelected : [])
    }

    /// The tab's context menu is its `menu` list: "unpin" for a label, nothing for a view (no menu is attached at all,
    /// so a right-click on Primary or All shows nothing label-like).
    @ViewBuilder private func withMenu(_ t: PinnedTab, _ button: some View) -> some View {
        if t.menu.contains("unpin") {
            button.contextMenu { Button(Words.text("Unpin")) { _ = PinnedTabs.setPinned(t.id, false, model) } }
        } else {
            button
        }
    }

    /// "+N": the pinned labels past the cap. Its menu opens one; the open one reads as the chip's state.
    private func chip(_ folded: [PinnedTab]) -> some View {
        let on = model.mailbox == nil && folded.contains { $0.id == model.tab }
        return Menu {
            ForEach(folded, id: \.id) { t in
                Button { go(t.id) } label: { Text(t.unread > 0 ? "\(t.name) · \(t.unread)" : t.name) }
            }
        } label: { label("+\(folded.count)", unread: 0, on: on) }
            .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
            .foregroundStyle(WhooshPalette.label)
            .help(Words.text("%lld more pinned labels", folded.count))
            .whooshAx("mac-main.splits.more")
    }
}
