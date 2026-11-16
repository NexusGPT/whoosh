import SwiftUI

// The Settings window's building blocks, from the S3 board (`.card` `.row` `.sec` `.pop` `.sw` `.btn` `.fld` `.k` `.sq`).
// Every pane draws with these and nothing else: grouped cards, hairline rows, label left, control right, one grey caption.

/// A white SF Symbol in a coloured rounded square: the pane icon (the lock amendment: inside Settings only).
struct SettingsPaneIcon: View {
    let pane: SettingsPane
    var size: CGFloat = SettingsMetrics.paneIcon
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let hue = pane.hue.value(scheme)
        let top = Color(.sRGB, red: hue.red * 0.82 + 0.18, green: hue.green * 0.82 + 0.18, blue: hue.blue * 0.82 + 0.18)
        let radius = size >= SettingsMetrics.headerIcon ? SettingsMetrics.headerIconRadius : SettingsMetrics.paneIconRadius
        RoundedRectangle(cornerRadius: radius, style: .continuous)
            .fill(LinearGradient(colors: [top, hue.color], startPoint: .top, endPoint: .bottom))
            .overlay(RoundedRectangle(cornerRadius: radius, style: .continuous).strokeBorder(Color.black.opacity(0.12), lineWidth: 0.5))
            .overlay(Image(systemName: pane.symbol).font(.system(size: size * 0.5, weight: .medium)).foregroundStyle(Color.white))
            .frame(width: size, height: size)
            .accessibilityHidden(true)
    }
}

/// `.k` a small key cap beside a control: the key of the control shown on it.
struct SettingsKey: View {
    let text: String
    var onSelection = false
    var body: some View {
        Text(text).font(.system(size: 10.5, weight: .medium)).monospacedDigit()
            .foregroundStyle(onSelection ? AnyShapeStyle(Color.white) : AnyShapeStyle(WhooshPalette.secondaryLabel))
            .padding(.horizontal, WhooshSpacing.s4).frame(minWidth: 16, minHeight: 16)
            .background(RoundedRectangle(cornerRadius: 4, style: .continuous)
                .fill(onSelection ? AnyShapeStyle(Color.white.opacity(0.22)) : AnyShapeStyle(WhooshPalette.keyCap)))
            .overlay(RoundedRectangle(cornerRadius: 4, style: .continuous)
                .strokeBorder(onSelection ? AnyShapeStyle(Color.clear) : AnyShapeStyle(WhooshPalette.separator), lineWidth: 0.5))
            .accessibilityHidden(true)
    }
}

/// The blue ring a found setting wears: 2 pt accent and a soft halo, until the next move.
struct SettingsRing: ViewModifier {
    let ax: String
    var radius: CGFloat = 8
    private let model = SettingsWindowModel.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func body(content: Content) -> some View {
        let on = model.ring == ax
        content
            .overlay {
                if on {
                    ZStack {
                        RoundedRectangle(cornerRadius: radius + 2, style: .continuous)
                            .stroke(WhooshPalette.accent.opacity(0.22), lineWidth: SettingsMetrics.ringHalo).padding(-SettingsMetrics.ringHalo / 2 - 1)
                        RoundedRectangle(cornerRadius: radius, style: .continuous)
                            .strokeBorder(WhooshPalette.accent, lineWidth: SettingsMetrics.ringWidth)
                    }
                    .allowsHitTesting(false)
                    .transition(.opacity)
                }
            }
            .animation(WhooshMotion.animation(.easeOut(duration: WhooshMotion.hoverTint), reduceMotion: reduceMotion), value: on)
            .accessibilityValue(on ? Words.text("Found") : "")
            .modifier(SettingsScrollTarget(ax: ax))
    }
}

/// What the pane's column scrolls to when a search finds this element (SettingsPaneScaffold: `proxy.scrollTo(ring)`). Every element that can wear the found
/// ring carries it: a whole group, a button, a field and a footnote had none, so the search ringed them where they were, below the fold.
/// The id sits on a clear view INSIDE the element, the element's size: an id on the element's own root is its identity as a subview, and the card that lists the
/// rows (GroupedCard: `Group(subviews:)` then `ForEach` over positions) gives each row its position for identity, so a row's root id was never found by the
/// scroll (main182 and main183, the hidden copy at the minimum window: every found ROW stayed below the foot while found groups were scrolled to).
struct SettingsScrollTarget: ViewModifier {
    let ax: String
    func body(content: Content) -> some View { if ax.isEmpty { content } else { content.background { Color.clear.id(ax) } } }
}

