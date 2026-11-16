import SwiftUI

/// The ring row: one chip per link or date of the focused message, the ring on the one Tab reached.
struct ReaderLinkRow: View {
    let items: [ReaderLinkItem]
    let cursor: Int?
    let open: (Int) -> Void

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: WhooshSpacing.s6) {
                ForEach(Array(items.enumerated()), id: \.offset) { i, item in
                    Button { open(i) } label: {
                        HStack(spacing: WhooshSpacing.s4) {
                            Image(systemName: item.kind == .link ? "link" : "calendar")
                            Text(item.kind == .link ? (item.url?.host ?? item.text) : item.text).lineLimit(1)
                        }
                        .whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.label)
                        .padding(.horizontal, WhooshSpacing.s8).padding(.vertical, WhooshSpacing.s4)
                        .background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous))
                        .overlay {
                            if cursor == i {
                                RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).strokeBorder(WhooshPalette.accent, lineWidth: WhooshSpacing.s2)
                            }
                        }
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .help(ReaderWords.linkTooltip)
                    .accessibilityIdentifier("mac-main.reader.link.\(i)")
                }
            }
            .padding(WhooshSpacing.s2)
        }
        .padding(.top, WhooshSpacing.s6)
        .accessibilityIdentifier("mac-main.reader.links")
    }
}
