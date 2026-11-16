#if os(macOS)
@preconcurrency import AVFoundation
import AppKit
import Observation
import QuickLookThumbnailing

/// The gallery's pictures at tile size: an image's or a PDF's thumbnail (QuickLookThumbnailing), a video's FIRST
/// frame (AVAssetImageGenerator at time zero, no tolerance) and its duration. Made off the main thread, kept per
/// attachment and bounded like the chip thumbnails.
@Observable @MainActor
final class AttachmentTileImages {
    static let shared = AttachmentTileImages()
    static var side: CGFloat { CGFloat(WhooshConstants.ATTACHMENT_GALLERY_TILE_PT) }

    private(set) var images: [AttachmentRef: NSImage] = [:]
    /// Seconds of each video, once its asset answered.
    private(set) var durations: [AttachmentRef: Double] = [:]
    /// True for a video whose picture is its first frame (not a system thumbnail).
    private(set) var firstFrame: Set<AttachmentRef> = []
    @ObservationIgnored private var order: [AttachmentRef] = []
    @ObservationIgnored private var asked: Set<AttachmentRef> = []

    func request(_ ref: AttachmentRef) {
        guard ref.kind.hasThumbnail, !AttachmentRig.mutant("no-thumbnail"), images[ref] == nil, !asked.contains(ref),
              let url = AttachmentStore.shared.peek(ref).url else { return }
        asked.insert(ref)
        let scale = NSScreen.main?.backingScaleFactor ?? 2, px = Self.side * scale
        if ref.kind == .video && !AttachmentRig.mutant("no-first-frame") {
            Task.detached(priority: .userInitiated) {
                let asset = AVURLAsset(url: url)
                let gen = AVAssetImageGenerator(asset: asset)
                gen.appliesPreferredTrackTransform = true
                gen.requestedTimeToleranceBefore = .zero
                gen.requestedTimeToleranceAfter = .zero
                gen.maximumSize = CGSize(width: px, height: px)
                let frame = try? await gen.image(at: .zero).image
                let seconds = (try? await asset.load(.duration)).map(CMTimeGetSeconds)
                await MainActor.run { AttachmentTileImages.shared.keepVideo(ref, frame, seconds, scale) }
            }
            return
        }
        let req = QLThumbnailGenerator.Request(fileAt: url, size: CGSize(width: Self.side, height: Self.side), scale: scale, representationTypes: .thumbnail)
        QLThumbnailGenerator.shared.generateBestRepresentation(for: req) { rep, _ in
            guard let cg = rep?.cgImage else { return }
            Task { @MainActor in AttachmentTileImages.shared.keep(ref, cg, scale) }
        }
    }

    private func keepVideo(_ ref: AttachmentRef, _ frame: CGImage?, _ seconds: Double?, _ scale: CGFloat) {
        if let s = seconds, s.isFinite { durations[ref] = s }
        guard let frame else { return }
        firstFrame.insert(ref)
        keep(ref, frame, scale)
    }

    private func keep(_ ref: AttachmentRef, _ cg: CGImage, _ scale: CGFloat) {
        images[ref] = NSImage(cgImage: cg, size: NSSize(width: CGFloat(cg.width) / scale, height: CGFloat(cg.height) / scale))
        order.removeAll { $0 == ref }
        order.append(ref)
        while order.count > WhooshConstants.ATTACHMENT_THUMB_CACHE_MAX {
            let old = order.removeFirst()
            images[old] = nil; asked.remove(old); firstFrame.remove(old)
        }
    }

    /// The file is no longer here (the rig's "not on this Mac yet" door): its picture goes, and is asked again when the bytes return.
    func forget(_ ref: AttachmentRef) {
        images[ref] = nil; asked.remove(ref); firstFrame.remove(ref); order.removeAll { $0 == ref }
    }

    /// "0:03", "12:40": a video's length as the Finder and Photos write it.
    static func durationWords(_ seconds: Double) -> String {
        let s = Int(seconds.rounded())
        return String(format: "%d:%02d", s / 60, s % 60)
    }
}
#endif