extension View {
    func settingsRing(_ ax: String, radius: CGFloat = 8) -> some View { modifier(SettingsRing(ax: ax, radius: radius)) }
}

/// `.card`: the shared form grid's card (Components/FormGrid.swift).
typealias SettingsCard = GroupedCard

/// `.sec` + `.card`: a heading, an optional grey note at its right, then the card; the ring of a found section goes round the card.
struct SettingsGroup<Content: View>: View {
    var title: String? = nil
    var note: String? = nil
    var ax: String? = nil
    @ViewBuilder var content: Content
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            FormSectionHeader(title: title, note: note)
            GroupedCard { content }
                .settingsRing(ax ?? "", radius: WhooshRadius.field)
                .modifier(SettingsOptionalAx(ax: ax))
        }
        .padding(.top, WhooshSpacing.s16)
    }
}

/// `.row`: the form grid's row (Components/FormGrid.swift) with the window's found-row ring, scroll id and socket id.
struct SettingsRow<Control: View>: View {
    let ax: String
    let title: String
    var caption: String? = nil
    var captionStrong = false
    var dot: Color? = nil
    var leading: AnyView? = nil
    var onTap: (() -> Void)? = nil
    @ViewBuilder var control: Control

    var body: some View {
        FormGridRow(title: title, caption: caption, captionStrong: captionStrong, dot: dot, leading: leading, onTap: onTap, probeId: ax) { control }
            .settingsRing(ax)
            .id(ax)
            .whooshAx(ax)
    }
}

/// `.sw` a 32 x 19 switch that flips on the input frame; the knob follows on the spring. A refused save nudges it back.
struct SettingsSwitch: View {
    let on: Bool
    var refusals = 0
    @State private var nudge: CGFloat = 0
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let s = SettingsMetrics.switchSize
        ZStack(alignment: on ? .trailing : .leading) {
            Capsule().fill(on ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(SettingsPalette.switchOff))
            Circle().fill(Color.white).padding(1.5)
                .shadow(color: .black.opacity(0.25), radius: 1, y: 1)
                .offset(x: nudge)
        }
        .frame(width: s.width, height: s.height)
        .animation(WhooshMotion.animation(WhooshMotion.tabThumb, reduceMotion: reduceMotion), value: on)
        .onChange(of: refusals) { _, _ in
            guard !reduceMotion else { return }
            withAnimation(.easeOut(duration: 0.08)) { nudge = on ? -WhooshSpacing.s6 : WhooshSpacing.s6 }
            withAnimation(WhooshMotion.tabThumb.delay(0.08)) { nudge = 0 }
        }
        .accessibilityHidden(true)
    }
}

/// A drawn tick: the chosen row of a list of choices. Drawn, so it never depends on a symbol font.
struct SettingsCheck: View {
    let on: Bool
    var body: some View {
        Image(systemName: "checkmark").font(.system(size: 12, weight: .semibold)).foregroundStyle(WhooshPalette.accent)
            .opacity(on ? 1 : 0).frame(width: 16)
            .animation(.easeOut(duration: WhooshMotion.hoverTint), value: on)
            .accessibilityHidden(true)
    }
}

/// `.pop` a pop-up button: the value, then the blue double chevron. The menu lists every choice with a tick on the current one.
struct SettingsPopup<Value: Hashable>: View {
    let ax: String
    let options: [(value: Value, title: String)]
    let selection: Value
    var width: CGFloat? = nil
    let onPick: (Value) -> Void
    @Environment(\.colorScheme) private var scheme

    private var current: String { options.first { $0.value == selection }?.title ?? "" }

