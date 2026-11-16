import SwiftUI

/// A 28 pt icon button of the composer foot: the glyph, a tooltip, the hover tint, and an "on" tint while its panel is open.
struct ComposerIconButton: View {
    let symbol: String
    let label: String
    /// The key bound to this control, shown in its tooltip beside the name.
    var key: String? = nil
    var active = false
    var disabled = false
    var id: String
    var action: () -> Void
    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: WhooshSpacing.s14, weight: .medium))
                .foregroundStyle(active ? WhooshPalette.accent : WhooshPalette.secondaryLabel)
                .frame(width: WhooshMetrics.actionButton, height: WhooshMetrics.actionButton)
                .background(active ? AnyShapeStyle(WhooshPalette.mine) : AnyShapeStyle(.clear), in: RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
                .hoverAck(radius: WhooshRadius.control)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(disabled)
        .opacity(disabled ? 0.4 : 1)
        .help(key.map { label + "  " + $0 } ?? label)
        .accessibilityLabel(key.map { label + ", key " + $0 } ?? label)
        .whooshAx(id)
    }
}

/// A recipient chip: the address, a remove button. Quiet fill, no border.
struct RecipientChip: View {
    let address: String
    var name: String? = nil
    /// The door of its × (`mac-composer.<field>.chip.<i>.remove`).
    var removeAx: String
    var onRemove: () -> Void
    var body: some View {
        HStack(spacing: WhooshSpacing.s4) {
            Text(name ?? address).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label).lineLimit(1).help(address)
            Button(action: onRemove) {
                Image(systemName: "xmark").font(.system(size: WhooshTypography.caption.size - WhooshSpacing.s2, weight: .bold))
                    .foregroundStyle(WhooshPalette.secondaryLabel)
            }.buttonStyle(.plain).accessibilityLabel(ComposerWords.remove(address)).whooshAx(removeAx)
        }
        .padding(.horizontal, WhooshSpacing.s8).frame(height: WhooshSpacing.s20 + WhooshSpacing.s2)
        .background(WhooshPalette.fill, in: Capsule())
    }
}

/// The send button: the filled control with the Send word and its key.
struct ComposerSendButton: View {
    let enabled: Bool
    var action: () -> Void
    var body: some View {
        Button(action: action) {
            HStack(spacing: WhooshSpacing.s8) {
                Text(ComposerWords.send).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.onAccent)
                KeyCap("⌘↵", onSelection: true)
            }
            .padding(.horizontal, WhooshSpacing.s14).frame(height: WhooshMetrics.glassPillHeight - WhooshSpacing.s2)
            .background(WhooshPalette.filledControl, in: Capsule())
            .opacity(enabled ? 1 : 0.45)
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .keyboardShortcut(.return, modifiers: .command)
        .help(ComposerControlKeys.tooltip(ComposerIds.send))
        .accessibilityLabel(ComposerWords.sendLabel)
        .whooshAx(ComposerIds.send)
    }
}

/// "Sent": the capsule that replaces the undo bar once the window has closed. Undo is no longer offered.
struct SentCapsule: View {
    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            Image(systemName: "checkmark.circle.fill").foregroundStyle(WhooshPalette.accent)
            Text(ComposerWords.sent).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.label)
        }
        .padding(.horizontal, WhooshSpacing.s16).frame(height: WhooshMetrics.segmentHeight)
        .whooshGlass(in: Capsule())
        .transition(.opacity)
        .whooshAx(ComposerIds.undoBar)
    }
}
