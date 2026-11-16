#if os(macOS)
import SwiftUI

/// The reader's attachments as a gallery (docs/PRD.md "Attachments: a gallery for images and videos"): a header with
/// the count, the total size, Quick Look over the whole set and Save all; images and videos as square tiles in rows
/// that fill the pane; documents as compact chips below. Collapsed, each group fills ATTACHMENT_GALLERY_ROWS row(s)
/// of the pane: as many tiles as fit at ATTACHMENT_GALLERY_TILE_PT, the last cell "+N more" when files remain (owner
/// on the 88 pt shot: one full row); the documents show as many chips as the tile row has cells.
struct AttachmentGallery: View {
    let set: AttachmentSet
    @Bindable private var store = AttachmentStore.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        VStack(alignment: .leading, spacing: WhooshSpacing.s10) {
            AttachmentGalleryHeader(set: set)
            let (tiles, chips) = AttachmentGallery.split(set)
            if !tiles.isEmpty {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: AttachmentTileImages.side, maximum: AttachmentTileImages.side), spacing: WhooshSpacing.s8)],
                          alignment: .leading, spacing: WhooshSpacing.s8) {
                    ForEach(AttachmentGallery.shown(tiles, open: store.expanded.contains(set.messageId)), id: \.self) { ref in
                        AttachmentGalleryTile(ref: ref).transition(appear)
                    }
                    if let more = AttachmentGallery.hidden(tiles, open: store.expanded.contains(set.messageId)) {
                        AttachmentMoreTile(count: more) { expand { store.expanded.insert(set.messageId) } }
                    }
                }
            }
            if chips.count == 1, let ref = chips.first {
                // a lone chip hugs its content up to the block's width: its name takes the free width first (main105: "quote-Q1…int-run.pdf" cut in a
                // half-width column). Drawn as a plain view, not as the one cell of a lazy grid: the likely cause of main107's quit (hid-search round 18,
                // read, not run): the reader scrolled to the open mail's foot for a related mail found by its attachment, and AppKit's layout pass raised
                // with this hugging cell in a LazyVGrid, the one view of that path changed since the state last passed. Same place and width as that cell.
                AttachmentChip(ref: ref, hugs: true).whooshAx(AttachmentSocket.ax(ref)).transition(appear)
                    .frame(maxWidth: WhooshMetrics.blockWidth, alignment: .leading)
            } else if !chips.isEmpty {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: WhooshMetrics.blockWidth / 2 - WhooshSpacing.s8), spacing: WhooshSpacing.s8)],
                          alignment: .leading, spacing: WhooshSpacing.s8) {
                    ForEach(AttachmentGallery.shown(chips, open: store.documentsExpanded.contains(set.messageId)), id: \.self) { ref in
                        AttachmentChip(ref: ref).whooshAx(AttachmentSocket.ax(ref)).transition(appear)
                    }
                    if let more = AttachmentGallery.hidden(chips, open: store.documentsExpanded.contains(set.messageId)) {
                        AttachmentMoreChip(count: more) { expand { store.documentsExpanded.insert(set.messageId) } }
                    }
                }
            }
        }
        .padding(.top, WhooshSpacing.s8)
        .whooshAx(AttachmentSocket.reader + ".gallery.frame")
        .onGeometryChange(for: CGFloat.self) { $0.size.width } action: { w in
            if abs(store.galleryWidth - w) >= 1 { store.galleryWidth = w }
        }
    }

    /// The rest opens in place: tiles fade and scale up from POPOVER_SCALE_FROM over POPOVER_MS (a cross-fade under
    /// Reduce Motion); the state changes on the click's frame.
    private func expand(_ change: () -> Void) {
        withAnimation(reduceMotion ? .easeOut(duration: WhooshMotion.readerCrossFade) : .easeOut(duration: Double(WhooshConstants.POPOVER_MS) / 1000), change)
    }
    private var appear: AnyTransition {
        reduceMotion ? .opacity : .opacity.combined(with: .scale(scale: CGFloat(WhooshConstants.POPOVER_SCALE_FROM)))
    }

    /// The picture tiles (images and videos that are here) and the chips (documents, and every file that is not on this Mac: design
    /// Attachment T1). A file that arrives leaves the chips for the tiles in the same place of the message.
    @MainActor
    static func split(_ set: AttachmentSet) -> (tiles: [AttachmentRef], chips: [AttachmentRef]) {
        let store = AttachmentStore.shared
        let here = set.media.filter { !store.isAbsent($0) }
        if AttachmentRig.mutant("absent-tile") { return (set.media, set.documents) }       // the A123 look: a big tile for a file that is not here
        return (here, set.media.filter { store.isAbsent($0) } + set.documents)
    }

    /// Cells in the collapsed gallery: the tiles one row of the pane holds at the tile side, times the rows.
    @MainActor
    static var cells: Int {
        perRow * WhooshConstants.ATTACHMENT_GALLERY_ROWS + (AttachmentRig.mutant("row-overflow") ? 1 : 0)
    }
    @MainActor
    static var perRow: Int {
        let side = AttachmentTileImages.side, gap = WhooshSpacing.s8
        let width = AttachmentStore.shared.galleryWidth > 0 ? AttachmentStore.shared.galleryWidth : WhooshMetrics.blockWidth
        return max(1, Int((width + gap) / (side + gap)))
    }

    /// What a group shows: everything when open or within the cells, else all but the last cell (which is "+N more").
    @MainActor
    static func shown(_ refs: [AttachmentRef], open: Bool) -> [AttachmentRef] {
        let cap = cells
        return open || refs.count <= cap || AttachmentRig.mutant("no-collapse") ? refs : Array(refs.prefix(cap - 1))
    }
    /// How many a "+N more" control stands for, or nil when every item shows.
    @MainActor
    static func hidden(_ refs: [AttachmentRef], open: Bool) -> Int? {
        let n = refs.count - shown(refs, open: open).count
        return n > 0 ? n : nil
    }
}
#endif
