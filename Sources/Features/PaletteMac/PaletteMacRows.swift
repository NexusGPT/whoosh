import SwiftUI

/// One palette line: an optional leading mark, the title, then its key caps (a command) or its detail (a jump).
/// The highlight snaps with the selection (docs/MOTION.md rule 3); hover tints within HOVER_TINT_MS.
struct PaletteRowView: View {
    let item: PaletteItem
    let highlighted: Bool
    @State private var hovering = false

    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            if hasMark { mark.frame(width: PaletteMacMetrics.markSlot) }
            title
            Spacer(minLength: WhooshSpacing.s8)
            trailing
        }
        .padding(.horizontal, WhooshSpacing.s10)
        .frame(height: PaletteMacMetrics.rowHeight)
        .background(fill, in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
        .animation(WhooshMotion.hover, value: hovering)
    }

    private var hasMark: Bool { item.isJump && item.kind != .settings }
    private var fill: AnyShapeStyle {
        highlighted ? AnyShapeStyle(WhooshPalette.selection) : (hovering ? AnyShapeStyle(WhooshPalette.hover) : AnyShapeStyle(.clear))
    }
    private var primary: AnyShapeStyle { highlighted ? AnyShapeStyle(WhooshPalette.onAccent) : AnyShapeStyle(WhooshPalette.label) }
    private var secondary: AnyShapeStyle { highlighted ? AnyShapeStyle(WhooshPalette.onAccent) : AnyShapeStyle(WhooshPalette.secondaryLabel) }

    @ViewBuilder private var mark: some View {
        switch item.kind {
        case .account:
            if let e = item.emblem {
                EmblemMark(glyph: EmblemLibrary.resolve(e.iconId), colourHex: e.colour, size: .compact, name: e.name, address: e.address)
            }
        case .tab: Image(systemName: "rectangle.split.2x1").foregroundStyle(secondary)
        case .label: Image(systemName: "tag").foregroundStyle(secondary)
        default: EmptyView()
        }
    }

    /// A label path reads "Work › Client" in the quiet colour and "Provanta" in the loud one.
    @ViewBuilder private var title: some View {
        if item.kind == .label, let cut = item.title.range(of: " › ", options: .backwards) {
            HStack(spacing: 0) {
                Text(String(item.title[..<cut.upperBound])).foregroundStyle(secondary).lineLimit(1).truncationMode(.head)
                Text(String(item.title[cut.upperBound...])).foregroundStyle(primary).lineLimit(1).layoutPriority(1)
            }
            .whooshText(WhooshTypography.body, emphasized: highlighted)
        } else {
            Text(item.title).whooshText(WhooshTypography.body, emphasized: highlighted).foregroundStyle(primary).lineLimit(1)
        }
    }

    @ViewBuilder private var trailing: some View {
        if item.isJump {
            Text(item.detail).whooshText(WhooshTypography.subhead).foregroundStyle(secondary).lineLimit(1)
        } else if !item.caps.isEmpty {
            HStack(spacing: WhooshSpacing.s2) { ForEach(item.caps, id: \.self) { KeyCap($0, onSelection: highlighted) } }
        }
    }
}

/// The account picker's line ("Move to another account"): the emblem, the name, the address.
struct PaletteAccountRowView: View {
    let account: AccountBook.Entry
    let highlighted: Bool
    @State private var hovering = false

    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            EmblemMark(glyph: EmblemLibrary.resolve(account.iconId), colourHex: account.colour, size: .compact, name: account.name, address: account.address)
                .frame(width: PaletteMacMetrics.markSlot)
            Text(account.name).whooshText(WhooshTypography.body, emphasized: highlighted)
                .foregroundStyle(highlighted ? WhooshPalette.onAccent : WhooshPalette.label).lineLimit(1)
            Spacer(minLength: WhooshSpacing.s8)
            Text(account.address).whooshText(WhooshTypography.subhead).lineLimit(1)
                .foregroundStyle(highlighted ? AnyShapeStyle(WhooshPalette.onAccent) : AnyShapeStyle(WhooshPalette.secondaryLabel))
        }
        .padding(.horizontal, WhooshSpacing.s10)
        .frame(height: PaletteMacMetrics.rowHeight)
        .background(highlighted ? AnyShapeStyle(WhooshPalette.selection) : (hovering ? AnyShapeStyle(WhooshPalette.hover) : AnyShapeStyle(.clear)),
                    in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
        .contentShape(Rectangle())
        .onHover { hovering = $0 }
        .animation(WhooshMotion.hover, value: hovering)
    }
}
