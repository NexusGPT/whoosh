import SwiftUI

/// A part of Réponses: its header (the reply square, the part's name, one line, the buttons that belong to it), then its groups, in one scrolling
/// column no wider than the window's other panes. The page « Quels messages » has no header: the toolbar names it.
struct RepliesScaffold<Trailing: View, Content: View>: View {
    let page: RepliesPage
    @ViewBuilder var trailing: Trailing
    @ViewBuilder var content: Content
    @State private var store = RepliesStore.shared
    /// The column's room, read when the window is resized (never per frame): a page the toolbar names lines its cards up with the toolbar's title.
    @State private var room: CGFloat = 0

    /// Where the toolbar's title starts, from the detail column's left edge: the toolbar's side, the back and forward capsule (two buttons and its
    /// 3 pt sides) and the toolbar's 12 pt gap (SettingsToolbar in SettingsWindowView.swift).
    private static var titleX: CGFloat { SettingsMetrics.toolbarSide + 2 * SettingsMetrics.navButton + 6 + 12 }

    /// hid-ai round 21 (main111 b1d38560, 1280x820: the cards started 65 pt right of the toolbar's title). A page with no header of its own (« Quels
    /// messages ») starts its column under the title when the window has the room, and moves left smoothly as the room shrinks, down to the usual side.
    /// The parts with a header keep the window's centred column, as every other pane.
    private var lead: CGFloat? {
        guard page.parent != nil, room > 0 else { return nil }
        let side = SettingsMetrics.contentSide
        return min(Self.titleX, max(side, room - SettingsMetrics.contentMax - side))
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if page.parent == nil { RepliesHeader(page: page) { trailing } }
                content
                SettingsNotice(text: store.error, ax: RepliesAx.id("error"))
            }
            .frame(maxWidth: SettingsMetrics.contentMax, alignment: .leading)
            .padding(.leading, lead ?? SettingsMetrics.contentSide).padding(.trailing, SettingsMetrics.contentSide)
            .padding(.top, SettingsMetrics.contentTop).padding(.bottom, SettingsMetrics.contentBottom)
            .frame(maxWidth: .infinity, alignment: lead == nil ? .center : .leading)
            .animation(.easeOut(duration: WhooshMotion.rowInsert), value: store.error)
        }
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { room = $0 }
        .modifier(SettingsScrollEdge())   // the window's one scroll edge (SettingsPaneScaffold.swift)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier(page.ax)
        .whooshAx(page.ax)
    }
}

extension RepliesScaffold where Trailing == EmptyView {
    init(page: RepliesPage, @ViewBuilder content: () -> Content) { self.init(page: page, trailing: { EmptyView() }, content: content) }
}

/// `.phead` of a part: the window's own header layout (FormSplit, so the buttons go under the words when the column is narrow).
struct RepliesHeader<Trailing: View>: View {
    let page: RepliesPage
    @ViewBuilder var trailing: Trailing

    var body: some View {
        FormSplit(labelMin: SettingsPaneHeader<EmptyView>.labelMin, probeId: RepliesAx.id("header", page.rawValue), title: page.title, caption: page.line) {
            HStack(alignment: .center, spacing: 14) {
                RepliesIcon()
                VStack(alignment: .leading, spacing: 1) {
                    Text(page.title).font(.system(size: 17, weight: .semibold)).tracking(-0.2).foregroundStyle(WhooshPalette.label)
                        .lineLimit(1).accessibilityAddTraits(.isHeader).whooshAx(RepliesAx.id("title", page.rawValue))
                    Text(page.line).font(.system(size: 12)).foregroundStyle(WhooshPalette.secondaryLabel).fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
            }
        } control: {
            HStack(spacing: 8) { trailing }
        }
        .padding(.top, 4).padding(.bottom, 10)
        .whooshAx(RepliesAx.id("header", page.rawValue))
    }
}

/// `.sec` + `.card` with an action at the right of the heading (« Essayer sur un message », « Le prompt de recherche »): the window's
/// SettingsGroup, whose note is plain words, gains a link here and nowhere else.
struct RepliesGroup<Content: View>: View {
    let title: String
    var note: String? = nil
    var link: (title: String, ax: String, action: () -> Void)? = nil
    var card = true
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: WhooshSpacing.s6) {
                Text(title).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label).accessibilityAddTraits(.isHeader)
                Spacer(minLength: WhooshSpacing.s8)
                if let note { Text(note).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).multilineTextAlignment(.trailing) }
                if let link { RepliesLink(title: link.title, ax: link.ax, action: link.action) }
            }
            .padding(.horizontal, WhooshSpacing.s4).padding(.bottom, WhooshSpacing.s6)
            if card { GroupedCard { content } } else { content }
        }
        .padding(.top, WhooshSpacing.s16)
    }
}

/// A grey link in a heading: it turns the accent colour under the pointer.
struct RepliesLink: View {
    let title: String
    let ax: String
    let action: () -> Void
    @State private var hover = false  // local-ui: hover tint only

    var body: some View {
        Button(action: action) {
            Text(title).whooshText(WhooshTypography.caption).foregroundStyle(hover ? WhooshPalette.accent : WhooshPalette.secondaryLabel)
        }
        .buttonStyle(.plain).onHover { hover = $0 }
        .animation(.easeOut(duration: WhooshMotion.hoverTint), value: hover)
        .whooshAx(ax)
    }
}

/// `.rv` + `.rcv`: a grey value and a chevron at the right of a row that opens something.
struct RepliesValue: View {
    let text: String
    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Text(text).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1).contentTransition(.numericText())
            Image(systemName: "chevron.right").font(.system(size: 11, weight: .semibold)).foregroundStyle(WhooshPalette.tertiaryLabel)
        }
    }
}

/// `.srow .ri`: the row's own symbol at its left, the AI's in the accent colour.
struct RepliesRowIcon: View {
    let symbol: String
    var ai = false
    var body: some View {
        Image(systemName: symbol).font(.system(size: 14, weight: .regular))
            .foregroundStyle(ai ? WhooshPalette.accent : WhooshPalette.secondaryLabel)
            .frame(width: 18, height: 18)
    }
}
