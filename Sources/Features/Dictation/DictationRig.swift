import Foundation
#if os(macOS)
import AppKit
#endif

/// RIG ONLY. Under `--fixture-core` the dictation session hears no microphone: its recorder is a `FixtureRecorder`, its engine a
/// `DictationFixtureSpeech`, and the socket's fixture verbs (posted as notifications by the shell, `whoosh.fixture.<verb>`) drive them:
///   fixture speak <words...>              the words the "microphone" hears while the key is held
///   fixture speak-file <path> <words...>  the same, with a recorded file as the audio (it is deleted when it becomes text)
///   fixture mic-refuse                    the microphone permission is refused until `fixture reset`
///   fixture cloud-no-key                  the engine is a cloud engine with no key until `fixture reset`
///   fixture caret <n>                     the caret of the composer's body goes to offset n (clamped to the text), so the next dictation lands there
///   fixture reset                         the session, the refusal and the key are back to normal
/// The verbs `speak-file`, `mic-refuse` and `cloud-no-key` need one line each in `FixtureEvents.served` (shell, frozen): see docs/lanes/dictation.md "## Round 2".
/// The recogniser, the clean-up, the insertion and the audio file's removal that these verbs exercise are the production ones.
@MainActor
enum DictationRig {
    static var active: Bool { CoreHost.shared.fixture != nil || CorePlace.onRig }   // probes-r1 R4: any rig copy hears no microphone, whatever its core
    static let recorder = FixtureRecorder()
    static let speech = DictationFixtureSpeech()
    private(set) static var cloudNoKey = false
    private static var tokens: [NSObjectProtocol] = []

    /// The engine the session uses now: the cloud engine with no key (it refuses before any request), else the fixture speech.
    static func engine() -> any Transcriber {
        cloudNoKey ? CloudSpeechEngine(provider: .openAI, keys: NoSpeechKeys(), http: NoHTTP()) : speech
    }

    /// Hears the verbs. Called from the feature's registrations, so it is in place before the first key the rig presses.
    static func start() {
        guard active, tokens.isEmpty else { return }
        let c = Dictation.shared
        func on(_ verb: String, _ body: @escaping @MainActor ([String]) -> Void) {
            tokens.append(NotificationCenter.default.addObserver(forName: Notification.Name("whoosh.fixture.\(verb)"), object: nil, queue: .main) { n in
                let args = n.userInfo?["args"] as? [String] ?? []
                MainActor.assumeIsolated { body(args) }
            })
        }
        on("speak") { a in speech.say(a.joined(separator: " ")); recorder.hear(samples: [0]) }
        on("speak-file") { a in
            guard let path = a.first else { return }
            speech.say(a.dropFirst().joined(separator: " "))
            recorder.speak(file: URL(fileURLWithPath: path), samples: [0])
        }
        on("caret") { a in
            #if os(macOS)
            // The same text view the composer inserts into (ComposerModel.insertAtCaret): its own window's first responder, else the key window's.
            let window = WindowBridge.window(for: ComposerContext.slot) ?? (NSApp as NSApplication?)?.keyWindow
            guard let n = a.first.flatMap({ Int($0) }), let tv = window?.firstResponder as? NSTextView, tv.isEditable else { return }
            tv.setSelectedRange(NSRange(location: min(max(n, 0), (tv.string as NSString).length), length: 0))
            #endif
        }
        on("mic-refuse") { _ in recorder.setDenied(true) }
        on("cloud-no-key") { _ in cloudNoKey = true }
        on("reset") { _ in recorder.setDenied(false); _ = recorder.stop(); cloudNoKey = false; speech.say(""); c.reset() }
    }
}

struct NoSpeechKeys: SpeechKeyProvider { func key(for provider: String) -> String? { nil } }
struct NoHTTP: HTTPTransport {
    func send(_ request: URLRequest) async throws -> (data: Data, status: Int) { throw DictationError.transport("rig.no-request") }
}

/// The recogniser of the rig: it "recognises" the words it was told, and only from a recording that exists, so a recording
/// removed before it was read is a failure here exactly as it is for a real engine. Live words arrive once, with the first samples.
final class DictationFixtureSpeech: LiveTranscriber, @unchecked Sendable {
    let name = "fixture"
    private let lock = NSLock()
    private var words = "", livePending = ""

    func say(_ w: String) { lock.withLock { words = w; livePending = w } }

    func transcribe(audio: URL) async throws -> String {
        guard FileManager.default.fileExists(atPath: audio.path) else { throw DictationError.engineFailed("rig.recording-gone") }
        return lock.withLock { let w = words; words = ""; return w }
    }

    func beginLive() -> LiveSession? { Live(owner: self) }

    private final class Live: LiveSession, @unchecked Sendable {
        let owner: DictationFixtureSpeech
        init(owner: DictationFixtureSpeech) { self.owner = owner }
        func feed(_ pcm16k: [Float]) -> String { owner.lock.withLock { let w = owner.livePending; owner.livePending = ""; return w } }
        func finish() -> String { "" }
    }
}
