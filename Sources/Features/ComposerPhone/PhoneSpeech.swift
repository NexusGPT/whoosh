import Foundation
import Observation
#if canImport(FluidAudio)
import FluidAudio
#endif

/// The phone's speech model and the microphone button's state (docs/JOURNEYS.md J7.4: the model is not on this phone, it downloads on first use,
/// with progress, then the words arrive). One object; the dictation session itself is the Dictation lane's `DictationController`:
///  - under the rig (`DictationRig.active`) the session is `Dictation.shared`, which hears no microphone (`fixture speak <words>`), and the
///    model download is a stand-in that counts up on a timer: nothing is fetched;
///  - on the phone the session is built here over `FluidAudioEngine` and `MicRecorder`, and the download is FluidAudio's own
///    (`AsrModels.downloadAndLoad`), whose progress the package does not hand back here, so the bar shows the download without a number.
@MainActor @Observable
public final class PhoneSpeech {
    public static let shared = PhoneSpeech()

    public enum Model: Equatable {
        case ready, missing
        /// 0...1, or nil when the engine reports no progress.
        case downloading(Double?)
        case failed(String)
    }

    /// What the button shows and the socket reads: idle, downloading, listening, cleaning, text (words just arrived).
    public enum Mic: String { case idle, downloading, listening, cleaning, text }

    public private(set) var model: Model
    public private(set) var textArrived = false
    /// The last words dictated into the draft; the row `iphone-composer.body` reports them.
    public private(set) var dictated = ""
    /// Bumped by the controller on every change, so the bar redraws its state and live words.
    public private(set) var revision = 0
    public private(set) var failure = ""

    // Private numbers (listed for the ledger in docs/lanes/composerphone.md): the rig's stand-in download takes RIG_STEPS steps of RIG_STEP_MS.
    private nonisolated static let rigSteps = 5
    private nonisolated static let rigStepMs = 250
    private static let readyKey = "composerphone.speechModelReady"

    @ObservationIgnored private var downloading: Task<Void, Never>?
    @ObservationIgnored private var tokens: [NSObjectProtocol] = []
    #if canImport(FluidAudio) && canImport(AVFoundation)
    private static let phoneEngine = FluidAudioEngine()
    @ObservationIgnored private lazy var phoneSession: DictationController = {
        let c = DictationController(recorder: MicRecorder(), engine: { Self.phoneEngine }, cleaner: LocalCleaner(language: { Words.current }))
        c.sink = { text in ComposerPhoneState.shared.insertDictated(text) }
        return c
    }()
    #endif

    private init() {
        model = DictationRig.active ? .missing : (UserDefaults.standard.bool(forKey: Self.readyKey) ? .ready : .missing)
        if DictationRig.active { listenForRigVerbs() }
    }

    var controller: DictationController {
        #if canImport(FluidAudio) && canImport(AVFoundation)
        if !DictationRig.active { return phoneSession }
        #endif
        DictationRig.start()
        return Dictation.shared
    }

    /// True while the session holds words it is about to hand over (the sink runs in `cleaning`): the text goes to the body, not to a field.
    var isDictating: Bool { controller.state == .cleaning || controller.state == .listening }

    public var mic: Mic {
        if case .downloading = model { return .downloading }
        switch controller.state {
        case .listening: return .listening
        case .cleaning: return .cleaning
        case .idle, .done: return textArrived ? .text : .idle
        }
    }

    public var liveWords: String { controller.liveWords }
    public var line: String { failure.isEmpty ? controller.line : failure }

    /// 0...1 for the download; 0 when there is none or no number.
    public var progress: Double {
        switch model {
        case .downloading(let p): return p ?? 0
        case .ready: return 1
        default: return 0
        }
    }
    public var percent: Int { Int((progress * 100).rounded()) }

    /// The microphone button. A tap while listening ends the session (the words arrive); with no model it starts the download and then listens.
    public func tap() {
        controller.onChange = { [weak self] in self?.revision += 1 }
        switch controller.state {
        case .listening: Task { _ = await controller.end() }; return
        case .cleaning: return
        default: break
        }
        switch model {
        case .downloading: return
        case .missing, .failed: startDownload()
        case .ready: Task { await beginListening() }
        }
    }

    /// Called by the composer when the draft is gone: the session and the arrived mark clear.
    public func reset() {
        textArrived = false; dictated = ""; failure = ""
        if controller.state != .idle { controller.reset() }
    }

    func delivered(_ text: String) { dictated = text; textArrived = true; revision += 1 }

    private func beginListening() async {
        textArrived = false; failure = ""
        _ = await controller.begin()
        revision += 1
    }

    private func startDownload() {
        guard downloading == nil else { return }
        failure = ""
        model = .downloading(DictationRig.active ? 0 : nil)         // the tap shows at once, before the first byte
        downloading = Task { [weak self] in
            do {
                try await Self.fetch { p in Task { @MainActor in self?.progressed(p) } }
                await self?.finished()
            } catch {
                await self?.failed()
            }
        }
    }

    private func progressed(_ p: Double) { if case .downloading = model { model = .downloading(p) } }

    private func finished() async {
        downloading = nil
        model = .ready
        if !DictationRig.active { UserDefaults.standard.set(true, forKey: Self.readyKey) }
        await beginListening()                                       // the owner tapped to dictate: listening starts when the model is there
    }

    private func failed() {
        downloading = nil
        let words = Words.text("The speech model could not be downloaded. Tap the microphone to try again.")
        model = .failed(words); failure = words
    }

    private nonisolated static func fetch(progress: @escaping @Sendable (Double) -> Void) async throws {
        if await DictationRig.active {
            for i in 1...rigSteps {
                try await Task.sleep(for: .milliseconds(rigStepMs))
                progress(Double(i) / Double(rigSteps))
            }
            return
        }
        #if canImport(FluidAudio)
        _ = try await AsrModels.downloadAndLoad(version: .v3)
        #else
        throw DictationError.engineUnavailable("no engine")
        #endif
    }

    /// `fixture speech-model missing|ready` (rig only): puts the model where the check needs it. Needs one line in `FixtureEvents.served`
    /// (shell, frozen): `"speech-model": 1...1,` (docs/lanes/composerphone.md).
    private func listenForRigVerbs() {
        tokens.append(NotificationCenter.default.addObserver(forName: FixtureEvents.name("speech-model"), object: nil, queue: .main) { n in
            let word = (n.userInfo?["args"] as? [String])?.first ?? ""
            MainActor.assumeIsolated { PhoneSpeech.shared.rigSet(word) }
        })
        tokens.append(NotificationCenter.default.addObserver(forName: FixtureEvents.name("reset"), object: nil, queue: .main) { _ in
            MainActor.assumeIsolated { PhoneSpeech.shared.rigSet("missing") }
        })
    }

    private func rigSet(_ word: String) {
        downloading?.cancel(); downloading = nil
        model = word == "ready" ? .ready : .missing
        textArrived = false; failure = ""
        revision += 1
    }
}
