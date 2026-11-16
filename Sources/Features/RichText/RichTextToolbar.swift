import SwiftUI

#if canImport(AppKit)
/// The contextual toolbar: it appears over the selection, never in a fixed bar. Entrance is the
/// popover rule of docs/MOTION.md (scale from its anchor over POPOVER_MS, set by RichTextEditor);
/// a press tints within HOVER_TINT_MS and acts on the same frame.
struct RichTextToolbar: View {
    let session: RichTextSession

    /// One button of the bar: its id, glyph, name, key and what it does. The key shows in the tooltip.
    struct Item {
        let id: String, symbol: String, title: String, key: String
        let act: (RichTextSession) -> Void
        let on: (RichFormatState) -> Bool
    }

    /// The bar's buttons in order, in three groups: marks, link and colour, blocks. Shared with the composer's control table.
    static let items: [[Item]] = [
        [Item(id: "bold", symbol: "bold", title: "Bold", key: "⌘B", act: { $0.hooks.toggleBold() }, on: { $0.bold }),
         Item(id: "italic", symbol: "italic", title: "Italic", key: "⌘I", act: { $0.hooks.toggleItalic() }, on: { $0.italic }),
         Item(id: "underline", symbol: "underline", title: "Underline", key: "⌘U", act: { $0.hooks.toggleUnderline() }, on: { $0.underline }),
         Item(id: "strike", symbol: "strikethrough", title: "Strikethrough", key: "⌘⇧X", act: { $0.hooks.toggleStrike() }, on: { $0.strike })],
        [Item(id: "link", symbol: "link", title: "Link", key: "⌘K", act: { $0.requestLink() }, on: { _ in false }),
         Item(id: "color", symbol: "paintpalette", title: "Colour", key: "⌘O", act: { $0.requestColor() }, on: { $0.color != nil })],
        [Item(id: "list", symbol: "list.bullet", title: "Bulleted list", key: "⌘⇧8", act: { $0.hooks.toggleList() }, on: { $0.list == "bullet" }),
         Item(id: "numbers", symbol: "list.number", title: "Numbered list", key: "⌘⇧7", act: { $0.hooks.toggleNumbered() }, on: { $0.list == "number" }),
         Item(id: "quote", symbol: "text.quote", title: "Quote", key: "⌘⇧9", act: { $0.hooks.toggleQuote() }, on: { $0.quote })],
    ]

    var body: some View {
        _ = session.document          // an edit redraws the bar, so a button shows whether its format is on
        let state = session.hooks.formatState()
        return HStack(spacing: WhooshSpacing.s2) {
            ForEach(Array(Self.items.enumerated()), id: \.offset) { g, group in
                if g > 0 { Rectangle().fill(WhooshPalette.separator).frame(width: WhooshMetrics.separator, height: WhooshSpacing.s16) }
                ForEach(group, id: \.id) { item in button(item, on: item.on(state)) }
            }
        }
        .padding(WhooshSpacing.s4)
        .whooshSystemMaterial(.regularMaterial, in: RoundedRectangle(cornerRadius: WhooshRadius.control))
        .overlay(RoundedRectangle(cornerRadius: WhooshRadius.control).strokeBorder(WhooshGlass.rim, lineWidth: WhooshGlass.rimWidth))
        .accessibilityIdentifier("mac-composer.format-bar")
    }

    private func button(_ item: Item, on: Bool) -> some View {
        let tip = Words.text(item.title) + "  " + item.key
        return Button { item.act(session) } label: {
            Image(systemName: item.symbol).font(WhooshTypography.body.font(emphasized: true))
                .foregroundStyle(on ? WhooshPalette.accent : WhooshPalette.label)
                .frame(width: WhooshSpacing.s24 + WhooshSpacing.s4, height: WhooshSpacing.s24)
                .background(on ? AnyShapeStyle(WhooshPalette.mine) : AnyShapeStyle(.clear), in: RoundedRectangle(cornerRadius: WhooshRadius.item))
        }
        .buttonStyle(RichToolbarButtonStyle())
        .help(tip)
        .accessibilityLabel(tip)
        .accessibilityIdentifier("mac-composer.format-bar.\(item.id)")
    }
}

private struct RichToolbarButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        Face(label: configuration.label, pressed: configuration.isPressed)
    }

    private struct Face<L: View>: View {
        let label: L
        let pressed: Bool
        @State private var hover = false
        var tint: AnyShapeStyle {
            if pressed { return AnyShapeStyle(WhooshPalette.fill) }
            return hover ? AnyShapeStyle(WhooshPalette.hover) : AnyShapeStyle(Color.clear)
        }
        var body: some View {
            label
                .foregroundStyle(WhooshPalette.label)
                .background(RoundedRectangle(cornerRadius: WhooshRadius.item).fill(tint))
                .onHover { hover = $0 }
                .animation(WhooshMotion.hover, value: hover)
        }
    }
}
#endif
