#if os(macOS)
import AppKit
import SwiftUI

/// The attachment chip, drawn in the reader and the composer: the file's real thumbnail (image, PDF, video) or its
/// document icon, the name, then its kind and size, or the download's progress, or what just happened ("Saved to
/// Downloads"). Hover and selection reveal its keys (Space, ⌘O). Input goes through the AppKit hit layer on top,
/// so a press selects on the frame it lands and a double-click opens (docs/MOTION.md: feedback first).
struct AttachmentChip: View {
    let ref: AttachmentRef
    var sizeWords: String?
    var onRemove: (() -> Void)?
    /// A lone chip in the gallery hugs its content up to the block's width (the name takes the free width first); chips in a grid fill their column.
    var hugs = false
    @Bindable private var store = AttachmentStore.shared
    @Bindable private var thumbs = AttachmentThumbnails.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var phase: AttachmentPhase { store.peek(ref) }
    private var selected: Bool { store.selected == ref }
    private var keysShown: Bool { store.keysShown(ref) }
    /// Design Attachment T1: a file that is not on this Mac is this same chip with a cloud in the thumbnail square.
    private var absent: Bool { store.isAbsent(ref) }
    /// While a file is fetched the tile shows the ring alone (design Attachment T1, `.pring` at the chip's right end): the key caps return when the file is here.
    private var fetching: Bool { absent && phase.isFetching }
    /// The room the key caps need at the chip's end, so a hugging chip never draws its name under them: Space, then ⌘O for a file that is here.
    private var capsReserve: CGFloat {
        guard hugs else { return 0 }
        // the same room in all three states (absent, fetching, here): Space and ⌘O, so the chip keeps one width while a file downloads; while fetching the ring sits in it
        let caps = MacHintPlace.size(AttachmentWords.spaceKey).width + MacHintPlace.size("⌘O").width + WhooshSpacing.s4 + 2 * WhooshSpacing.s6
        return caps
    }

    var body: some View {
        HStack(spacing: WhooshSpacing.s8) {
            HStack(spacing: WhooshSpacing.s10) {
                if absent { AttachmentAbsentGlyph(ref: ref, fetching: phase.isFetching) }
                else { AttachmentPicture(ref: ref, image: thumbs.images[ref], progress: phase.progress) }
                VStack(alignment: .leading, spacing: 0) {
                    Text(ref.name).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.label)
                        .lineLimit(1).truncationMode(.middle)
                        .whooshAx(AttachmentSocket.ax(ref) + ".name")   // the name's own box: cut only when the chip truly has no more room (par-attachments)
                    Text(AttachmentChipText.subtitle(ref, sizeWords: sizeWords)).whooshText(WhooshTypography.caption)
                        .foregroundStyle(AttachmentChipText.failed(ref) ? WhooshPalette.warning : WhooshPalette.secondaryLabel)
                        .lineLimit(1).contentTransition(.opacity)
                }
                .padding(.trailing, capsReserve)
                .frame(maxWidth: hugs ? CGFloat?.none : CGFloat.infinity, alignment: .leading).layoutPriority(1)
                // The glyph of an act that waits or just happened keeps its room in every state (hidden when there is none): the chip is as wide before, during
                // and after a download (main137: 279, then 299.5 while the hourglass showed, then 279).
                let b = AttachmentChipText.badge(ref)
                Image(systemName: b?.symbol ?? "hourglass").foregroundStyle(WhooshPalette.secondaryLabel)
                    .frame(width: WhooshSpacing.s16)
                    .opacity(b == nil ? 0 : 1).help(b?.words ?? "")
                    .accessibilityLabel(b?.words ?? "").accessibilityHidden(b == nil)
                    .animation(reduceMotion ? nil : .easeOut(duration: Double(WhooshConstants.HOVER_REVEAL_MS) / 1000), value: b?.symbol)
            }
            // the key caps float over the chip's end while it is hovered or selected: hidden, they take no room from the name
            .overlay(alignment: .trailing) {
                // not drawn at all while the ring is: two controls never share the chip's end
                if fetching {
                    // the ring takes the caps' place in the room they reserve: the chip is as wide while it downloads as before and after (main112: 299.5 against 279)
                    AttachmentFetchRing(fraction: phase.fraction).padding(.trailing, WhooshSpacing.s6)
                } else {
                    HStack(spacing: WhooshSpacing.s4) { KeyCap(AttachmentWords.spaceKey); if !absent { KeyCap("⌘O") } }
                        .padding(.horizontal, WhooshSpacing.s6).background(chipFill, in: Capsule())
                        .opacity(keysShown ? 1 : 0).allowsHitTesting(false)
                        .animation(reduceMotion ? nil : .easeOut(duration: Double(WhooshConstants.HOVER_REVEAL_MS) / 1000), value: keysShown)
                        .whooshAx(AttachmentSocket.ax(ref) + ".keys")
                        .accessibilityHidden(true)
                }
            }
            .padding(.leading, WhooshSpacing.s6).padding(.trailing, WhooshSpacing.s10).padding(.vertical, WhooshSpacing.s6)
            .overlay { AttachmentHitLayer(ref: ref) }
            if let onRemove {
                Button(action: onRemove) { Image(systemName: "xmark").foregroundStyle(WhooshPalette.secondaryLabel) }
                    .buttonStyle(.plain).help(AttachmentWords.remove).accessibilityLabel("\(AttachmentWords.remove) \(ref.name)")
                    .whooshAx(AttachmentSocket.ax(ref) + ".remove")   // doors-read: the pending file's own remove button, a click door beside the menu's Remove
                    .padding(.trailing, WhooshSpacing.s10)
            }
        }
        .background(chipFill, in: shape)
        .overlay { shape.strokeBorder(WhooshGlass.rim, lineWidth: WhooshGlass.rimWidth) }
        .overlay { shape.stroke(WhooshGlass.hairline, lineWidth: WhooshGlass.rimWidth) }
        .overlay { if selected || ReaderPassage.rings(messageId: ref.messageId, name: ref.name) { shape.strokeBorder(WhooshPalette.focusHalo, lineWidth: WhooshSpacing.s2) } }
        .overlay { if store.pressed == ref { shape.fill(WhooshPalette.hover.color(scheme)).allowsHitTesting(false) } }
        .onAppear { if case .ready = store.phase(ref) { thumbs.request(ref) } }
        .accessibilityElement(children: .contain)
        .accessibilityAddTraits(selected ? .isSelected : [])
    }

    private var shape: RoundedRectangle { RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous) }
    private var chipFill: Color { store.hovered == ref ? WhooshPalette.hover.color(scheme) : WhooshPalette.fill.color(scheme) }
    @Environment(\.colorScheme) private var scheme
}

