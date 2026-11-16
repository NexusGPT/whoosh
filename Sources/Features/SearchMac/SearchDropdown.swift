import SwiftUI

/// The glass dropdown under the field: Search all mail, People, Labels and smart folders, the three "contains" words, recent searches.
/// One highlight glides between rows (docs/MOTION.md rule 11: it slides when the new row is at most SEARCH_SLIDE_MAX_ROWS rows away, else it
/// fades out and in). The dropdown itself fades in and rises 2 pt over SEARCH_DROPDOWN_ENTER_MS and is on screen the frame the key lands.
struct SearchDropdown: View {
    let model: SearchMacModel
    let accounts: [String: RowAccount]
    @State private var shown = 0
    @State private var glow = 1.0
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    static let rowHeight: CGFloat = 30
    static let groupGap: CGFloat = 9
    static let width: CGFloat = 404

    /// The y of row `i`: the rows above it, plus a hairline gap where the group changes.
    private func top(_ i: Int, _ rows: [SearchSuggestion]) -> CGFloat {
        guard rows.indices.contains(i) else { return 0 }
        let gaps = (1...max(i, 1)).filter { $0 <= i && rows[$0 - 1].group != rows[$0].group }.count
        return CGFloat(i) * Self.rowHeight + CGFloat(gaps) * Self.groupGap
    }

    var body: some View {
        let rows = model.suggestions
        VStack(alignment: .leading, spacing: 0) {
            ZStack(alignment: .topLeading) {
                RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous).fill(WhooshPalette.hover)
                    .overlay(RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous).stroke(WhooshGlass.rimHighlight, lineWidth: WhooshGlass.rimWidth))
                    .frame(height: Self.rowHeight)
                    .offset(y: top(min(shown, max(rows.count - 1, 0)), rows)).opacity(glow)
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(Array(rows.enumerated()), id: \.element.id) { i, s in
                        if i > 0, rows[i - 1].group != s.group {
                            Rectangle().fill(WhooshPalette.separator).frame(height: WhooshGlass.rimWidth)
                                .padding(.horizontal, WhooshSpacing.s10).padding(.vertical, WhooshSpacing.s4)
                        }
                        SuggestionRow(suggestion: s, accounts: accounts)
                            .frame(height: Self.rowHeight)
                            .contentShape(Rectangle())
                            .background(RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous).fill(WhooshPalette.hover.opacity(model.hovered == i ? 0.5 : 0)))   // the pointer's own tint, apart from the keyboard highlight
                            .animation(.easeOut(duration: Double(WhooshConstants.SEARCH_HOVER_TINT_MS) / 1000), value: model.hovered)
                            .onHover { if $0 { model.hovered = i } else if model.hovered == i { model.hovered = nil } }
                            .onTapGesture { model.choose(i); _ = model.enter() }
                            .accessibilityIdentifier("mac-search.dropdown.\(i)")
                    }
                }
            }
            DropdownKeys()
        }
        .padding(WhooshSpacing.s6)
        .frame(maxWidth: Self.width)   // the list pane's width at most: the pane clips what leaves it
        .whooshGlass(in: RoundedRectangle(cornerRadius: 13, style: .continuous))
        .background(RoundedRectangle(cornerRadius: 13, style: .continuous).fill(WhooshPalette.background.opacity(0.82)))   // the rows behind (the inbox list) do not show through
        .shadow(color: .black.opacity(0.16), radius: 22, y: 10)
        .onAppear { shown = model.highlight }
        .onChange(of: model.highlight) { old, new in glide(old, new) }
        .whooshAx("mac-search.dropdown")
    }

    /// The highlight follows the model's: a slide when near, a fade out and in when far, none under Reduced Motion's cross-fade rule.
    private func glide(_ old: Int, _ new: Int) {
        let near = abs(new - old) <= WhooshConstants.SEARCH_SLIDE_MAX_ROWS
        if near || reduceMotion {
            withAnimation(reduceMotion ? nil : .spring(response: 0.22, dampingFraction: 0.9)) { shown = new }
        } else {
            let fade = Double(WhooshConstants.SEARCH_HOVER_TINT_MS) / 1000
            withAnimation(.linear(duration: fade)) { glow = 0 }
            Task { @MainActor in
                try? await Task.sleep(nanoseconds: UInt64(fade * 1_000_000_000))
                shown = new
                withAnimation(.linear(duration: fade)) { glow = 1 }
            }
        }
    }
}

