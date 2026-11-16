import SwiftUI

/// One line: the tile, a noun-first title, where it leads. The row carries no key (the phone has none), no coloured edge,
/// and the account lines draw the same tile the inbox draws.
struct PalettePhoneRow: View {
    let item: PalettePhoneItem
    let index: Int
    var onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: WhooshSpacing.s12) {
                tile
                VStack(alignment: .leading, spacing: 0) {
                    Text(item.title).phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.label).lineLimit(2)
                    if !item.detail.isEmpty {
                        Text(item.detail).phoneText(PhoneType.subhead).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
                    }
                }
                Spacer(minLength: WhooshSpacing.s8)
            }
            .padding(.horizontal, PhoneMetrics.gutter)
            .frame(maxWidth: .infinity, minHeight: PalettePhoneMetrics.rowHeight, alignment: .leading)
            .contentShape(Rectangle())
        }
        .buttonStyle(PressStyle())
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("iphone-palette.results.\(index)")
    }

    @ViewBuilder private var tile: some View {
        if let id = item.accountId {
            PhoneTile(account: PhoneAccounts.of(id, in: AppModel.shared), small: true)
        } else {
            Image(systemName: item.symbol)
                .font(.system(size: PhoneMetrics.tileGlyphSmall, weight: .regular))
                .foregroundStyle(WhooshPalette.label)
                .frame(width: PalettePhoneMetrics.tile, height: PalettePhoneMetrics.tile)
                .background(RoundedRectangle(cornerRadius: PhoneMetrics.tileRadiusSmall, style: .continuous).fill(PhoneGround.trackFill))
        }
    }
}
