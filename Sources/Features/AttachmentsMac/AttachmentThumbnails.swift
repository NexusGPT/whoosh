#if os(macOS)
import AppKit
import Observation
import QuickLookThumbnailing
import UniformTypeIdentifiers

/// Real thumbnails for images, PDFs and videos (QuickLookThumbnailing, generated off the main thread), kept per
/// attachment of the message and bounded in number; other kinds draw the system's document icon for their type.
@Observable @MainActor
final class AttachmentThumbnails {
    static let shared = AttachmentThumbnails()
    /// The thumbnail's side in the chip, in points (docs/CONSTANTS.md).
    static var side: CGFloat { CGFloat(WhooshConstants.ATTACHMENT_THUMB_PT) }

    private(set) var images: [AttachmentRef: NSImage] = [:]
    @ObservationIgnored private var order: [AttachmentRef] = []
    @ObservationIgnored private var asked: Set<AttachmentRef> = []

    /// Asks for the thumbnail once the bytes are local; a no-op for kinds that draw an icon.
    func request(_ ref: AttachmentRef) {
        guard ref.kind.hasThumbnail, !AttachmentRig.mutant("no-thumbnail"), images[ref] == nil, !asked.contains(ref),
              let url = AttachmentStore.shared.peek(ref).url else { return }
        asked.insert(ref)
        let scale = NSScreen.main?.backingScaleFactor ?? 2
        let req = QLThumbnailGenerator.Request(fileAt: url, size: CGSize(width: Self.side, height: Self.side), scale: scale,
                                               representationTypes: .thumbnail)
        QLThumbnailGenerator.shared.generateBestRepresentation(for: req) { rep, _ in
            guard let cg = rep?.cgImage else { return }
            Task { @MainActor in
                AttachmentThumbnails.shared.keep(ref, NSImage(cgImage: cg, size: NSSize(width: CGFloat(cg.width) / scale, height: CGFloat(cg.height) / scale)))
            }
        }
    }

    func forget(_ ref: AttachmentRef) { images[ref] = nil; asked.remove(ref) }

    private func keep(_ ref: AttachmentRef, _ image: NSImage) {
        images[ref] = image
        order.removeAll { $0 == ref }
        order.append(ref)
        while order.count > WhooshConstants.ATTACHMENT_THUMB_CACHE_MAX {
            let old = order.removeFirst()
            images[old] = nil; asked.remove(old)
        }
    }

    /// The system's icon for the file's type (the Finder's document icon).
    static func icon(_ ref: AttachmentRef) -> NSImage {
        let t = UTType(filenameExtension: (ref.name as NSString).pathExtension.lowercased()) ?? .data
        return NSWorkspace.shared.icon(for: t)
    }
}
#endif
