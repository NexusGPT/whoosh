#if os(macOS)
import Foundation
import Quartz

/// One item of the panel: the local file when it is here, else its name and how far the download is.
final class AttachmentPreviewItem: NSObject, QLPreviewItem {
    let ref: AttachmentRef
    let phase: AttachmentPhase
    init(ref: AttachmentRef, phase: AttachmentPhase) { self.ref = ref; self.phase = phase }
    var previewItemURL: URL! { phase.url }
    var previewItemTitle: String! {
        if let p = phase.fraction { return "\(ref.name) · \(AttachmentWords.downloading(Int(p * 100)))" }
        return ref.name
    }
}
#endif
