import SwiftUI

/// The thread's count pill after the sender (docs/design/boards/message-card-r1, pick A "Count badge"): caption semibold,
/// 16 pt high, 16 pt wide at least, fully rounded, 5 pt each side. Filled accent with the page colour as text while the row is
/// unread; the quiet fill with secondary text once read; white at 24% with white text on the focused row.
/// FINDING: the pill's 16 / 5 pt sizes are not in WhooshSpacing; they are local constants here until Design takes them.
struct CountPill: View {
    let count: Int
    let unread: Bool
    let focused: Bool

    private static let height: CGFloat = 16
    private static let side: CGFloat = 5

    var body: some View {
        Text("\(count)")
            .font(WhooshTypography.caption.font(emphasized: true)).monospacedDigit()
            .foregroundStyle(text)
            .padding(.horizontal, Self.side)
            .frame(minWidth: Self.height, minHeight: Self.height, maxHeight: Self.height)
            .background(Capsule(style: .continuous).fill(fill))
            .fixedSize()
            .accessibilityLabel(Words.text("%lld messages", count))
    }

    /// Which of the pill's three fills a row gets; the `rows` probe serves it as `count_pill_fill`, so what is checked is what is drawn.
    enum Fill: String { case onAccent = "onaccent", accent, quiet }
    static func fillKind(unread: Bool, focused: Bool) -> Fill { focused ? .onAccent : (unread ? .accent : .quiet) }

    private var fill: AnyShapeStyle {
        switch Self.fillKind(unread: unread, focused: focused) {
        case .onAccent: return AnyShapeStyle(WhooshPalette.onAccent.opacity(0.24))
        case .accent: return AnyShapeStyle(WhooshPalette.accent)
        case .quiet: return AnyShapeStyle(WhooshPalette.fill)
        }
    }

    private var text: AnyShapeStyle {
        if focused { return AnyShapeStyle(WhooshPalette.onAccent) }
        return unread ? AnyShapeStyle(WhooshPalette.background) : AnyShapeStyle(WhooshPalette.secondaryLabel)
    }
}

/// The small reply arrow before a thread's subject: the newest message is the owner's own.
struct RepliedMark: View {
    let tint: Color
    var body: some View {
        Image(systemName: "arrowshape.turn.up.left").font(WhooshTypography.caption.font(emphasized: false))
            .foregroundStyle(tint).fixedSize()
            .accessibilityLabel(Words.text("You replied last"))
    }
}
