import SwiftUI

/// The inspector's small controls, drawn from the Native Glass tokens: the pop-up pill, the colour swatches, the key-capped
/// button, the quiet error line. The rows and cards are the shared form grid (Components/FormGrid.swift). None knows about labels.

struct ChangeDot: View {
    var body: some View {
        Circle().fill(WhooshPalette.accent).frame(width: WhooshSpacing.s6, height: WhooshSpacing.s6)
            .transition(.opacity).accessibilityHidden(true)
    }
}

/// A pop-up pill: the word and the up-down chevrons. The menu itself is the system's; the pill is the drawing's `.pop`.
struct PopPill<Label: View>: View {
    @ViewBuilder var label: Label
    @State private var hover = false
    var body: some View {
        HStack(spacing: WhooshSpacing.s4) {
            label.whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label).lineLimit(1)
            Image(systemName: "chevron.up.chevron.down").font(WhooshTypography.caption.font).imageScale(.small).foregroundStyle(WhooshPalette.secondaryLabel)
        }
        .padding(.leading, WhooshSpacing.s8).padding(.trailing, WhooshSpacing.s6).padding(.vertical, WhooshSpacing.s4)
        .background(RoundedRectangle(cornerRadius: WhooshRadius.item, style: .continuous).fill(hover ? WhooshPalette.hover : WhooshPalette.fill))
        .onHover { hover = $0 }
        .animation(WhooshMotion.hover, value: hover)
        .contentShape(Rectangle())
    }
}

/// The colour row: none, then the six. The chosen one wears a ring.
struct ColourSwatches: View {
    let chosen: String?
    let pick: (String?) -> Void
    var body: some View {
        HStack(spacing: WhooshSpacing.s2) {
            swatch(nil)
            ForEach(LabelPalette.swatches, id: \.self) { swatch($0) }
        }
    }
    private func swatch(_ hex: String?) -> some View {
        let on = chosen == hex
        return Button { pick(hex) } label: {
            Circle().fill(hex.map(AccountColour.ring) ?? .clear).frame(width: WhooshSpacing.s14, height: WhooshSpacing.s14)
                .overlay { Circle().strokeBorder(hex == nil ? WhooshPalette.tertiaryLabel : WhooshPalette.separator, lineWidth: WhooshMetrics.separator) }
                .padding(WhooshSpacing.s2)
                .overlay { Circle().strokeBorder(WhooshPalette.secondaryLabel, lineWidth: WhooshSpacing.s2).opacity(on ? 1 : 0) }
                .animation(WhooshMotion.hover, value: on)
        }
        .buttonStyle(.plain)
        .help(hex == nil ? Words.text("No colour") : Words.text("Colour %@", hex ?? ""))
        .accessibilityLabel(hex == nil ? Words.text("No colour") : Words.text("Colour %@", hex ?? ""))
        .accessibilityIdentifier("mac-labels.inspector.colour.\(hex ?? "none")")
    }
}

/// A button that names its key on its face (a key cap) and in its tooltip.
struct KeyedButton: View {
    let title: String
    let key: String
    var primary = false
    var enabled = true
    let action: () -> Void
    @State private var hover = false
    var body: some View {
        Button(action: action) {
            HStack(spacing: WhooshSpacing.s6) {
                Text(title).whooshText(WhooshTypography.body, emphasized: primary)
                KeyCap(key, onSelection: primary && enabled)
            }
            .foregroundStyle(!enabled ? WhooshPalette.tertiaryLabel : (primary ? WhooshPalette.onAccent : WhooshPalette.label))
            .padding(.leading, WhooshSpacing.s12).padding(.trailing, WhooshSpacing.s6).frame(height: WhooshMetrics.segmentHeight)
            .background(RoundedRectangle(cornerRadius: WhooshRadius.row, style: .continuous)
                .fill(primary && enabled ? AnyShapeStyle(WhooshPalette.filledControl) : AnyShapeStyle(hover && enabled ? WhooshPalette.hover : WhooshPalette.fill)))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain).disabled(!enabled)
        .onHover { hover = $0 }
        .animation(WhooshMotion.hover, value: hover)
        .help("\(title) · \(key)")
        .accessibilityLabel(title).accessibilityHint(Words.text("Key %@", key))
    }
}

/// An orange dot and one sentence under the row it is about (the lock's orange; no red).
struct InlineError: View {
    let words: String
    var body: some View {
        HStack(alignment: .top, spacing: WhooshSpacing.s6) {
            Image(systemName: "exclamationmark.circle").font(.system(size: WhooshTypography.caption.size)).foregroundStyle(WhooshPalette.warning)
            Text(words).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.label).fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, WhooshSpacing.s4).padding(.top, WhooshSpacing.s6)
        .transition(.opacity)
        .accessibilityElement(children: .combine)
    }
}