/// The dropdown's entrance as its parent applies it: fade and a 2 pt rise, no delay.
extension AnyTransition {
    static func dropdown(reduceMotion: Bool) -> AnyTransition {
        reduceMotion ? .opacity : .opacity.combined(with: .offset(y: 2))
    }
}

struct SuggestionRow: View {
    let suggestion: SearchSuggestion
    let accounts: [String: RowAccount]

    private var symbol: String {
        switch suggestion.kind {
        case .search: return "magnifyingglass"
        case .from: return "envelope"
        case .to: return "paperplane"
        case .label: return "tag"
        case .folder: return "folder"
        case .subject: return "doc.text"
        case .attachment: return "paperclip"
        case .filter: return suggestion.chip?.kind == .attachment ? "paperclip" : "calendar"
        case .recent: return "clock"
        case .domain: return "at"
        }
    }

    var body: some View {
        HStack(spacing: WhooshSpacing.s8 + 1) {
            if suggestion.kind == .domain {   // the drawing's "@" in the icon's place (.sx-at: 14 wide, semibold)
                Text("@").font(.system(size: 12.5, weight: .semibold)).foregroundStyle(WhooshPalette.secondaryLabel).frame(width: 14)
            } else {
                Image(systemName: symbol).font(.system(size: 12.5)).foregroundStyle(WhooshPalette.secondaryLabel).frame(width: 14)
            }
            content
        }
        .padding(.leading, WhooshSpacing.s10).padding(.trailing, WhooshSpacing.s8 + 1)
        .lineLimit(1)
    }

    /// The kind's word before the value: "From", "To", "Subject", "Attachment".
    var kindWord: String {
        switch suggestion.kind {
        case .from: return Words.text("From")
        case .to: return Words.text("To")
        case .subject: return Words.text("Subject")
        case .attachment: return Words.text("Attachment")
        case .label: return Words.text("Label")
        case .domain: return Words.text("From domain")
        default: return ""
        }
    }

    @ViewBuilder private var content: some View {
        switch suggestion.kind {
        case .search:
            sentence
            Spacer(minLength: WhooshSpacing.s8)
            Image(systemName: "return").font(.system(size: 11, weight: .semibold)).foregroundStyle(WhooshPalette.secondaryLabel)
        case .from, .to:
            (Text(kindWord + " ").foregroundStyle(WhooshPalette.secondaryLabel) + named).whooshTextOnly.lineLimit(1)
            Spacer(minLength: WhooshSpacing.s8)
            Text(suggestion.detail).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1).truncationMode(.tail)
        case .domain:
            (Text(kindWord + " ").foregroundStyle(WhooshPalette.secondaryLabel) + named).whooshTextOnly.lineLimit(1).truncationMode(.tail)
            Spacer(minLength: WhooshSpacing.s8)
            Text(suggestion.detail).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
        case .subject, .attachment, .label:
            (Text(kindWord + " ").foregroundStyle(WhooshPalette.secondaryLabel) + named).whooshTextOnly.lineLimit(1).truncationMode(.tail)
        case .folder:
            named.whooshTextOnly
            Spacer(minLength: WhooshSpacing.s8)
            Text(suggestion.detail).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.tertiaryLabel)
        case .filter, .recent:
            Text(suggestion.title).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label).truncationMode(.tail)
        }
    }

    /// "Search all mail for “claire”": the typed words in bold.
    private var sentence: some View {
        let typed = suggestion.title
        let full = Words.text("Search all mail for “%@”", typed)
        guard let r = full.range(of: typed) else { return Text(full).whooshTextOnly }
        return (Text(full[..<r.lowerBound]) + Text(full[r]).fontWeight(.semibold) + Text(full[r.upperBound...])).whooshTextOnly
    }

    /// The value with the typed letters in bold.
    private var named: Text {
        let t = suggestion.title
        guard let r = suggestion.bold, !r.isEmpty, r.upperBound <= t.count else { return Text(t) }
        let a = t.index(t.startIndex, offsetBy: r.lowerBound), b = t.index(t.startIndex, offsetBy: r.upperBound)
        return Text(t[..<a]) + Text(t[a..<b]).fontWeight(.semibold) + Text(t[b...])
    }
}

