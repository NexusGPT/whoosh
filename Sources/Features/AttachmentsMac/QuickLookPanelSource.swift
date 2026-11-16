#if os(macOS)
import AppKit
import Quartz

/// The panel's data source and delegate: the message's attachments, the chip as the zoom source.
extension QuickLookController: @preconcurrency QLPreviewPanelDataSource, @preconcurrency QLPreviewPanelDelegate {
    func numberOfPreviewItems(in panel: QLPreviewPanel!) -> Int { items.count }

    func previewPanel(_ panel: QLPreviewPanel!, previewItemAt index: Int) -> (any QLPreviewItem)! {
        guard items.indices.contains(index) else { return nil }
        return AttachmentPreviewItem(ref: items[index], phase: AttachmentStore.shared.peek(items[index]))
    }

    /// The zoom starts from the chip on screen.
    func previewPanel(_ panel: QLPreviewPanel!, sourceFrameOnScreenFor item: (any QLPreviewItem)!) -> NSRect {
        // a hidden rig copy draws no zoom: the system's transition window is not one of this app's concealed windows
        guard !HiddenRig.isOn, let ref = (item as? AttachmentPreviewItem)?.ref else { return .zero }
        return AttachmentChipFrames.screenFrame(ref) ?? .zero
    }

    func previewPanel(_ panel: QLPreviewPanel!, transitionImageFor item: (any QLPreviewItem)!, contentRect: UnsafeMutablePointer<NSRect>!) -> Any! {
        guard !HiddenRig.isOn, let ref = (item as? AttachmentPreviewItem)?.ref else { return nil }
        return AttachmentThumbnails.shared.images[ref] ?? AttachmentThumbnails.icon(ref)
    }

    /// The panel closed itself (its close button, or the system): the keys go back to the reader.
    func windowWillClose(_ notification: Notification) {
        if ownCloses > 0 { ownCloses -= 1; return }
        shown = false
    }

    /// Keys reach the app's key path first (ChordKit → AppModel.press), so the panel's own handling is not needed.
    func previewPanel(_ panel: QLPreviewPanel!, handle event: NSEvent!) -> Bool { false }
}
#endif
