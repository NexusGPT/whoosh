import Foundation

/// Twins of the four readers: answers keyed by file name, so checks run without PDFKit, Vision, Speech or AVFoundation.
public struct FixtureTextReader: AttachmentTextReading {
    public var texts: [String: String]
    public init(texts: [String: String] = [:]) { self.texts = texts }
    public func readText(at url: URL, kind: ReadableKind) throws -> String? { texts[url.lastPathComponent] }
}

public struct FixturePictureReader: PictureReading {
    public var words: [String: String]
    public var scenes: [String: [String]]
    public init(words: [String: String] = [:], scenes: [String: [String]] = [:]) { self.words = words; self.scenes = scenes }
    public func ocr(at url: URL) throws -> String? { words[url.lastPathComponent] }
    public func labels(at url: URL) throws -> [String] { scenes[url.lastPathComponent] ?? [] }
}

public struct FixtureSpeech: SpeechTranscribing {
    public var said: [String: String]
    public var refusal: String?
    public init(said: [String: String] = [:], refusal: String? = nil) { self.said = said; self.refusal = refusal }
    public func transcribe(at url: URL, fromVideo: Bool) throws -> SpeechOutcome {
        if let refusal { return .refused(reasonWords: refusal) }
        let key = fromVideo ? url.deletingLastPathComponent().lastPathComponent : url.lastPathComponent
        return said[key].map(SpeechOutcome.text) ?? .noSpeech
    }
}

public struct FixtureVideoProbe: VideoProbing {
    public var duration: Double
    public var hasSound: Bool
    public init(duration: Double, hasSound: Bool = true) { self.duration = duration; self.hasSound = hasSound }
    public func durationSeconds(of url: URL) throws -> Double { duration }
    public func extractFrame(of url: URL, atSeconds: Double, to out: URL) throws { try Data("frame at \(atSeconds)".utf8).write(to: out) }
    public func extractAudio(of url: URL, to out: URL) throws -> Bool {
        if hasSound { try Data("sound".utf8).write(to: out) }
        return hasSound
    }
}
