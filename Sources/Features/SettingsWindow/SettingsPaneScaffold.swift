import SwiftUI

/// A pane: its header (icon, name, one line, buttons at the right), then its groups, in one scrolling column.
/// The column scrolls to a found row and never grows past `contentMax`.
struct SettingsPaneScaffold<Trailing: View, Content: View>: View {
    let pane: SettingsPane
    @ViewBuilder var trailing: Trailing
    @ViewBuilder var content: Content
    private let model = SettingsWindowModel.shared

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    SettingsPaneHeader(pane: pane) { trailing }
                    content
                }
                .frame(maxWidth: SettingsMetrics.contentMax, alignment: .leading)
                .padding(.horizontal, SettingsMetrics.contentSide)
                .padding(.top, SettingsMetrics.contentTop).padding(.bottom, SettingsMetrics.contentBottom)
                .frame(maxWidth: .infinity)
            }
            .modifier(SettingsScrollEdge())
            // A found setting is SHOWN, when the ring changes in the open pane and when a pane opens already ringed (one task covers both: it starts with the
            // view and again at each new ring). The scroll is asked once the column is laid out, then once more without animation: asked in the same turn
            // or one turn later it moved nothing (main182, the hidden copy at the minimum window: 23 of the found entries stayed below the foot, rows included),
            // and an ask for a setting already in place moves nothing.
            .task(id: model.ring) {
                guard let ring = model.ring else { return }
                try? await Task.sleep(for: .milliseconds(WhooshConstants.SETTINGS_FOUND_SCROLL_MS))
                guard !Task.isCancelled, model.ring == ring else { return }
                withAnimation(.easeInOut(duration: WhooshMotion.readerCrossFade)) { proxy.scrollTo(ring, anchor: .center) }
                try? await Task.sleep(for: .milliseconds(WhooshConstants.SETTINGS_FOUND_SCROLL_AGAIN_MS))
                guard !Task.isCancelled, model.ring == ring else { return }
                proxy.scrollTo(ring, anchor: .center)
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("mac-settings-window.pane.\(pane.id)")
    }
}

/// The one edge of every Settings column under the toolbar (hid-ai round 21, main111 b1d38560: a row scrolled under the toolbar was cut flat through its
/// letters). While the column is scrolled, its top `SettingsMetrics.scrollEdge` points fade into the window's background; at rest nothing is drawn, so a
/// pane that has not moved looks as before. Off the frame path: the scroll's geometry is read as one Bool and the action runs only when it flips (the
/// column leaves or reaches its top); the fade is a plain gradient layer over the column, never a mask or an offscreen pass, and takes no click.
struct SettingsScrollEdge: ViewModifier {
    @State private var scrolled = false
    @Environment(\.colorScheme) private var scheme

    func body(content: Content) -> some View {
        let bg = WhooshPalette.background.color(scheme)
        content
            .onScrollGeometryChange(for: Bool.self) { g in g.contentOffset.y + g.contentInsets.top > 0.5 } action: { _, now in scrolled = now }
            .overlay(alignment: .top) {
                LinearGradient(colors: [bg, bg.opacity(0)], startPoint: .top, endPoint: .bottom)
                    .frame(height: SettingsMetrics.scrollEdge)
                    .opacity(scrolled ? 1 : 0)
                    .allowsHitTesting(false).accessibilityHidden(true)
            }
    }
}

extension SettingsPaneScaffold where Trailing == EmptyView {
    init(pane: SettingsPane, @ViewBuilder content: () -> Content) {
        self.init(pane: pane, trailing: { EmptyView() }, content: content)
    }
}

/// `.phead`: the pane's coloured square, its name and one line, and the buttons that belong to the pane as a whole.
struct SettingsPaneHeader<Trailing: View>: View {
    let pane: SettingsPane
    @ViewBuilder var trailing: Trailing
    /// The icon, name and one line keep at least this much: four 32 pt steps and the icon. Below it the buttons go to a row of their own.
    static var labelMin: CGFloat { WhooshSpacing.s32 * 7 }

    var body: some View {
        FormSplit(labelMin: Self.labelMin, probeId: "mac-settings-window.header.\(pane.id)", title: pane.title, caption: pane.line) {
            HStack(alignment: .center, spacing: 14) {
                SettingsPaneIcon(pane: pane, size: SettingsMetrics.headerIcon)
                VStack(alignment: .leading, spacing: 1) {
                    Text(pane.title).font(.system(size: 17, weight: .semibold)).tracking(-0.2).foregroundStyle(WhooshPalette.label)
                        .lineLimit(1).fixedSize(horizontal: false, vertical: true).accessibilityAddTraits(.isHeader)
                        .whooshAx("mac-settings-window.title.\(pane.id)")
                    Text(pane.line).font(.system(size: 12)).foregroundStyle(WhooshPalette.secondaryLabel).fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 0)
            }
        } control: {
            HStack(spacing: 8) { trailing }
        }
        .padding(.top, 4).padding(.bottom, 10)
        .whooshAx("mac-settings-window.header.\(pane.id)")
    }
}

/// A pane that has no settings yet: a card saying what comes, and no control that does nothing.
struct SettingsPlaceholderPane: View {
    let pane: SettingsPane
    var body: some View {
        SettingsPaneScaffold(pane: pane) {
            SettingsComing(pane: pane, lines: Self.lines(pane))
            SettingsFootnote(text: Words.text("Nothing here changes yet. These settings arrive in a later update."), ax: "mac-settings-window.\(pane.id).note")
        }
    }

    static func lines(_ pane: SettingsPane) -> [String] {
        switch pane {
        case .viewing: return [Words.text("Message font and size"), Words.text("Load remote images: ask, always or never"), Words.text("Mark as read after a pause in the reading pane")]
        case .signatures: return [Words.text("One signature per account, plain or rich"), Words.text("A different signature for replies")]
        case .junk: return [Words.text("What counts as junk, and how sure Whoosh must be"), Words.text("When junk is deleted for good")]
        default: return []
        }
    }
}