/// The picture at the chip's leading edge: the thumbnail (a play glyph over a video), else the type's icon; a
/// progress ring over it while the file downloads.
struct AttachmentPicture: View {
    let ref: AttachmentRef
    let image: NSImage?
    let progress: Double?
    private var side: CGFloat { AttachmentThumbnails.side }
    private var ring: CGFloat { side - WhooshSpacing.s16 }

    var body: some View {
        ZStack {
            if let image {
                Image(nsImage: image).resizable().aspectRatio(contentMode: .fill)
                    .frame(width: side, height: side)
                    .clipShape(RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous))
                    .overlay { RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator) }
                    .transition(.opacity)
                if ref.kind == .video {
                    Image(systemName: "play.fill").font(.system(size: WhooshTypography.caption.size, weight: .bold))
                        .foregroundStyle(.white).shadow(color: WhooshGlass.thumbShadow.color(.light), radius: WhooshGlass.thumbShadowBlur)
                }
            } else {
                Image(nsImage: AttachmentThumbnails.icon(ref)).resizable().aspectRatio(contentMode: .fit)
                    .frame(width: side, height: side)
            }
            if let p = progress, !AttachmentRig.mutant("no-wait") { AttachmentRing(progress: p, diameter: ring) }
        }
        .frame(width: side, height: side)
        .animation(.easeOut(duration: WhooshMotion.readerCrossFade), value: image != nil)
    }
}

/// The download's progress over a picture: a track and the accent arc from the top, on a disc of the background.
struct AttachmentRing: View {
    let progress: Double
    let diameter: CGFloat
    @Environment(\.colorScheme) private var scheme
    var body: some View {
        ZStack {
            Circle().fill(WhooshPalette.background.color(scheme)).frame(width: diameter + WhooshSpacing.s6, height: diameter + WhooshSpacing.s6)
            Circle().stroke(WhooshPalette.separator, lineWidth: WhooshSpacing.s2).frame(width: diameter, height: diameter)
            Circle().trim(from: 0, to: progress).stroke(WhooshPalette.accent, style: StrokeStyle(lineWidth: WhooshSpacing.s2, lineCap: .round))
                .rotationEffect(.degrees(-90)).frame(width: diameter, height: diameter)
                .animation(.easeOut(duration: WhooshMotion.hoverTint), value: progress)
        }
        .accessibilityLabel(AttachmentWords.downloading(Int((progress * 100).rounded())))
    }
}

/// The thumbnail square of a file that is not on this Mac: a cloud with an arrow (not fetched, failed) or the file's type glyph
/// (while it is fetched), on the fill, the size of the chip's picture (design Attachment T1, `.th`).
struct AttachmentAbsentGlyph: View {
    let ref: AttachmentRef
    let fetching: Bool
    private var side: CGFloat { AttachmentThumbnails.side }
    private var symbol: String {
        guard fetching else { return "icloud.and.arrow.down" }
        switch ref.kind { case .image: return "photo"; case .pdf: return "doc.richtext"; case .video: return "play.rectangle"; case .audio: return "waveform"; default: return "doc" }
    }
    var body: some View {
        RoundedRectangle(cornerRadius: WhooshRadius.control, style: .continuous).fill(WhooshPalette.fill)
            .frame(width: side, height: side)
            .overlay { Image(systemName: symbol).font(.system(size: WhooshTypography.title3.size, weight: .regular)).foregroundStyle(WhooshPalette.secondaryLabel) }
            .accessibilityHidden(true)
    }
}

/// The fetch's ring at the chip's right end (design `.pring`, 20 pt): it fills with the fraction, or spins when the source gives no
/// figure (the real core). Reduced Motion: a quarter arc that holds still.
struct AttachmentFetchRing: View {
    let fraction: Double?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var turn = false  // local-ui: the spin's phase
    private var diameter: CGFloat { WhooshSpacing.s20 }
    var body: some View {
        ZStack {
            Circle().stroke(WhooshPalette.separator, lineWidth: WhooshSpacing.s2)
            Circle().trim(from: 0, to: fraction ?? 0.25).stroke(WhooshPalette.accent, style: StrokeStyle(lineWidth: WhooshSpacing.s2, lineCap: .round))
                .rotationEffect(.degrees(-90 + (fraction == nil && turn ? 360 : 0)))
                .animation(fraction == nil && !reduceMotion ? .linear(duration: 1).repeatForever(autoreverses: false) : .easeOut(duration: WhooshMotion.hoverTint), value: turn)
        }
        .frame(width: diameter, height: diameter)
        .onAppear { turn = true }
        .whooshAx(AttachmentSocket.reader + ".fetch.ring")
        .accessibilityLabel(fraction.map { AttachmentWords.downloading(Int($0 * 100)) } ?? AttachmentWords.fetchingBare)
    }
}
#endif