    var body: some View {
        Menu {
            ForEach(Array(options.enumerated()), id: \.offset) { _, o in
                Button { onPick(o.value) } label: {
                    if o.value == selection { Label(o.title, systemImage: "checkmark") } else { Text(o.title) }
                }
            }
        } label: {
            HStack(spacing: WhooshSpacing.s6) {
                Text(current).font(.system(size: 13)).foregroundStyle(WhooshPalette.label).lineLimit(1)
                Image(systemName: "chevron.up.chevron.down").font(.system(size: 8, weight: .bold)).foregroundStyle(Color.white)
                    .frame(width: 16, height: 16)
                    .background(RoundedRectangle(cornerRadius: 4, style: .continuous).fill(WhooshPalette.accent))
            }
            .padding(.leading, 10).padding(.trailing, 4).frame(height: SettingsMetrics.popupHeight)
            .background(RoundedRectangle(cornerRadius: SettingsMetrics.popupRadius, style: .continuous).fill(WhooshPalette.thumb)
                .shadow(color: WhooshGlass.thumbShadow.color(scheme), radius: 1.5, y: WhooshGlass.thumbShadowY))
            .overlay(RoundedRectangle(cornerRadius: SettingsMetrics.popupRadius, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: 0.5))
            .contentShape(Rectangle())
        }
        .menuStyle(.button).buttonStyle(.plain).menuIndicator(.hidden).fixedSize()
        .whooshAx(ax)
    }
}

/// `.seg` a segmented control of a few choices; the thumb slides under the chosen one on the tab spring.
struct SettingsSegmented<Value: Hashable>: View {
    let axPrefix: String
    let options: [(value: Value, key: String, title: String)]
    let selection: Value
    let onPick: (Value) -> Void
    @Namespace private var thumb
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(spacing: 0) {
            ForEach(Array(options.enumerated()), id: \.offset) { _, o in
                let on = o.value == selection
                Button { onPick(o.value) } label: {
                    Text(o.title).font(.system(size: 12, weight: on ? .semibold : .regular)).lineLimit(1)
                        .foregroundStyle(WhooshPalette.label)
                        .padding(.horizontal, WhooshSpacing.s12).frame(height: 24)
                        // hid-ai: every option is the ONE source of its own id, on screen at all times; the thumb below follows the chosen one.
                        // The thumb used to be the source of "thumb" inside `if on`: the old and the new option held it in the same transaction,
                        // SwiftUI's MatchedGeometryScope trap (main88: EXC_BREAKPOINT on `click mac-settings-ai.nav.decisions`, the engine control).
                        .matchedGeometryEffect(id: o.key, in: thumb, isSource: true)
                        .contentShape(Capsule())
                }
                .buttonStyle(.plain)
                .whooshAx("\(axPrefix).\(o.key)")
            }
        }
        .background {   // the one thumb, never a source: it takes the chosen option's frame and slides on the tab spring
            if let k = options.first(where: { $0.value == selection })?.key {
                Capsule().fill(WhooshPalette.thumb).shadow(color: .black.opacity(0.14), radius: 1.5, y: 1)
                    .matchedGeometryEffect(id: k, in: thumb, isSource: false)
            }
        }
        .padding(2).background(Capsule().fill(WhooshPalette.fill))
        .animation(WhooshMotion.animation(WhooshMotion.tabThumb, reduceMotion: reduceMotion), value: selection)
    }
}

/// `.btn` a quiet capsule button; `primary` is the accent one, `destructive` the red one. A key cap shows the key where one is bound.
struct SettingsButton: View {
    let title: String
    var symbol: String? = nil
    var key: String? = nil
    var primary = false
    var destructive = false
    let ax: String
    let action: () -> Void
    @State private var hover = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                if let symbol { Image(systemName: symbol).font(.system(size: 11, weight: .medium)) }
                Text(title).font(.system(size: 12.5, weight: primary ? .semibold : .medium)).lineLimit(1)
                if let key { SettingsKey(text: key, onSelection: primary) }
            }
            .foregroundStyle(primary ? AnyShapeStyle(Color.white) : (destructive ? AnyShapeStyle(SettingsPalette.danger) : AnyShapeStyle(WhooshPalette.label)))
            .padding(.horizontal, 12).frame(height: SettingsMetrics.buttonHeight)
            .fixedSize()
            .background(Capsule().fill(primary ? AnyShapeStyle(WhooshPalette.filledControl) : AnyShapeStyle(WhooshPalette.fill)))
            .overlay(Capsule().strokeBorder(primary ? AnyShapeStyle(Color.clear) : AnyShapeStyle(WhooshPalette.separator), lineWidth: 0.5))
            .overlay(Capsule().fill(WhooshPalette.label.opacity(hover ? 0.05 : 0)))   // the theme's text colour: a dark tint on light, a light tint on dark (black did not show on dark)
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .onHover { hover = $0 }
        .animation(.easeOut(duration: WhooshMotion.hoverTint), value: hover)
        .help(key.map { "\(title) (\($0))" } ?? title)
        .settingsRing(ax, radius: SettingsMetrics.buttonHeight / 2)
        .whooshAx(ax)
    }
}

