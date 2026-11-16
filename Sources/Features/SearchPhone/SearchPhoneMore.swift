import SwiftUI

/// The closed line under the related mail: "All other mail · N". A tap opens the rest under it, the same tap closes it. A 44 pt target.
struct SearchPhoneMoreLine: View {
    let model: SearchPhoneModel
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        Button { model.toggleRest() } label: {
            HStack(spacing: WhooshSpacing.s8) {
                Image(systemName: "chevron.right").font(.system(size: PhoneType.footnote.size, weight: .semibold))
                    .rotationEffect(.degrees(model.restOpen ? 90 : 0))
                    .animation(reduceMotion ? nil : WhooshMotion.newRow, value: model.restOpen)
                (Text(SearchResultsList.moreTitle).font(PhoneType.subhead.font(emphasized: true)) + Text(verbatim: " \u{B7} \(model.restCount)").font(PhoneType.subhead.font(emphasized: false)))
                    .lineLimit(1)
                Spacer(minLength: 0)
            }
            .foregroundStyle(WhooshPalette.secondaryLabel)
            .padding(.horizontal, PhoneMetrics.gutter).frame(minHeight: PhoneMetrics.roundButton).contentShape(Rectangle())
        }
        .buttonStyle(SearchRowPressStyle())
        .accessibilityLabel(model.restOpen ? Words.text("Hide all other mail") : Words.text("Show all other mail"))
        .accessibilityIdentifier("iphone-search.more")
    }
}
