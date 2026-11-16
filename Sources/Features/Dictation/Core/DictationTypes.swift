import Foundation

/// The four states the composer shows (fixtures/tests-j3.json `states`).
public enum DictationState: String, Sendable { case idle, listening, cleaning, done }

public enum DictationError: Error, Equatable, Sendable {
    case micRefused
    case cloudKeyMissing(provider: String)
    /// The local library or model is not on this device (the line sends the owner to Settings to get it).
    case engineUnavailable(String)
    /// The engine ran and could not read the recording.
    case engineFailed(String)
    case transport(String)
    case noAudio
    case recorderFailed
}

/// The one-line explanations the composer line shows (docs/CAPABILITY-CONTRACT.md C-J3.1, C-J3.2). Every line goes through `Words`, so it
/// is in the app's one language when it is made; a reason from a library or a provider never reaches the line in its own words.
public enum DictationLines {
    public static var micRefused: String { Words.text("The microphone is off for Whoosh. Open System Settings, Privacy and Security, Microphone, and turn Whoosh on.") }
    public static var cloudKeyMissing: String { Words.text("Add your cloud speech key in Settings: AI providers, speech and agents.") }
    public static var modelMissing: String { Words.text("The local speech model is not on this device yet. Open Settings: AI providers, speech and agents to download it.") }
    public static var noAudio: String { Words.text("Dictation stopped: nothing was recorded.") }
    public static var recorderFailed: String { Words.text("Dictation stopped: the microphone could not start.") }
    public static var engineFailed: String { Words.text("Dictation stopped: the speech engine could not read the recording.") }
    public static var noField: String { Words.text("Dictation found no text field to type into.") }
    public static var serviceFailed: String { Words.text("Dictation stopped: the speech service did not answer.") }
}

/// Turns recorded audio into raw words. Engines: local Parakeet (speech.cpp), cloud with the user's key, WhisperKit or FluidAudio on the iPhone.
public protocol Transcriber: Sendable {
    var name: String { get }
    func transcribe(audio: URL) async throws -> String
}

/// An engine that also gives words while the key is held. Each `feed` returns the words finalised by that chunk.
public protocol LiveTranscriber: Transcriber {
    func beginLive() -> LiveSession?
}

public protocol LiveSession: AnyObject, Sendable {
    func feed(_ pcm16k: [Float]) -> String
    func finish() -> String
}

/// Clean-up pass: filler words and punctuation, through the user's provider.
public protocol TextCleaner: Sendable {
    func clean(_ raw: String) async throws -> String
}

/// The user's chat provider, behind the provider port. Tests stub it; nothing here opens a connection.
public protocol ChatPort: Sendable {
    func complete(system: String, user: String) async throws -> String
}

/// One recording, in and out. Audio lives in a file only until it becomes text.
public protocol AudioRecorder: AnyObject, Sendable {
    func authorize() async -> Bool
    func start(into file: URL, onSamples: @escaping @Sendable ([Float]) -> Void) throws
    func stop() -> URL?
}
