#if canImport(FluidAudio) && canImport(AVFoundation)
import AVFoundation
import FluidAudio

/// The iPhone engine (and an alternative on the Mac): FluidInference/FluidAudio (Apache-2.0), Parakeet v3 on Core ML.
/// Inert until Package.swift lists the dependency (a finding for the root; the exact lines are in docs/lanes/dictation.md).
/// The Core ML model downloads on first use, into FluidAudio's own cache; no audio leaves the device.
/// UNVERIFIED against a build: written from FluidAudio's README (AsrModels.downloadAndLoad, AsrManager.loadModels, transcribe).
public actor FluidAudioEngine: Transcriber {
    public nonisolated let name = "fluidaudio"
    private var manager: AsrManager?
    public init() {}

    private func ready() async throws -> AsrManager {
        if let manager { return manager }
        let models = try await AsrModels.downloadAndLoad(version: .v3)
        let m = AsrManager(config: .default)
        try await m.loadModels(models)
        manager = m
        return m
    }

    public func transcribe(audio: URL) async throws -> String {
        let file = try AVAudioFile(forReading: audio)
        guard let fmt = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 16_000, channels: 1, interleaved: false),
              let buf = AVAudioPCMBuffer(pcmFormat: file.processingFormat, frameCapacity: AVAudioFrameCount(file.length)) else {
            throw DictationError.engineFailed("the recording could not be read")
        }
        try file.read(into: buf)
        guard file.processingFormat.sampleRate == fmt.sampleRate, let ch = buf.floatChannelData?[0] else {
            throw DictationError.engineFailed("the recording is not 16 kHz mono")   // MicRecorder writes exactly that
        }
        let samples = Array(UnsafeBufferPointer(start: ch, count: Int(buf.frameLength)))
        var state = TdtDecoderState.make()   // FluidAudio 0.18: a fresh decoder state per utterance (lead fix after the package resolution moved)
        return try await ready().transcribe(samples, decoderState: &state).text.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
#endif
