import AVFoundation
import Foundation
import ImageIO
import UniformTypeIdentifiers

/// AVFoundation: the length of a video, a JPEG still at a time, the sound track as an m4a. The caller owns the output folder.
public struct AVVideoProbe: VideoProbing {
    public init() {}

    public func durationSeconds(of url: URL) throws -> Double {
        try wrapped("the video could not be opened") { try await AVURLAsset(url: url).load(.duration).seconds }
    }

    public func extractFrame(of url: URL, atSeconds: Double, to out: URL) throws {
        try wrapped("a frame could not be taken") {
            let generator = AVAssetImageGenerator(asset: AVURLAsset(url: url))
            generator.appliesPreferredTrackTransform = true
            let (image, _) = try await generator.image(at: CMTime(seconds: atSeconds, preferredTimescale: 600))
            guard let sink = CGImageDestinationCreateWithURL(out as CFURL, UTType.jpeg.identifier as CFString, 1, nil) else {
                throw AttachmentReadError.unreadable("the frame file could not be created")
            }
            CGImageDestinationAddImage(sink, image, nil)
            guard CGImageDestinationFinalize(sink) else { throw AttachmentReadError.unreadable("the frame file could not be written") }
        }
    }

    public func extractAudio(of url: URL, to out: URL) throws -> Bool {
        try wrapped("the sound track could not be taken") {
            let asset = AVURLAsset(url: url)
            guard try await !asset.loadTracks(withMediaType: .audio).isEmpty else { return false }
            guard let export = AVAssetExportSession(asset: asset, presetName: AVAssetExportPresetAppleM4A) else {
                throw AttachmentReadError.unreadable("the sound track cannot be exported on this Mac")
            }
            try await export.export(to: out, as: .m4a)
            return true
        }
    }

    private func wrapped<T: Sendable>(_ words: String, _ work: @escaping @Sendable () async throws -> T) throws -> T {
        do { return try blocking(work) } catch let e as AttachmentReadError { throw e } catch {
            throw AttachmentReadError.unreadable("\(words): \(error.localizedDescription)")
        }
    }
}
