import SwiftUI

/// The sidebar: a search field (⌘F), the accounts summary card, then the panes in four named groups; while a search
/// runs, the list of settings it found. See-through material, the blue selection (S3).
struct SettingsSidebar: View {
    /// False at the minimum window height: the summary card makes room for the panes.
    var showsSummary = true
    @State private var model = SettingsWindowModel.shared
    @State private var accounts = AccountsSession.shared
    @Namespace private var selection
    @FocusState private var searchFocused: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            SettingsSearchField(model: model, focused: $searchFocused)
                .padding(.horizontal, 2).padding(.bottom, 10)
            if model.searching { SettingsResults(model: model) } else { panes }
            Spacer(minLength: 0)
        }
        .padding(.top, SettingsMetrics.sidebarTop).padding(.horizontal, SettingsMetrics.sidebarSide)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background { SettingsSidebarMaterial() }
        .overlay(alignment: .trailing) { Rectangle().fill(WhooshPalette.sidebarEdge).frame(width: 0.5) }
        .onChange(of: model.searchRequests) { _, _ in searchFocused = true }
        .animation(WhooshMotion.animation(.easeOut(duration: WhooshMotion.readerCrossFade), reduceMotion: reduceMotion), value: model.searching)
        .whooshAx("mac-settings-window.sidebar")
    }

    private var panes: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if showsSummary { SettingsAccountsSummary(model: model, accounts: accounts) }
                ForEach(SettingsPane.Group.allCases, id: \.self) { group in
                    Text(group.title).font(.system(size: 11, weight: .semibold)).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                        .padding(.horizontal, 8).frame(height: SettingsMetrics.groupHeaderHeight, alignment: .leading)
                        .padding(.top, group == SettingsPane.Group.allCases.first ? 0 : 8)
                        .accessibilityAddTraits(.isHeader)
                    ForEach(group.panes) { pane in
                        row(pane)
                        if pane == .replies && model.repliesOpen { repliesChildren }
                    }
                    .animation(WhooshMotion.animation(WhooshMotion.unfold, reduceMotion: reduceMotion), value: model.repliesOpen)
                }
            }
            .background(alignment: .topLeading) {   // the one blue selection, never a source: it takes the selected row's frame and slides there
                if let p = slidingSelection {
                    RoundedRectangle(cornerRadius: SettingsMetrics.paneRowRadius, style: .continuous).fill(WhooshPalette.accent)
                        .matchedGeometryEffect(id: p.id, in: selection, isSource: false)
                        .animation(WhooshMotion.animation(WhooshMotion.tabThumb, reduceMotion: reduceMotion), value: model.pane)
                }
            }
        }
        .scrollIndicators(.never)
    }

    /// The six parts of Réponses, indented under their parent (layout L2, `.sbr.kid`): 23 pt high, 12.5 pt, the label under the parent's
    /// title; the selected part is blue. The parent row stays plain while a part is open.
    private var repliesChildren: some View {
        VStack(spacing: 0) { ForEach(RepliesPage.panes) { p in
            let on = model.place.reply == p || model.place.reply?.parent == p
            Button { model.openReply(p) } label: {
                Text(p.title).font(.system(size: 12.5)).lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .foregroundStyle(on ? AnyShapeStyle(Color.white) : AnyShapeStyle(WhooshPalette.sidebarSecondaryLabel))
                    .padding(.leading, 34).padding(.trailing, 6).frame(height: SettingsMetrics.childRowHeight)
                    .background {
                        if on { RoundedRectangle(cornerRadius: SettingsMetrics.paneRowRadius, style: .continuous).fill(WhooshPalette.accent) }
                    }
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(on ? .isSelected : [])
            .whooshAx("mac-settings-window.pane-row.replies.\(p.rawValue)")
        } }
        .transition(.opacity)
    }

    /// The disclosure chevron of the Réponses row: right while closed, down while open. It opens and closes the parts and never moves the selection.
    private func repliesChevron(on: Bool) -> some View {
        Image(systemName: "chevron.right").font(.system(size: 9, weight: .bold))
            .rotationEffect(.degrees(model.repliesOpen ? 90 : 0))
            .foregroundStyle(on ? AnyShapeStyle(Color.white.opacity(0.85)) : AnyShapeStyle(WhooshPalette.sidebarSecondaryLabel))
            .frame(width: 18, height: SettingsMetrics.paneRowHeight)
            .contentShape(Rectangle())
            .onTapGesture { model.toggleReplies() }
            .accessibilityLabel(Words.text("Replies"))
            .whooshAx("mac-settings-window.pane-row.replies.chevron")
    }

    /// The pane whose row the sliding selection sits under: none while a Réponses part is the blue row, or when the move is a long jump (it fades).
    private var slidingSelection: SettingsPane? {
        guard model.lastDistance <= WhooshConstants.SLIDE_MAX_ROWS, !(model.pane == .replies && model.repliesOpen) else { return nil }
        return model.pane
    }

    private func row(_ pane: SettingsPane) -> some View {
        let on = model.pane == pane && !(pane == .replies && model.repliesOpen)   // an open Réponses: the selected part is the blue row, never its parent
        let slides = model.lastDistance <= WhooshConstants.SLIDE_MAX_ROWS
        return Button { model.select(pane) } label: {
            HStack(spacing: 8) {
                SettingsPaneIcon(pane: pane)
                Text(pane.title).font(.system(size: 13, weight: pane == .replies ? .semibold : .regular)).lineLimit(1).frame(maxWidth: .infinity, alignment: .leading)
                if pane == .accounts, !accounts.failing.isEmpty {
                    Text("\(accounts.failing.count)").font(.system(size: 10.5, weight: .semibold)).foregroundStyle(Color.white)
                        .padding(.horizontal, 4).frame(minWidth: 16, minHeight: 16)
                        .background(Capsule().fill(WhooshPalette.warning))
                        .whooshAx("mac-settings-window.badge.accounts")
                }
                if !pane.aiNeeds.isEmpty { MissingSetupDot(needs: pane.aiNeeds, id: "mac-settings-window.dot.\(pane.id)") }   // ai-setup W1: the warn dot where the fix lives
                if !pane.chord.isEmpty { SettingsKey(text: pane.chord, onSelection: on) }
                if pane == .replies { repliesChevron(on: on) }
            }
            .foregroundStyle(on ? AnyShapeStyle(Color.white) : AnyShapeStyle(WhooshPalette.label))
            .padding(.horizontal, 6).frame(height: SettingsMetrics.paneRowHeight)
            // hid-ai: each row is the ONE source of its own id at all times; the blue shape of `panes` follows the selected row. The shape used to
            // be the source of "sel" inside `if on`, held by two rows in the same transaction (SwiftUI's MatchedGeometryScope trap, see SettingsKit).
            .matchedGeometryEffect(id: pane.id, in: selection, isSource: true)
            .background {
                if on && !slides {   // a long jump fades in place, as before
                    RoundedRectangle(cornerRadius: SettingsMetrics.paneRowRadius, style: .continuous).fill(WhooshPalette.accent).transition(.opacity)
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .help(pane.chord.isEmpty ? pane.title : "\(pane.title) (\(pane.chord))")
        .animation(WhooshMotion.animation(slides ? WhooshMotion.tabThumb : .easeOut(duration: WhooshMotion.hoverTint), reduceMotion: reduceMotion), value: model.pane)
        .accessibilityAddTraits(on ? .isSelected : [])
        .whooshAx("mac-settings-window.pane-row.\(pane.id)")
    }
}

/// `.srch` the search field: a magnifier, the text, a clear button and `esc` while it holds text, ⌘F while it is empty.
struct SettingsSearchField: View {
    let model: SettingsWindowModel
    var focused: FocusState<Bool>.Binding

    var body: some View {
        let text = Binding(get: { model.query }, set: { model.setQuery($0) })
        HStack(spacing: 6) {
            Image(systemName: "magnifyingglass").font(.system(size: 12, weight: .medium)).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
            TextField(Words.text("Search"), text: text).textFieldStyle(.plain).font(.system(size: 13))
                .foregroundStyle(WhooshPalette.label)
                .focused(focused)
                .onSubmit { model.commitSearch() }
                .onKeyPress(.upArrow) { model.moveSelection(-1); return .handled }
                .onKeyPress(.downArrow) { model.moveSelection(1); return .handled }
                .onKeyPress(.escape) { model.clearQuery(); focused.wrappedValue = false; return .handled }
            if model.query.isEmpty {
                SettingsKey(text: "⌘F")
            } else {
                Button { model.clearQuery() } label: {
                    Image(systemName: "xmark.circle.fill").font(.system(size: 13)).foregroundStyle(WhooshPalette.tertiaryLabel)
                }
                .buttonStyle(.plain).help(Words.text("Clear the search")).whooshAx("mac-settings-window.search.clear")
                SettingsKey(text: "esc")
            }
        }
        .padding(.leading, 8).padding(.trailing, 6).frame(height: SettingsMetrics.searchHeight)
        .whooshAx("mac-settings-window.search")
        .background(RoundedRectangle(cornerRadius: SettingsMetrics.searchRadius, style: .continuous)
            .fill(focused.wrappedValue ? AnyShapeStyle(WhooshPalette.background) : AnyShapeStyle(WhooshPalette.fill)))
        .overlay(RoundedRectangle(cornerRadius: SettingsMetrics.searchRadius, style: .continuous)
            .strokeBorder(focused.wrappedValue ? WhooshPalette.accent : WhooshPalette.separator, lineWidth: 0.5))
        .overlay(RoundedRectangle(cornerRadius: SettingsMetrics.searchRadius + 1.5, style: .continuous)
            .stroke(WhooshPalette.focusHalo, lineWidth: 3).padding(-1.5).opacity(focused.wrappedValue ? 1 : 0))
        .animation(.easeOut(duration: WhooshMotion.hoverTint), value: focused.wrappedValue)
    }
}

/// `.sum` the accounts card at the top: the first five marks, "+N", the count and how many need you. It opens Accounts.
struct SettingsAccountsSummary: View {
    let model: SettingsWindowModel
    let accounts: AccountsSession

    var body: some View {
        let all = accounts.accounts
        let needs = accounts.failing.count
        Button { model.select(.accounts) } label: {
            HStack(spacing: 10) {
                HStack(spacing: -5) {
                    ForEach(Array(all.prefix(5).enumerated()), id: \.element.id) { i, a in
                        EmblemMark(glyph: EmblemLibrary.resolve(a.iconId), colourHex: a.colour, size: .points(22),
                                   state: accounts.state(a.id)?.lastError != nil ? .needsSignIn : .live,
                                   imagePath: a.imagePath, name: a.name, address: a.address)
                            .zIndex(Double(5 - i))
                    }
                    if all.count > 5 {
                        Text("+\(all.count - 5)").font(.system(size: 11)).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                            .lineLimit(1).fixedSize().padding(.leading, 9)
                    }
                }
                .fixedSize()
                VStack(alignment: .leading, spacing: 0) {
                    Text(Self.count(all.count)).font(.system(size: 12.5, weight: .semibold)).foregroundStyle(WhooshPalette.label)
                        .lineLimit(1).minimumScaleFactor(0.8)
                    if needs > 0 {
                        HStack(spacing: 5) {
                            Circle().fill(WhooshPalette.warning).frame(width: 7, height: 7)
                            Text(Words.text("%lld needs you", needs)).font(.system(size: 11)).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                        }
                    } else if !all.isEmpty {
                        Text(Words.text("All working")).font(.system(size: 11)).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                    }
                }
                Spacer(minLength: 0)
            }
            .padding(8)
            .background(RoundedRectangle(cornerRadius: SettingsMetrics.summaryRadius, style: .continuous).fill(WhooshPalette.hover))
            .overlay(RoundedRectangle(cornerRadius: SettingsMetrics.summaryRadius, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: 0.5))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .padding(.bottom, 6)
        .help(Words.text("Accounts"))
        .whooshAx("mac-settings-window.summary")
    }

    static func count(_ n: Int) -> String { n == 1 ? Words.text("1 account") : Words.text("%@ accounts", n) }
}

/// The settings a search found, each with its coloured square and its path; the chosen one is blue.
struct SettingsResults: View {
    let model: SettingsWindowModel
    var body: some View {
        let results = model.results
        VStack(alignment: .leading, spacing: 0) {
            Text(Words.text("Settings · %lld", results.count)).font(.system(size: 11, weight: .semibold))
                .foregroundStyle(WhooshPalette.sidebarSecondaryLabel).padding(.horizontal, 8).frame(height: 22, alignment: .leading)
                .whooshAx("mac-settings-window.results.count")
            if results.isEmpty {
                Text(Words.text("No setting matches that.")).font(.system(size: 11.5)).foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                    .padding(.horizontal, 8).padding(.top, 6).whooshAx("mac-settings-window.results.empty")
            }
            ScrollView {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(Array(results.enumerated()), id: \.element.id) { i, e in
                        SettingsResultRow(entry: e, query: model.query, on: i == model.selectedResult) { model.open(e) }
                            .whooshAx("mac-settings-window.result.\(i)")
                    }
                }
            }
            .scrollIndicators(.never)
            Text(Words.text("↑ ↓ move · Return opens · esc clears and brings the panes back")).font(.system(size: 11))
                .foregroundStyle(WhooshPalette.sidebarSecondaryLabel).lineSpacing(1).fixedSize(horizontal: false, vertical: true)
                .padding(.horizontal, 8).padding(.vertical, 10)
        }
        .transition(.opacity)
    }

    /// The matched part of the title in bold.
    static func highlight(_ title: String, _ query: String) -> AttributedString {
        var out = AttributedString(title)
        let q = query.trimmingCharacters(in: .whitespaces)
        if !q.isEmpty, let r = title.range(of: q, options: [.caseInsensitive, .diacriticInsensitive]),
           let a = AttributedString.Index(r.lowerBound, within: out), let b = AttributedString.Index(r.upperBound, within: out) {
            out[a..<b].font = .system(size: 13, weight: .bold)
        }
        return out
    }
}

/// One found setting: its coloured square, its title with the match in bold, its path.
struct SettingsResultRow: View {
    let entry: SettingsEntry
    let query: String
    let on: Bool
    let open: () -> Void

    var body: some View {
        Button(action: open) {
            HStack(spacing: 9) {
                SettingsPaneIcon(pane: entry.pane)
                VStack(alignment: .leading, spacing: 0) {
                    Text(SettingsResults.highlight(entry.title, query)).font(.system(size: 13)).lineLimit(1)
                    Text(entry.path).font(.system(size: 11)).lineLimit(1).foregroundStyle(pathStyle)
                }
                Spacer(minLength: 0)
            }
            .foregroundStyle(titleStyle)
            .padding(.horizontal, 8).padding(.vertical, 6)
            .background(RoundedRectangle(cornerRadius: 8, style: .continuous).fill(fill))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private var titleStyle: AnyShapeStyle { on ? AnyShapeStyle(Color.white) : AnyShapeStyle(WhooshPalette.label) }
    private var pathStyle: AnyShapeStyle { on ? AnyShapeStyle(Color.white.opacity(0.8)) : AnyShapeStyle(WhooshPalette.sidebarSecondaryLabel) }
    private var fill: AnyShapeStyle { on ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(Color.clear) }
}

#if os(macOS)
import AppKit

/// `.sb`: the desktop, blurred behind the window (AppKit's own sidebar material), with the board's light tint over it.
struct SettingsSidebarMaterial: View {
    var body: some View {
        ZStack {
            Group { if WhooshRigStandIn.isOn { Rectangle().fill(WhooshRigStandIn.underlay) } else { SettingsVibrancy() } }
            Rectangle().fill(SettingsPalette.sidebarTint)
        }
    }
}

struct SettingsVibrancy: NSViewRepresentable {
    func makeNSView(context: Context) -> NSVisualEffectView {
        let v = NSVisualEffectView()
        v.material = .sidebar
        v.blendingMode = .behindWindow
        v.state = .followsWindowActiveState
        return v
    }
    func updateNSView(_ v: NSVisualEffectView, context: Context) {}
}
#else
struct SettingsSidebarMaterial: View {
    var body: some View { Rectangle().fill(SettingsPalette.sidebarTint) }
}
#endif
