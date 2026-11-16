import Foundation

/// What was read out of one downloaded attachment, in the shape of the Rust crate's `AttachmentIndexEntry` (the mirror that runs
/// in the fixture core until the app mounts the Rust core; the real core runs core/crates/attachment-index with these readers).
public struct ScannedAttachment: Sendable {
    public var text: String?
    public var ocrText: String?
    public var labels: [String] = []
    public var transcript: String?
    /// embedded pieces: key suffix ("", "#f0", "#audio") and the file the model was given
    public var mediaParts: [(suffix: String, kind: MediaKind, url: URL)] = []
    public var notes: [String] = []
    /// frames and sound tracks live here until the caller has embedded them; the caller removes it
    public var workFolder: URL?

    /// Everything readable out of the file, for the word index.
    public var words: String { [text, ocrText, labels.joined(separator: " "), transcript].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: "\n") }
}

/// Reads one file with the four readers. Every piece fails on its own: a frame that will not decode becomes a note.
public struct AttachmentScanner: Sendable {
    public let text: AttachmentTextReading
    public let pictures: PictureReading
    public let speech: SpeechTranscribing
    public let video: VideoProbing
    /// frames of a video: one per `frameEveryS` seconds, at most `framesMax` (docs/CONSTANTS.md VIDEO_FRAME_EVERY_S, VIDEO_FRAMES_MAX)
    public let frameEveryS: Double
    public let framesMax: Int

    public init(text: AttachmentTextReading, pictures: PictureReading, speech: SpeechTranscribing, video: VideoProbing, frameEveryS: Double, framesMax: Int) {
        self.text = text; self.pictures = pictures; self.speech = speech; self.video = video; self.frameEveryS = frameEveryS; self.framesMax = framesMax
    }

    /// Positions of the sampled frames; the same rule as the Rust `frame_times`.
    public func frameTimes(duration: Double) -> [Double] {
        guard duration.isFinite, duration > 0 else { return [0] }
        let wanted = max(1, Int((duration / frameEveryS).rounded(.up)))
        let count = min(wanted, framesMax)
        let step = wanted > framesMax ? duration / Double(count) : frameEveryS
        return (0 ..< count).map { min(Double($0) * step, max(duration - 0.05, 0)) }
    }

    public func scan(_ url: URL, kind: ReadableKind, scratch: URL) -> ScannedAttachment {
        var out = ScannedAttachment()
        switch kind {
        case .pdf, .document:
            do { out.text = try text.readText(at: url, kind: kind) } catch { out.notes.append("reading its text: \(error)") }
        case .image:
            out.mediaParts.append(("", .image, url))
            readPixels(url, into: &out)
        case .audio:
            out.mediaParts.append(("", .audio, url))
            listen(url, fromVideo: false, into: &out)
        case .video:
            scanVideo(url, scratch: scratch, into: &out)
        }
        return out
    }

    private func readPixels(_ url: URL, into out: inout ScannedAttachment) {
        do { if let w = try pictures.ocr(at: url) { out.ocrText = [out.ocrText, w].compactMap { $0 }.joined(separator: "\n") } } catch { out.notes.append("reading the words in the picture: \(error)") }
        do { for l in try pictures.labels(at: url) where !out.labels.contains(l) { out.labels.append(l) } } catch { out.notes.append("naming what the picture shows: \(error)") }
    }

    private func listen(_ url: URL, fromVideo: Bool, into out: inout ScannedAttachment) {
        do {
            switch try speech.transcribe(at: url, fromVideo: fromVideo) {
            case .text(let t): out.transcript = [out.transcript, t].compactMap { $0 }.joined(separator: "\n")
            case .noSpeech: break
            case .refused(let words): out.notes.append(words)
            }
        } catch { out.notes.append("transcribing the speech: \(error)") }
    }

    private func scanVideo(_ url: URL, scratch: URL, into out: inout ScannedAttachment) {
        let dir = scratch.appendingPathComponent(UUID().uuidString, isDirectory: true)
        out.workFolder = dir
        do { try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true) } catch { out.notes.append("making room for the frames: \(error)"); return }
        let duration: Double
        do { duration = try video.durationSeconds(of: url) } catch { out.notes.append("opening the video: \(error)"); return }
        for (i, at) in frameTimes(duration: duration).enumerated() {
            let frame = dir.appendingPathComponent("frame-\(i).jpg")
            do { try video.extractFrame(of: url, atSeconds: at, to: frame) } catch { out.notes.append("taking frame \(i): \(error)"); continue }
            out.mediaParts.append(("#f\(i)", .videoFrame, frame))
            readPixels(frame, into: &out)
        }
        let sound = dir.appendingPathComponent("audio.m4a")
        do { if try video.extractAudio(of: url, to: sound) { out.mediaParts.append(("#audio", .audio, sound)); listen(sound, fromVideo: true, into: &out) } } catch { out.notes.append("taking the sound track: \(error)") }
    }
}
