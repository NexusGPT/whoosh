import SwiftUI

/// `.ft` an attachment card: a document icon, the file name and its kind (PDF, DOCX) read from the name.
struct AttachmentTile: View {
    let words: String
    var index = 0
    private var kind: String { (words as NSString).pathExtension.uppercased() }
    var body: some View {
        proxied
            .padding(.top, WhooshSpacing.s8)
            .accessibilityIdentifier("mac-main.reader.attachment.\(index)")
    }

    // attachments hook: a tile that names a file is the AttachmentsMac file proxy (Quick Look, open, drag, menu); off the Mac, the bare tile.
    @ViewBuilder private var proxied: some View {
        #if os(macOS)
        AttachmentProxy(readerWords: words, index: index) { tile }
        #else
        tile
        #endif
    }

    private var tile: some View {
        HStack(spacing: WhooshSpacing.s10) {
            Image(systemName: kind == "PDF" ? "doc.richtext" : "doc")
                .font(.system(size: WhooshTypography.title3.size)).foregroundStyle(WhooshPalette.accent)
            VStack(alignment: .leading, spacing: 0) {
                Text(words).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label)
                    .lineLimit(1).truncationMode(.middle)
                if !kind.isEmpty { Text(kind).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel) }
            }
        }
        .padding(.vertical, WhooshSpacing.s8).padding(.horizontal, WhooshSpacing.s12)
        .background(WhooshPalette.fill, in: RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator))
    }
}
