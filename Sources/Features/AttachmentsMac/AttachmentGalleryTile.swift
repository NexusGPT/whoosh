#if os(macOS)
import AppKit
import SwiftUI

/// One square of the gallery: the image's (or PDF's) thumbnail filling the tile, a video's first frame with a play
/// glyph and its length, the download ring while the bytes come. The same AppKit input layer as the chip: a press
/// selects, a double-click opens, a drag carries the file out, a right-click shows the menu; Space opens Quick Look.
struct AttachmentGalleryTile: View {
    let ref: AttachmentRef
    @Bindable private var store = AttachmentStore.shared
    @Bindable private var images = AttachmentTileImages.shared
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private var side: CGFloat { AttachmentTileImages.side }
    private var selected: Bool { store.selected == ref }
    private var keysShown: Bool { (selected || store.hovered == ref) && !AttachmentRig.mutant("no-keys") }
    private var shape: RoundedRectangle { RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous) }

    var body: some View {
        ZStack {
            WhooshPalette.fill.color(scheme)
            if let image = images.images[ref] {
                Image(nsImage: image).resizable().aspectRatio(contentMode: .fill).frame(width: side, height: side).clipped()
                    .transition(.opacity)
            } else {
                Image(nsImage: AttachmentThumbnails.icon(ref)).resizable().aspectRatio(contentMode: .fit)
                    .frame(width: side / 2, height: side / 2)
            }
            if ref.kind == .video { videoMarks }
            if let p = store.peek(ref).progress, !AttachmentRig.mutant("no-wait") { AttachmentRing(progress: p, diameter: side / 4) }
            KeyCap(AttachmentWords.spaceKey)
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomLeading).padding(WhooshSpacing.s6)
                .opacity(keysShown ? 1 : 0)
                .animation(reduceMotion ? nil : .easeOut(duration: Double(WhooshConstants.HOVER_REVEAL_MS) / 1000), value: keysShown)
        }
        .frame(width: side, height: side)
        .clipShape(shape)
        .overlay { shape.strokeBorder(WhooshGlass.rim, lineWidth: WhooshGlass.rimWidth) }
        .overlay { shape.stroke(WhooshGlass.hairline, lineWidth: WhooshGlass.rimWidth) }
        .overlay { if store.hovered == ref || store.pressed == ref { shape.fill(WhooshPalette.hover.color(scheme)).allowsHitTesting(false) } }
        .overlay { if selected || ReaderPassage.rings(messageId: ref.messageId, name: ref.name) { shape.strokeBorder(WhooshPalette.focusHalo, lineWidth: WhooshSpacing.s2 + WhooshSpacing.s2 / 2) } }
        .overlay { AttachmentHitLayer(ref: ref) }
        .animation(.easeOut(duration: WhooshMotion.readerCrossFade), value: images.images[ref] != nil)
        .onAppear { if case .ready = store.phase(ref) { images.request(ref) } }
        .whooshAx(AttachmentSocket.ax(ref))
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(AttachmentWords.accessibility(ref.name, ref.kind.rawValue))
        .accessibilityAddTraits(selected ? [.isButton, .isSelected] : .isButton)
    }

    /// A play glyph on a glass disc in the middle, the length on a glass capsule at the bottom right.
    private var videoMarks: some View {
        ZStack {
            Image(systemName: "play.fill").font(.system(size: WhooshTypography.title3.size, weight: .semibold))
                .foregroundStyle(WhooshPalette.label)
                .frame(width: side / 3, height: side / 3).whooshSystemMaterial(.ultraThinMaterial, in: Circle())
            if let d = images.durations[ref] {
                Text(AttachmentTileImages.durationWords(d)).whooshText(WhooshTypography.caption, emphasized: true)
                    .monospacedDigit().foregroundStyle(WhooshPalette.label)
                    .padding(.horizontal, WhooshSpacing.s6).whooshSystemMaterial(.ultraThinMaterial, in: Capsule())
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .bottomTrailing).padding(WhooshSpacing.s6)
            }
        }
    }
}
#endif
