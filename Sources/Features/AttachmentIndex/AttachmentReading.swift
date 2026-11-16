// The answering side of the attachment index on this Mac: what the OS reads out of a file. Each piece is a protocol with a
// fixture twin (FixtureAttachmentReaders.swift), so checks run without the model, the permissions or the frameworks.
// The Rust crate core/crates/attachment-index holds the same four ports (TextReader, ImageReader, Transcriber, MediaProbe).
import Foundation

public enum ReadableKind: String, Sendable { case pdf, document, image, audio, video }

public enum AttachmentReadError: Error, CustomStringConvertible, Sendable {
    case unreadable(String)
    public var description: String { if case .unreadable(let words) = self { return words } else { return "" } }
}

/// PDFKit for a PDF the core's reader declined; the system's document readers for word-processor files.
public protocol AttachmentTextReading: Sendable {
    func readText(at url: URL, kind: ReadableKind) throws -> String?
}

/// Vision: the words in a picture or a video frame, and what the picture shows.
public protocol PictureReading: Sendable {
    func ocr(at url: URL) throws -> String?
    func labels(at url: URL) throws -> [String]
}

public enum SpeechOutcome: Equatable, Sendable {
    case text(String)
    case noSpeech
    /// not allowed or not available: said to the owner in words
    case refused(reasonWords: String)
}

/// Speech on device only: nothing leaves this Mac.
public protocol SpeechTranscribing: Sendable {
    func transcribe(at url: URL, fromVideo: Bool) throws -> SpeechOutcome
}

/// AVFoundation: the length of a video, a still at a time, the sound of a video (false = no sound track).
public protocol VideoProbing: Sendable {
    func durationSeconds(of url: URL) throws -> Double
    func extractFrame(of url: URL, atSeconds: Double, to out: URL) throws
    func extractAudio(of url: URL, to out: URL) throws -> Bool
}

/// Numbers the readers need, handed in by the caller from docs/CONSTANTS.md (this folder holds none of its own).
public struct ReadingLimits: Sendable {
    public let sceneLabelMinConfidence: Float
    public let sceneLabelsMax: Int
    public let speechTimeoutSeconds: Double
    public init(sceneLabelMinConfidence: Float, sceneLabelsMax: Int, speechTimeoutSeconds: Double) {
        self.sceneLabelMinConfidence = sceneLabelMinConfidence
        self.sceneLabelsMax = sceneLabelsMax
        self.speechTimeoutSeconds = speechTimeoutSeconds
    }
}

/// Runs async work from a synchronous port (the Rust side calls these on its own threads), like GemmaEmbedder's bridge.
private final class BlockingBox<T: Sendable>: @unchecked Sendable { var value: Result<T, Error>? }

func blocking<T: Sendable>(_ work: @escaping @Sendable () async throws -> T) throws -> T {
    let box = BlockingBox<T>()
    let done = DispatchSemaphore(value: 0)
    Task.detached {
        do { box.value = .success(try await work()) } catch { box.value = .failure(error) }
        done.signal()
    }
    done.wait()
    return try box.value!.get()
}
