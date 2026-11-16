import SwiftUI

/// The thin line where the second group starts (the Mac's `SearchDivider`, the drawing's `.sx-div`): the words between two hairlines, in the
/// phone's footnote style. Its words come from the engine's stage (`SearchPhoneModel.dividerTitle`): "Related", the ranking line, or "Other mails, by date".
struct SearchPhoneDivider: View {
    let title: String
    var body: some View {
        HStack(spacing: WhooshSpacing.s10) {
            line
            Text(title).phoneText(PhoneType.footnote, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1).fixedSize()
            line
        }
        .padding(.horizontal, PhoneMetrics.gutter).frame(minHeight: PhoneMetrics.roundButton - WhooshSpacing.s12)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isHeader)
        .accessibilityIdentifier("iphone-search.divider")
    }
    private var line: some View { Rectangle().fill(WhooshPalette.separator).frame(height: WhooshMetrics.separator / 2) }
}

/// No mail holds the words and none is related (the Mac's `SearchNoMatch`): "No match" in bold and the Mac's sentence under it, in the middle.
struct SearchPhoneNoMatch: View {
    var body: some View {
        VStack(spacing: WhooshSpacing.s4) {
            Text(Words.text("No match")).phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.label)
            Text(Words.text("No mail holds these words, and none is close to them.")).phoneText(PhoneType.subhead)
                .foregroundStyle(WhooshPalette.secondaryLabel).multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.horizontal, PhoneMetrics.gutter).padding(.vertical, WhooshSpacing.s16)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("iphone-search.empty")
    }
}
