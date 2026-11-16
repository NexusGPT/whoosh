#if os(macOS)
import SwiftUI

/// The gallery's header: how many files and how much, Quick Look over the whole set (Space), Save all.
struct AttachmentGalleryHeader: View {
    let set: AttachmentSet
    @Bindable private var store = AttachmentStore.shared

    /// ONE line at every width (owner 23:04, "should be in one line"): the widest rung that fits, never a broken word.
    /// (1) count · Quick Look · [Space] · Save all; (2) count · Quick Look · Save all, the key in Quick Look's tooltip;
    /// (3) count · an eye · a tray, words and key in the tooltips. Each label keeps its natural width on one line.
    var body: some View {
        let words = AttachmentGalleryHeader.words(set)
        if AttachmentRig.mutant("wrap-label") {
            rung(1, words)                                   // main's header: one form, its labels free to wrap
        } else {
            ViewThatFits(in: .horizontal) { rung(1, words); rung(2, words); rung(3, words) }
        }
    }

    private func rung(_ n: Int, _ words: String) -> some View {
        let r = AttachmentSocket.reader
        return HStack(spacing: WhooshSpacing.s10) {
            Image(systemName: "paperclip").foregroundStyle(WhooshPalette.secondaryLabel)
            Text(words).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.secondaryLabel)
                .modifier(AttachmentHeaderFit.Natural()).whooshAx(r + ".gallery.count")
            Spacer(minLength: WhooshSpacing.s8)
            Button { _ = AttachmentActs.quickLookAll(set) } label: {
                if n == 3 {
                    Image(systemName: "eye")
                } else {
                    HStack(spacing: WhooshSpacing.s4) {
                        Text(AttachmentWords.quickLook).modifier(AttachmentHeaderFit.Natural()).whooshAx(r + ".gallery.quick-look-words")
                        if n == 1 { KeyCap(AttachmentWords.spaceKey).modifier(AttachmentHeaderFit.Natural()).whooshAx(r + ".gallery.space-cap") }
                    }
                }
            }
            .buttonStyle(.plain).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.accent)
            .help(n == 3 ? "\(AttachmentWords.quickLook) · \(AttachmentWords.quickLookAllTip)" : AttachmentWords.quickLookAllTip)
            .whooshAx(r + ".gallery.quick-look")
            Button { _ = AttachmentActs.saveAll(set) } label: {
                if n == 3 { Image(systemName: "square.and.arrow.down") } else {
                    Text(AttachmentWords.saveAll).modifier(AttachmentHeaderFit.Natural()).whooshAx(r + ".gallery.save-all-words")
                }
            }
            .buttonStyle(.plain).whooshText(WhooshTypography.caption, emphasized: true).foregroundStyle(WhooshPalette.accent)
            .help(n == 3 ? "\(AttachmentWords.saveAll) · \(AttachmentWords.saveAllTip)" : AttachmentWords.saveAllTip)
            .whooshAx(r + ".gallery.save-all")
        }
        .onAppear { AttachmentHeaderFit.rung = n }
    }

    /// "12 files · 4.2 MB" when every file is here; "3 files · 2 on this Mac" when some are; "1 file · not on this Mac yet" when none is
    /// (no size is printed for bytes that are not here).
    @MainActor
    static func words(_ set: AttachmentSet) -> String {
        let store = AttachmentStore.shared
        let sizes = set.all.compactMap { r in store.peek(r).url.flatMap { try? $0.resourceValues(forKeys: [.fileSizeKey]).fileSize } }
        if sizes.isEmpty && !AttachmentRig.mutant("size-when-absent") { return AttachmentWords.galleryNoneHere(set.all.count) }
        let total = AttachmentSize.words(Int64(sizes.reduce(0, +)))
        if sizes.count < set.all.count && !AttachmentRig.mutant("size-when-absent") { return AttachmentWords.galleryPartial(set.all.count, sizes.count) }
        return AttachmentWords.galleryCount(set.all.count, total)
    }
}

/// The last tile of a collapsed gallery: "+N" and "more"; a click opens the rest in place.
struct AttachmentMoreTile: View {
    let count: Int
    let open: () -> Void
    @Environment(\.colorScheme) private var scheme
    var body: some View {
        Button(action: open) {
            VStack(spacing: WhooshSpacing.s2) {
                Text("+\(count)").whooshText(WhooshTypography.title3, emphasized: true).foregroundStyle(WhooshPalette.label)
                Text(AttachmentWords.more).whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
            }
            .frame(width: AttachmentTileImages.side, height: AttachmentTileImages.side)
            .background(WhooshPalette.fill.color(scheme), in: RoundedRectangle(cornerRadius: WhooshRadius.field, style: .continuous))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain).hoverAck(radius: WhooshRadius.field)
        .help(AttachmentWords.showMore(count)).accessibilityLabel(AttachmentWords.showMore(count))
        .whooshAx(AttachmentSocket.reader + ".gallery.more")
    }
}

/// The documents' "+N more" chip.
struct AttachmentMoreChip: View {
    let count: Int
    let open: () -> Void
    var body: some View {
        Button(action: open) {
            Text(AttachmentWords.showMore(count)).whooshText(WhooshTypography.body, emphasized: true).foregroundStyle(WhooshPalette.accent)
                .padding(.horizontal, WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s10)
                .frame(maxWidth: .infinity, alignment: .leading).contentShape(Rectangle())
        }
        .buttonStyle(.plain).hoverAck(radius: WhooshRadius.field)
        .whooshAx(AttachmentSocket.reader + ".documents.more")
    }
}
#endif
