import SwiftUI

@MainActor private func panelTitle(_ text: String, key: String) -> some View {
    HStack {
        Text(text).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
        Spacer(minLength: 0)
        KeyCap(key)
    }
    .padding(.horizontal, WhooshSpacing.s10).padding(.top, WhooshSpacing.s4)
}

/// The list `;name` and `:smi` open under the caret: snippets by name, emoji by name. Up and Down move, Return or Tab
/// inserts, Esc closes it; a click inserts the row. The highlighted row shows the key that inserts it.
struct ComposerInlineList: View {
    @Bindable var model: ComposerModel
    static let width = WhooshSpacing.s32 * 9

    var body: some View {
        PickerPanel(width: Self.width) {
            ForEach(Array(model.inlineItems.enumerated()), id: \.element.id) { i, item in
                Button { _ = model.pickInline(i) } label: {
                    PickerRow(title: item.title, highlighted: i == model.inlineIndex) {
                        if i == model.inlineIndex { KeyCap("↵", onSelection: true) }
                        if !item.glyph.isEmpty { Text(item.glyph).whooshText(WhooshTypography.title3) }
                    }
                }
                .buttonStyle(.plain).whooshAx(ComposerIds.inlineChoice(i))
            }
        }
        .fixedSize()
        .whooshAx(ComposerIds.inline)
    }
}

/// ⌘K: the address of the link. Return applies it, Esc closes it. With text selected the text becomes the link; with
/// none, the address goes in as the linked text.
struct LinkPanel: View {
    @Bindable var model: ComposerModel
    @FocusState private var focused: Bool

    var body: some View {
        PickerPanel(width: WhooshSpacing.s32 * 11) {
            panelTitle(ComposerWords.link, key: "⌘K")
            TextField(ComposerWords.linkPlaceholder, text: $model.linkInput)
                .textFieldStyle(.plain).whooshText(WhooshTypography.body).foregroundStyle(WhooshPalette.label)
                .focused($focused).onSubmit { _ = model.applyLink() }
                .padding(.horizontal, WhooshSpacing.s10).frame(height: WhooshMetrics.sidebarRow + WhooshSpacing.s4)
                .background(WhooshPalette.field, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
                .whooshAx(ComposerIds.linkField)
            HStack(spacing: WhooshSpacing.s8) {
                Spacer(minLength: 0)
                Button { model.closePanels(); model.session.hooks.focusBody() } label: {
                    HStack(spacing: WhooshSpacing.s6) { Text(ComposerWords.cancel); KeyCap("Esc") }
                        .whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel)
                }.buttonStyle(.plain).whooshAx(ComposerIds.linkCancel)
                Button { _ = model.applyLink() } label: {
                    HStack(spacing: WhooshSpacing.s6) { Text(ComposerWords.apply); KeyCap("↵") }
                        .whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.accent)
                }.buttonStyle(.plain).whooshAx(ComposerIds.linkApply)
            }
            .padding(.horizontal, WhooshSpacing.s10).padding(.vertical, WhooshSpacing.s4)
        }
        .fixedSize()
        .onAppear { focused = true }
        .whooshAx(ComposerIds.link)
    }
}

/// ⌘O: the text colour. Left and Right move, Return applies; a swatch applies on a click. Only palette tokens are offered.
struct ColorPanel: View {
    @Bindable var model: ComposerModel

    var body: some View {
        PickerPanel(width: WhooshSpacing.s32 * 11) {
            panelTitle(ComposerWords.colour, key: "⌘O")
            HStack(spacing: WhooshSpacing.s4) {
                ForEach(Array(ComposerColor.all.enumerated()), id: \.element.slug) { i, c in
                    Button { _ = model.pickColor(c.name) } label: {
                        VStack(spacing: WhooshSpacing.s4) {
                            Circle().fill(c.name.flatMap(RichColor.init(rawValue:))?.token ?? WhooshPalette.label)
                                .frame(width: WhooshSpacing.s16, height: WhooshSpacing.s16)
                            Text(c.title).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                        }
                        .frame(maxWidth: .infinity).padding(.vertical, WhooshSpacing.s6)
                        .background(i == model.listIndex ? AnyShapeStyle(WhooshPalette.mine) : AnyShapeStyle(.clear),
                                    in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
                        .contentShape(Rectangle()).hoverAck(radius: WhooshRadius.control)
                    }
                    .buttonStyle(.plain).whooshAx(ComposerIds.colorChoice(c.slug))
                }
            }
        }
        .fixedSize()
        .whooshAx("mac-composer.color")
    }
}
