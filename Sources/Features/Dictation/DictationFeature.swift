import Foundation

public extension Notification.Name {
    /// Finished dictation text; `userInfo["text"]` is a String. The composer observes it and inserts at its cursor.
    static let dictationText = Notification.Name("whoosh.dictation.text")
}

/// The app-wide dictation session. Defaults: the microphone; the local engine when its library and model are in the app container, otherwise an
/// engine that says the model is missing; the on-device clean-up (`LocalCleaner`). Settings (the speech-engine record) swap `engine`/`cleaner`,
/// for a cloud engine with the owner's key or a provider pass after the local one (`ChainedCleaner`). Under `--fixture-core` the session
/// hears no microphone at all (`DictationRig`).
@MainActor
public enum Dictation {
    public static let shared: DictationController = {
        let rig = DictationRig.active
        let c = DictationController(recorder: rig ? DictationRig.recorder : MicRecorder(),
                                    engine: { rig ? DictationRig.engine() : localEngine() },
                                    cleaner: LocalCleaner(language: { Words.current }))
        // Finished text goes to the field that holds the keys; with none, the line says so and nothing is typed anywhere.
        c.sink = { [unowned c] text in if !DictationInsert.deliver(text, AppModel.shared) { c.line = DictationLines.noField } }
        return c
    }()

    nonisolated static var supportDirectory: URL {
        ModelsFolder.whooshRoot().appendingPathComponent("Speech", isDirectory: true)
    }
    nonisolated static var libraryURL: URL { supportDirectory.appendingPathComponent("libparakeet.dylib") }
    nonisolated static var modelURL: URL { supportDirectory.appendingPathComponent("parakeet-tdt-0.6b-v3.gguf") }

    /// The one local engine: nothing loads until the first dictation, and the library and model go again after EMBED_IDLE_UNLOAD_S idle.
    static let local = IdleLocalEngine(make: {
        let lib = libraryURL, model = modelURL
        guard FileManager.default.fileExists(atPath: model.path) else { throw DictationError.engineUnavailable("the local speech model is not on this device") }
        return try ParakeetCppEngine(library: lib, model: model)
    })

    static func localEngine() -> any Transcriber {
        FileManager.default.fileExists(atPath: modelURL.path) ? local : NoEngine()
    }

    struct NoEngine: Transcriber {
        let name = "none"
        func transcribe(audio: URL) async throws -> String { throw DictationError.engineUnavailable("the local speech model is not on this device") }
    }
}

public enum DictationFeature: FeatureModule {
    public static let id = "dictation"
    public static let surfaces: [String] = []

    /// Hold Globe (Fn): the shortcuts engine routes the chord pair here. A press while a session runs, or a release with none, does nothing (false).
    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        DictationRig.start()   // the rig's verbs are heard from the first key on; a no-op outside --fixture-core
        return [
            "Fn:down": { _ in
                guard Dictation.shared.state == .idle else { return false }
                Task { @MainActor in await Dictation.shared.begin() }
                return true
            },
            "Fn:up": { _ in
                guard Dictation.shared.state == .listening else { return false }
                Task { @MainActor in await Dictation.shared.end() }
                return true
            },
        ]
    }

    /// The composer's dictation state and line, as one row: socket `rows mac-composer.dictation`.
    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        DictationRig.start()
        return ["mac-composer.dictation": { _ in
            let d = Dictation.shared
            return [["state": d.state.rawValue, "live": d.liveWords, "line": d.line, "engine": d.lastEngine,
                     "latency_ms": d.lastLatencyMs ?? NSNull(), "transcribe_ms": d.lastTranscribeMs ?? NSNull(),
                     "audio_files": d.store.files.count]]
        }]
    }

    public static func paletteCommands() -> [(title: String, key: String)] { [(Words.text("Dictate (hold to talk)"), "Fn")] }
}
