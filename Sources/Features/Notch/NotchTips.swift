import SwiftUI

/// A control's name and key, after a short rest of the pointer, drawn in the panel itself. Adapted from the
/// owner's ghostty fork (macos/Sources/Features/OneTerminal/Notch/DesignSystem.swift `Tip`, `TipsKey`,
/// `TipsLayer`, by the owner, MIT): a non-key panel shows no system tooltip, so `.help` names nothing in the
/// notch. The tip is not drawn by its control: the control publishes its text and frame, and the root draws every
/// tip in one layer outside the shape's clip, so a tip is never cut by the silhouette.
struct NotchTipItem: Equatable, Identifiable {
    var id: String
    var text: String
    var frame: CGRect
}

struct NotchTipsKey: PreferenceKey {
    static let defaultValue: [NotchTipItem] = []
    static func reduce(value: inout [NotchTipItem], nextValue: () -> [NotchTipItem]) { value += nextValue() }
}

enum NotchTips {
    /// The coordinate space the root names, so a tip's frame and the layer agree.
    static let space = "notch.tips"
}

struct NotchTip: ViewModifier {
    let text: String
    @State private var hovering = false
    @State private var shown = false
    @State private var id = UUID().uuidString

    func body(content: Content) -> some View {
        content
            .onHover { h in
                hovering = h
                guard h else { shown = false; return }
                DispatchQueue.main.asyncAfter(deadline: .now() + Double(WhooshConstants.NOTCH_TIP_DELAY_MS) / 1000) {
                    if hovering { shown = true }
                }
            }
            .background(GeometryReader { g in
                Color.clear.preference(key: NotchTipsKey.self,
                                       value: shown && !text.isEmpty ? [NotchTipItem(id: id, text: text, frame: g.frame(in: .named(NotchTips.space)))] : [])
            })
    }
}

extension View {
    /// The notch's tooltip: a control's name, and its key where it has one ("Archive · E").
    func notchTip(_ text: String) -> some View { modifier(NotchTip(text: text)) }
}

/// Every tip, drawn once at the root, below its control, above the shape and everything in it.
struct NotchTipsLayer: View {
    let tips: [NotchTipItem]

    var body: some View {
        ZStack(alignment: .topLeading) {
            ForEach(tips) { t in
                Text(t.text)
                    .whooshText(WhooshTypography.caption, emphasized: true)
                    .foregroundStyle(WhooshPalette.label)
                    .padding(.horizontal, WhooshSpacing.s8)
                    .padding(.vertical, WhooshSpacing.s4)
                    .background(WhooshPalette.panel, in: RoundedRectangle(cornerRadius: WhooshRadius.item, style: .continuous))
                    .fixedSize()
                    .position(x: t.frame.midX, y: t.frame.maxY + WhooshSpacing.s14)
                    .transition(.opacity)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .allowsHitTesting(false)
        .animation(WhooshMotion.hover, value: tips)
    }
}
