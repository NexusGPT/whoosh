import SwiftUI

/// The list (board a; owner's picks of 14 Nov 2026): the search field on the first line, then the latest mail, newest first, unread in bold; under
/// the Unread reading the list ends with the plain end mark and the unread count. Words typed in the field give the list's place to the
/// search's answer: the matches, then under the word "Related" the related mail. The foot says the panel is out of screen shares, and the keys.
struct NotchListView: View {
    let model: NotchModel

    var body: some View {
        let all = model.listed
        let shown = Array(all.prefix(NotchMetrics.listMax))
        VStack(alignment: .leading, spacing: WhooshSpacing.s2) {
            NotchSearchField(model: model).padding(.bottom, WhooshSpacing.s4)
            if model.search.active {
                answer
            } else if all.isEmpty {
                NotchDoneRow(title: NotchCopy.empty).whooshAx(NotchIds.empty)
            } else {
                ForEach(Array(shown.enumerated()), id: \.element.id) { i, item in
                    NotchRowView(item: item, index: i + 1, buttons: model.settings.buttons, keyed: model.rowFocusId == item.id)
                }
                if all.count > shown.count {
                    Text(NotchCopy.more(all.count - shown.count))
                        .whooshText(WhooshTypography.caption)
                        .foregroundStyle(WhooshPalette.secondaryLabel)
                        .padding(.horizontal, WhooshSpacing.s8)
                }
                if model.settings.show == .unread {
                    NotchDoneRow(title: NotchCopy.empty, detail: NotchCopy.unreadCount(model.unreadCount))
                        .overlay(alignment: .top) { Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator) }
                        .padding(.top, WhooshSpacing.s4)
                        .whooshAx(NotchIds.end)
                        .transition(.opacity)
                }
            }
            foot.padding(.top, WhooshSpacing.s6)
        }
        .animation(WhooshMotion.hover, value: model.settings.show)
    }

    /// The search's answer in the list's place: the engine's own sentence when it refused the words; "No match"; else the mails that hold the
    /// typed letters, then the related mail under its word. While the first answer is on its way the rows of the last answer stay.
    @ViewBuilder private var answer: some View {
        let search = model.search
        if let refusal = search.refusal {
            Label(refusal, systemImage: "exclamationmark.circle")
                .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.warning)
                .padding(.horizontal, WhooshSpacing.s8).frame(minHeight: WhooshMetrics.listRow)
                .whooshAx(NotchIds.searchResults)
        } else if search.results.isEmpty {
            Text(search.waiting ? " " : NotchCopy.noMatch)
                .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                .padding(.horizontal, WhooshSpacing.s8).frame(minHeight: WhooshMetrics.listRow)
                .whooshAx(NotchIds.searchResults)
        } else {
            ForEach(Array(search.results.enumerated()), id: \.element.id) { i, item in
                if i == search.matches {
                    Text(NotchCopy.related)
                        .whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.tertiaryLabel)
                        .padding(.horizontal, WhooshSpacing.s8).padding(.top, i == 0 ? 0 : WhooshSpacing.s4)
                }
                NotchRowView(item: item, index: i + 1, buttons: model.settings.buttons, keyed: model.rowFocusId == item.id, ax: NotchIds.searchRow(i + 1),
                             detail: search.gists[item.id] ?? item.preview)
            }
        }
    }

    private var foot: some View {
        HStack(spacing: WhooshSpacing.s6) {
            if model.settings.hiddenInScreenShare {
                Image(systemName: "eye.slash").font(.system(size: WhooshTypography.caption.size, weight: .regular))
                    .notchTip(NotchCopy.hiddenWhileSharing)
                    .accessibilityLabel(NotchCopy.hiddenWhileSharing)
                // by the pointer alone the note says its words; with the keyboard in the notch the keys need the line, and the note is its
                // picture (the words in its tip)
                if !model.keyboardWanted { Text(NotchCopy.hiddenWhileSharing).whooshText(WhooshTypography.caption).lineLimit(1) }
            }
            Spacer(minLength: WhooshSpacing.s8)
            NotchIconButton(symbol: "gearshape", title: NotchCopy.settings) {
                #if os(macOS)
                _ = WindowBridge.open(.macSettingsGeneral, in: .shared)
                #endif
            }
            .whooshAx(NotchIds.settings)
            NotchKeyHints(model: model).fixedSize()
        }
        .foregroundStyle(WhooshPalette.tertiaryLabel)
        .padding(.horizontal, WhooshSpacing.s8)
        .whooshAx(NotchIds.foot)
    }
}
