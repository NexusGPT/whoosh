import Foundation
#if canImport(AVFoundation)
@preconcurrency import AVFoundation

/// The microphone, Mac and iPhone: AVAudioEngine tap, converted to 16 kHz mono, written as 16-bit WAV, with the float samples handed on for live words.
public final class MicRecorder: AudioRecorder, @unchecked Sendable {
    private let engine = AVAudioEngine()
    private var file: AVAudioFile?
    private var url: URL?
    public init() {}

    public func authorize() async -> Bool {
        #if os(macOS)
        switch AVCaptureDevice.authorizationStatus(for: .audio) {
        case .authorized: return true
        case .notDetermined: return await AVCaptureDevice.requestAccess(for: .audio)
        default: return false
        }
        #else
        return await AVAudioApplication.requestRecordPermission()
        #endif
    }

    public func start(into file: URL, onSamples: @escaping @Sendable ([Float]) -> Void) throws {
        #if os(iOS)
        try AVAudioSession.sharedInstance().setCategory(.record, mode: .measurement)
        try AVAudioSession.sharedInstance().setActive(true)
        #endif
        let input = engine.inputNode
        let inFmt = input.outputFormat(forBus: 0)
        guard let outFmt = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 16_000, channels: 1, interleaved: false),
              let conv = AVAudioConverter(from: inFmt, to: outFmt) else { throw DictationError.recorderFailed }
        let out = try AVAudioFile(forWriting: file, settings: [AVFormatIDKey: kAudioFormatLinearPCM, AVSampleRateKey: 16_000, AVNumberOfChannelsKey: 1,
                                                              AVLinearPCMBitDepthKey: 16, AVLinearPCMIsFloatKey: false], commonFormat: .pcmFormatFloat32, interleaved: false)
        self.file = out; self.url = file
        input.installTap(onBus: 0, bufferSize: 4096, format: inFmt) { buf, _ in
            let cap = AVAudioFrameCount(Double(buf.frameLength) * 16_000 / inFmt.sampleRate) + 16
            guard let o = AVAudioPCMBuffer(pcmFormat: outFmt, frameCapacity: cap) else { return }
            nonisolated(unsafe) var fed = false
            conv.convert(to: o, error: nil) { _, status in
                if fed { status.pointee = .noDataNow; return nil }
                fed = true; status.pointee = .haveData; return buf
            }
            guard o.frameLength > 0, let ch = o.floatChannelData?[0] else { return }
            try? out.write(from: o)
            onSamples(Array(UnsafeBufferPointer(start: ch, count: Int(o.frameLength))))
        }
        engine.prepare()
        try engine.start()
    }

    public func stop() -> URL? {
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
        file = nil   // closes the WAV header
        return url
    }
}
#endif

/// Recorded audio standing in for the microphone: the fixture path (`fixture speak-file <path>`). The file it is given is the recording,
/// so it is deleted when it becomes text exactly like a real one. Only the control socket's fixture verbs and tests use it.
public final class FixtureRecorder: AudioRecorder, @unchecked Sendable {
    private let lock = NSLock()
    private var queued: URL?
    private var denied = false
    private var onSamples: (@Sendable ([Float]) -> Void)?
    private var early: [Float] = []   // samples heard before the session had started (the key press is handled a turn after its verb)
    public init() {}
    public func setDenied(_ d: Bool) { lock.lock(); denied = d; lock.unlock() }
    public func authorize() async -> Bool { lock.withLock { !denied } }
    public func start(into file: URL, onSamples: @escaping @Sendable ([Float]) -> Void) throws {
        let replay: [Float] = lock.withLock {
            self.onSamples = onSamples
            if queued == nil { queued = file }
            defer { early = [] }
            return early
        }
        try? Self.silence.write(to: file)   // the file a real recording would be: it exists while the key is held
        if !replay.isEmpty { onSamples(replay) }
    }
    /// A WAV header and no samples: 16 kHz, mono, 16 bit.
    static let silence: Data = {
        func le(_ v: UInt32) -> [UInt8] { (0..<4).map { UInt8((v >> (8 * UInt32($0))) & 0xff) } }
        var d = Data("RIFF".utf8); d.append(contentsOf: le(36)); d.append(Data("WAVEfmt ".utf8)); d.append(contentsOf: le(16))
        d.append(contentsOf: [1, 0, 1, 0]); d.append(contentsOf: le(16_000)); d.append(contentsOf: le(32_000)); d.append(contentsOf: [2, 0, 16, 0])
        d.append(Data("data".utf8)); d.append(contentsOf: le(0))
        return d
    }()
    /// The audio the "microphone" hears while the key is held.
    public func speak(file: URL, samples: [Float] = []) {
        lock.lock(); queued = file; lock.unlock()
        hear(samples: samples)
    }
    /// Samples heard with no file of their own: the live words, for the session in progress (or the one about to start).
    public func hear(samples: [Float]) {
        guard !samples.isEmpty else { return }
        let cb: (@Sendable ([Float]) -> Void)? = lock.withLock { if onSamples == nil { early += samples }; return onSamples }
        cb?(samples)
    }
    public func stop() -> URL? { lock.lock(); defer { lock.unlock() }; let u = queued; queued = nil; onSamples = nil; early = []; return u }
}
