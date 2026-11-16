import SwiftUI

// TEMPORARY (docs/lanes/settingsreplies-mac.md "Findings" F2, F3): thin copies of three shared parts this lane may not edit. Each one is the
// original's body with ONE difference, named on it; when its owner lands the named change, this file is deleted and the call sites use the
// original. Nothing else in the feature may grow a copy.

/// Copy of SettingsPaneIcon (Sources/Features/SettingsWindow/SettingsKit.swift). Difference: it takes the symbol and the hue, because the
/// six parts of Réponses are not cases of `SettingsPane` yet (F2: `SettingsPaneIcon(symbol:hue:size:)`).
struct RepliesIcon: View {
    var size: CGFloat = SettingsMetrics.headerIcon
    static let symbol = "arrowshape.turn.up.left.fill"
    static let hue = SettingsHue.purple
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let hue = Self.hue.value(scheme)
        let top = Color(.sRGB, red: hue.red * 0.82 + 0.18, green: hue.green * 0.82 + 0.18, blue: hue.blue * 0.82 + 0.18)
        let radius = size >= SettingsMetrics.headerIcon ? SettingsMetrics.headerIconRadius : SettingsMetrics.paneIconRadius
        RoundedRectangle(cornerRadius: radius, style: .continuous)
            .fill(LinearGradient(colors: [top, hue.color], startPoint: .top, endPoint: .bottom))
            .overlay(RoundedRectangle(cornerRadius: radius, style: .continuous).strokeBorder(Color.black.opacity(0.12), lineWidth: 0.5))
            .overlay(Image(systemName: Self.symbol).font(.system(size: size * 0.5, weight: .medium)).foregroundStyle(Color.white))
            .frame(width: size, height: size)
            .accessibilityHidden(true)
    }
}

/// Copy of FlowCardShell (Sources/Features/LabelsContactsMac/LabelFlowCards.swift). Difference: its socket ids take `ax` instead of the label
/// inspector's `LabelGeo` ids, so these cards never enter `mac-labels.inspector.geometry` (F3: an `axBase` parameter on FlowCardShell).
/// Second difference (hid-ai round 17): the card's switch is the Settings window's drawn SettingsSwitch, as on every other Replies pane.
struct ReplyCardShell<Content: View>: View {
    let ax: String
    let title: String
    let subtitle: String
    let count: String
    let on: Bool
    var tinted = false
    let toggle: (Bool) -> Void
    @ViewBuilder var content: Content
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var shape: RoundedRectangle { RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous) }

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s6) {
            head
            VStack(spacing: 0) { content }
                .frame(maxWidth: .infinity, alignment: .leading)
                .background { shape.fill(tinted ? AnyShapeStyle(wash) : AnyShapeStyle(WhooshPalette.fill)) }
                .overlay { shape.strokeBorder(tinted ? WhooshPalette.accent.color(scheme).opacity(0.4) : WhooshPalette.separator.color(scheme), lineWidth: WhooshMetrics.separator) }
                .clipShape(shape)
                .opacity(on ? 1 : 0.4)
                .whooshAx("\(ax).card")
        }
        .animation(WhooshMotion.animation(WhooshMotion.hover, reduceMotion: reduceMotion), value: on)
    }

    private var wash: LinearGradient {
        LinearGradient(colors: [FlowTone.indigo.color(scheme).opacity(0.12), WhooshPalette.accent.color(scheme).opacity(0.08), FlowTone.teal.color(scheme).opacity(0.12)],
                       startPoint: .topLeading, endPoint: .bottomTrailing)
    }

    private var head: some View {
        ViewThatFits(in: .horizontal) {
            HStack(spacing: WhooshSpacing.s8) { name; Text(subtitle).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1); Spacer(minLength: WhooshSpacing.s8); figures }
            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: WhooshSpacing.s8) { name; Spacer(minLength: WhooshSpacing.s8); figures }
                Text(subtitle).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
            }
        }
        .padding(.horizontal, WhooshSpacing.s2).frame(minHeight: WhooshSpacing.s24)
        .accessibilityElement(children: .contain).accessibilityIdentifier(ax)
    }

    private var name: some View {
        HStack(spacing: WhooshSpacing.s6) {
            if tinted { Image(systemName: "sparkles").font(WhooshTypography.subhead.font).foregroundStyle(WhooshPalette.accent) }
            Text(title).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(tinted ? WhooshPalette.accent : WhooshPalette.label).lineLimit(1)
        }
    }

    private var figures: some View {
        HStack(spacing: WhooshSpacing.s10) {
            Text(count).whooshText(WhooshTypography.subhead).foregroundStyle(on ? WhooshPalette.secondaryLabel : WhooshPalette.tertiaryLabel).lineLimit(1)
                .contentTransition(.numericText())
                .accessibilityIdentifier("\(ax).count")
            // hid-ai round 17 (main107, item d58d8ad5): the system switch drew its ON state grey whenever the Settings window was not the key window
            // (every picture of the hidden copy, and the owner's window behind another one), where board b and the panes' own switch draw it in the
            // accent. The drawn switch of the other panes (SettingsSwitch, AutoDraftPane.swift:13) keeps the accent; same door, same ids.
            let isOn = on
            SettingsSwitch(on: isOn)
                .contentShape(Rectangle())
                .onTapGesture { toggle(!isOn) }
                .accessibilityElement().accessibilityLabel(title)
                .accessibilityAddTraits(.isButton).accessibilityValue(isOn ? RepliesWords.on : RepliesWords.off)
                .help(Words.text("Turn this card on or off")).accessibilityIdentifier("\(ax).switch")
                .whooshAx("\(ax).switch")
        }
    }
}

/// Copy of ReadTick (Sources/Features/LabelsContactsMac/LabelFlowAi.swift). Difference: its socket id is the one passed in (F3: the same `axBase`).
struct ReplyTick: View {
    let title: String
    let on: Bool
    let ax: String
    let action: () -> Void
    @State private var hover = false  // local-ui: hover tint only

    var body: some View {
        Button(action: action) {
            HStack(spacing: WhooshSpacing.s6) {
                RoundedRectangle(cornerRadius: WhooshRadius.keyCap - 1, style: .continuous).fill(on ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(.clear))
                    .overlay { RoundedRectangle(cornerRadius: WhooshRadius.keyCap - 1, style: .continuous).strokeBorder(WhooshPalette.tertiaryLabel, lineWidth: WhooshSpacing.s2 * 0.75).opacity(on ? 0 : 1) }
                    .overlay { Image(systemName: "checkmark").font(.system(size: WhooshTypography.caption.size - 3, weight: .bold)).foregroundStyle(WhooshPalette.onAccent).opacity(on ? 1 : 0) }
                    .frame(width: WhooshSpacing.s14, height: WhooshSpacing.s14)
                Text(title).whooshText(WhooshTypography.subhead).foregroundStyle(on ? WhooshPalette.label : WhooshPalette.secondaryLabel).lineLimit(1)
            }
            .padding(.horizontal, WhooshSpacing.s8).frame(height: WhooshSpacing.s24)
            .background(RoundedRectangle(cornerRadius: WhooshRadius.item, style: .continuous).fill(hover ? WhooshPalette.hover : WhooshPalette.field))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain).onHover { hover = $0 }
        .animation(WhooshMotion.hover, value: on).animation(WhooshMotion.hover, value: hover)
        .accessibilityAddTraits(on ? .isSelected : []).accessibilityLabel(title)
        .accessibilityIdentifier(ax).whooshAx(ax)
    }
}
