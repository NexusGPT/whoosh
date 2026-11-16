import SwiftUI

/// The key cap at the row strip's scale: the strip's icons are 22 pt apart, so its caps are 16 pt tall (KeyCap's metrics scaled down).
struct MacCompactCap: View {
    let label: String
    var body: some View {
        Text(KeyCap.glyph(for: label))
            .font(.system(size: MacHintPlace.compactFont, weight: .semibold))
            .foregroundStyle(WhooshPalette.label)
            .padding(.horizontal, 2)
            .frame(minWidth: MacHintPlace.compactH).frame(height: MacHintPlace.compactH)   // exactly CAP_H tall: the placement's numbers are the drawn ones
            .background(WhooshPalette.keyCap, in: RoundedRectangle(cornerRadius: WhooshRadius.keyCap - 1, style: .continuous))
            .accessibilityHidden(true)
    }
}