/// `.fld` a one-line field at the right of a row.
struct SettingsField: View {
    let ax: String
    let placeholder: String
    @Binding var text: String
    var secure = false
    var width: CGFloat = 200
    var mono = false
    var onSubmit: () -> Void = {}

    var body: some View {
        Group {
            if secure { SecureField(placeholder, text: $text) } else { TextField(placeholder, text: $text) }
        }
        .textFieldStyle(.plain)
        .font(mono ? .system(size: 12, design: .monospaced) : .system(size: 13))
        .foregroundStyle(WhooshPalette.label)
        .onSubmit(onSubmit)
        .padding(.horizontal, 8).frame(width: width, height: SettingsMetrics.fieldHeight)
        .background(RoundedRectangle(cornerRadius: 6, style: .continuous).fill(WhooshPalette.field))
        .overlay(RoundedRectangle(cornerRadius: 6, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: 0.5))
        .settingsRing(ax, radius: 6)
        .whooshAx(ax)
    }
}

/// The reason a save was refused, under the card it belongs to.
struct SettingsNotice: View {
    let text: String?
    let ax: String
    var body: some View {
        if let text {
            HStack(alignment: .firstTextBaseline, spacing: WhooshSpacing.s6) {
                Image(systemName: "exclamationmark.circle.fill").foregroundStyle(WhooshPalette.warning)
                Text(text).font(.system(size: 12)).foregroundStyle(WhooshPalette.label).fixedSize(horizontal: false, vertical: true)
            }
            .padding(.horizontal, WhooshSpacing.s4).padding(.top, WhooshSpacing.s8)
            .transition(.opacity.combined(with: .move(edge: .top)))
            .whooshAx(ax)
        }
    }
}

/// A grey line under a card, for what the card cannot say in a row.
struct SettingsFootnote: View {
    let text: String
    var ax: String? = nil
    var body: some View {
        Text(text).font(.system(size: 11.5)).foregroundStyle(WhooshPalette.secondaryLabel).lineSpacing(1)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, WhooshSpacing.s4).padding(.top, WhooshSpacing.s8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .modifier(SettingsOptionalAx(ax: ax))
    }
}

struct SettingsOptionalAx: ViewModifier {
    let ax: String?
    func body(content: Content) -> some View { if let ax { content.whooshAx(ax).modifier(SettingsScrollTarget(ax: ax)) } else { content } }
}

/// `.chip` a small status chip at the right of a row.
struct SettingsChip: View {
    let text: String
    var tone: Tone = .neutral
    enum Tone { case neutral, good, warn }
    var body: some View {
        Text(text).font(.system(size: 11, weight: .semibold))
            .foregroundStyle(tone == .good ? WhooshPalette.accent : (tone == .warn ? WhooshPalette.warning : WhooshPalette.secondaryLabel))
            .padding(.horizontal, 8).frame(height: 18).fixedSize()
            .background(Capsule().fill(tone == .good ? AnyShapeStyle(WhooshPalette.mine) : AnyShapeStyle(WhooshPalette.fill)))
            .contentTransition(.opacity)
    }
}

/// A card for a pane whose settings are still to come: it says what is coming, and carries no control.
struct SettingsComing: View {
    let pane: SettingsPane
    let lines: [String]
    var body: some View {
        SettingsGroup(title: Words.text("Coming")) {
            ForEach(Array(lines.enumerated()), id: \.offset) { i, line in
                SettingsRow(ax: "mac-settings-window.\(pane.id).coming.\(i)", title: line) { EmptyView() }
            }
        }
    }
}

/// A colour from a "#RRGGBB" string; the neutral grey when the string is empty or not a colour.
func settingsColor(hex: String?) -> Color {
    guard let hex, let c = EmblemRGB(hex: hex) else { return Color(.sRGB, red: 0.56, green: 0.56, blue: 0.58) }
    return Color(.sRGB, red: c.r, green: c.g, blue: c.b)
}
