import SwiftUI

/// The small grey tag of a row (the LC-S1 look: a hairline capsule, caption type, no colour). It sits in the row's second line, a fixed
/// 16 pt high, so it never moves a row; the card word keeps its width and the condition after it is cut with an ellipsis.
struct WhyTagView: View {
    let tag: WhyTag
    let messageId: String
    let onSelection: Bool
    @Environment(\.colorScheme) private var scheme

    private var ink: Color { onSelection ? WhooshPalette.onAccent.color(scheme).opacity(0.85) : WhooshPalette.secondaryLabel.color(scheme) }

    var body: some View {
        HStack(spacing: 0) {
            Text(tag.card).fixedSize().whooshAx("mac-main.list.row.\(messageId).whytag.card")
            if let d = tag.detail { Text(" \u{00B7} " + d).lineLimit(1).truncationMode(.tail) }
        }
        .font(WhooshTypography.caption.font(emphasized: true))
        .foregroundStyle(ink)
        .padding(.horizontal, WhooshSpacing.s6)
        .frame(height: WhooshMetrics.listRowLine - WhooshSpacing.s2)
        .overlay { Capsule().strokeBorder(onSelection ? ink.opacity(0.5) : WhooshPalette.separator.color(scheme), lineWidth: WhooshGlass.rimWidth * 2) }
        .whooshAx("mac-main.list.row.\(messageId).whytag")
    }
}