private extension Text {
    var whooshTextOnly: some View { self.font(WhooshTypography.body.font(emphasized: false)).foregroundStyle(WhooshPalette.label) }
}

/// The keys the dropdown answers to, quiet at its foot.
struct DropdownKeys: View {
    var body: some View {
        ViewThatFits(in: .horizontal) {
            row(labels: true)
            row(labels: true, tight: true)   // the drawing labels every key (designs.js `keys`): a narrower pane first closes the gaps (round 19, d1ffe36c: bare caps at 960)
            row(labels: false)   // a pane too narrow even then (or longer words in another language): the keys alone
        }
        .padding(.horizontal, WhooshSpacing.s10).padding(.top, WhooshSpacing.s8).padding(.bottom, WhooshSpacing.s2)
        .padding(.top, WhooshSpacing.s2 + 1)
        .overlay(alignment: .top) { Rectangle().fill(WhooshPalette.separator).frame(height: WhooshGlass.rimWidth).padding(.horizontal, WhooshSpacing.s10) }
        .padding(.top, WhooshSpacing.s4)
    }

    private func row(labels: Bool, tight: Bool = false) -> some View {
        HStack(spacing: tight ? WhooshSpacing.s6 : labels ? WhooshSpacing.s14 : WhooshSpacing.s10) {
            hint(["↑", "↓"], labels ? Words.text("Move") : nil); hint(["Tab"], labels ? Words.text("Add as filter") : nil)
            hint(["↵"], labels ? Words.text("Search") : nil); hint(["Esc"], labels ? Words.text("Close") : nil)
            Spacer(minLength: 0)
        }
        .fixedSize(horizontal: true, vertical: false)
    }

    private func hint(_ keys: [String], _ label: String?) -> some View {
        HStack(spacing: WhooshSpacing.s4) {
            ForEach(keys, id: \.self) { k in
                Text(k).font(.system(size: 10, weight: .semibold)).foregroundStyle(WhooshPalette.secondaryLabel)
                    .padding(.horizontal, WhooshSpacing.s4).frame(minWidth: 16, minHeight: 16)
                    .overlay(RoundedRectangle(cornerRadius: WhooshRadius.keyCap, style: .continuous).stroke(WhooshPalette.separator, lineWidth: WhooshGlass.rimWidth))
            }
            if let label { Text(label).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.tertiaryLabel).lineLimit(1) }
        }
    }
}

/// The dropdown as the list pane mounts it: under the field, inside the pane's width, drawn only while the search is open and typing.
struct SearchDropdownHost: View {
    let model: SearchMacModel
    @Bindable var app: AppModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let open = model.active && model.dropdownVisible
        SearchDropdown(model: model, accounts: open ? RowAccount.directory(app) : [:])
            .background(FrameReader { model.dropdownFrame = $0 })
            .padding(.horizontal, WhooshMetrics.listRowInset)
            .opacity(open ? 1 : 0).offset(y: open ? 0 : (reduceMotion ? 0 : 2))
            .animation(WhooshMotion.animation(.easeOut(duration: Double(WhooshConstants.SEARCH_DROPDOWN_ENTER_MS) / 1000), reduceMotion: reduceMotion), value: open)
            .allowsHitTesting(open).accessibilityHidden(!open)
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
    }
}
