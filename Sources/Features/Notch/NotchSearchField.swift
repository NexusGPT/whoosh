import SwiftUI

/// The list's first line: the search field (owner's pick 3A, 14 Nov 2026: "in the search field"). The pointer alone never puts the cursor in
/// it: the notch's key, "/" and a click on the field do, and each takes the keyboard for the notch (NotchModel.focusSearch). What is typed
/// replaces the list with the search's answer; Down leaves the field for the mails, Return opens the answer's first mail, Escape empties it.
struct NotchSearchField: View {
    let model: NotchModel
    @Bindable var app = AppModel.shared
    @FocusState private var focused: Bool

    private var text: Binding<String> {
        Binding(get: { model.search.text }, set: { new in if new != model.search.text { model.setSearch(new, app) } })
    }

    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: WhooshTypography.subhead.size, weight: .medium))
                .foregroundStyle(WhooshPalette.secondaryLabel)
            TextField(NotchCopy.searchPlaceholder, text: text)
                .textFieldStyle(.plain)
                .whooshText(WhooshTypography.body)
                .foregroundStyle(WhooshPalette.label)
                .focused($focused)
                .simultaneousGesture(TapGesture().onEnded { _ = model.focusSearch() })   // a click into the field asks for the keyboard
                .onSubmit { _ = model.enter(app) }
                // real keys: the shell's monitor leaves Down and Escape of a focused field to the field, so the field sends them on itself
                .onKeyPress(.downArrow) { model.moveRow(1) ? .handled : .ignored }
                .onKeyPress(.escape) { model.back(app) ? .handled : .ignored }
                .whooshAx(NotchIds.searchField)
            if model.search.active {
                Button { model.setSearch("", app) } label: {
                    Image(systemName: "xmark.circle.fill").font(.system(size: WhooshTypography.subhead.size)).foregroundStyle(WhooshPalette.tertiaryLabel)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(Words.text("Clear"))
                .whooshAx(NotchIds.searchClear)
            } else if !model.searchFocused {
                KeyCap(NotchFeature.searchKey)
            }
        }
        .padding(.horizontal, WhooshSpacing.s10)
        .frame(height: NotchMetrics.searchField)
        .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: NotchMetrics.rowCorner, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: NotchMetrics.rowCorner, style: .continuous)
                .strokeBorder(model.searchFocused ? AnyShapeStyle(WhooshPalette.accent.opacity(0.7)) : AnyShapeStyle(WhooshPalette.separator), lineWidth: WhooshMetrics.separator)
        }
        .contentShape(Rectangle())
        .onAppear { focused = model.searchFocused }
        .onChange(of: model.searchFocused) { _, on in focused = on }
        .onChange(of: focused) { _, on in if on, !model.searchFocused { _ = model.focusSearch() } }
        .accessibilityElement(children: .contain)
    }
}

/// The keys under the list, by where the keyboard is: in the field, on a mail, in an answer. With the pointer alone it is the close key.
struct NotchKeyHints: View {
    let model: NotchModel

    var body: some View {
        HStack(spacing: WhooshSpacing.s10) {
            if !model.keyboardWanted {
                hint(["Esc"], NotchCopy.close)
            } else if model.rowFocusId != nil {
                hint(["Down", "Up"], NotchCopy.select)
                hint(["Enter"], NotchCopy.open)
                hint([NotchFeature.archiveKey], NotchCopy.archive)
                hint([NotchFeature.replyKey], NotchCopy.reply)
                hint([NotchFeature.searchKey], NotchCopy.searchWord)
            } else if model.search.active {
                hint(["Down"], NotchCopy.toTheMails)
                hint(["Enter"], NotchCopy.open)
                hint([NotchFeature.openInWhooshKey], NotchCopy.openInWhoosh)
            } else {
                Text(NotchCopy.typeToSearch).whooshText(WhooshTypography.caption)
                hint(["Down"], NotchCopy.toTheMails)
                hint(["Esc"], NotchCopy.close)
            }
        }
        .lineLimit(1)
    }

    private func hint(_ keys: [String], _ words: String) -> some View {
        HStack(spacing: WhooshSpacing.s4) {
            ForEach(keys, id: \.self) { KeyCap($0) }
            Text(words).whooshText(WhooshTypography.caption)
        }
    }
}
