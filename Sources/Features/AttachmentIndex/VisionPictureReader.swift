import Foundation
import Vision

/// Vision on device: the words in a picture (accurate recognition, language detected) and the scene labels above the caller's
/// confidence, best first. The picture's embedding (EmbeddingGemma 2, the media path) is made elsewhere; this is the text beside it.
public struct VisionPictureReader: PictureReading {
    private let limits: ReadingLimits
    public init(limits: ReadingLimits) { self.limits = limits }

    public func ocr(at url: URL) throws -> String? {
        let request = VNRecognizeTextRequest()
        request.recognitionLevel = .accurate
        request.usesLanguageCorrection = true
        request.automaticallyDetectsLanguage = true
        try perform(request, at: url)
        let lines = (request.results ?? []).compactMap { $0.topCandidates(1).first?.string }
        let text = lines.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
        return text.isEmpty ? nil : text
    }

    public func labels(at url: URL) throws -> [String] {
        let request = VNClassifyImageRequest()
        try perform(request, at: url)
        return (request.results ?? [])
            .filter { $0.confidence >= limits.sceneLabelMinConfidence }
            .sorted { $0.confidence > $1.confidence }
            .prefix(limits.sceneLabelsMax)
            .map { $0.identifier.replacingOccurrences(of: "_", with: " ") }
    }

    private func perform(_ request: VNRequest, at url: URL) throws {
        do { try VNImageRequestHandler(url: url).perform([request]) } catch {
            throw AttachmentReadError.unreadable("the picture could not be read: \(error.localizedDescription)")
        }
    }
}
