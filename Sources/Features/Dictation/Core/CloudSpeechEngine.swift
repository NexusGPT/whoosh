import Foundation

public protocol SpeechKeyProvider: Sendable { func key(for provider: String) -> String? }
public protocol HTTPTransport: Sendable { func send(_ request: URLRequest) async throws -> (data: Data, status: Int) }

/// One cloud speech provider the user may pick, with the user's own key. Request shapes are the provider's documented
/// transcription endpoints (to be re-read against each provider's live docs before release; tests stub the transport).
public struct CloudSpeechProvider: Sendable, Equatable {
    public enum Shape: Sendable { case multipartBearer, multipartHeader(String), rawToken }
    public let id: String, endpoint: URL, model: String, shape: Shape
    public static func == (a: Self, b: Self) -> Bool { a.id == b.id && a.endpoint == b.endpoint && a.model == b.model }
    public static let openAI = Self(id: "openai", endpoint: URL(string: "https://api.openai.com/v1/audio/transcriptions")!, model: "gpt-4o-transcribe", shape: .multipartBearer)
    public static let groq = Self(id: "groq", endpoint: URL(string: "https://api.groq.com/openai/v1/audio/transcriptions")!, model: "whisper-large-v3-turbo", shape: .multipartBearer)
    public static let mistral = Self(id: "mistral", endpoint: URL(string: "https://api.mistral.ai/v1/audio/transcriptions")!, model: "voxtral-mini-latest", shape: .multipartBearer)
    public static let elevenLabs = Self(id: "elevenlabs", endpoint: URL(string: "https://api.elevenlabs.io/v1/speech-to-text")!, model: "scribe_v1", shape: .multipartHeader("xi-api-key"))
    public static let deepgram = Self(id: "deepgram", endpoint: URL(string: "https://api.deepgram.com/v1/listen?model=nova-3&smart_format=true")!, model: "nova-3", shape: .rawToken)
    public static let all = [openAI, groq, mistral, elevenLabs, deepgram]
}

public struct CloudSpeechEngine: Transcriber {
    public var name: String { "cloud:\(provider.id)" }
    let provider: CloudSpeechProvider, keys: any SpeechKeyProvider, http: any HTTPTransport
    public init(provider: CloudSpeechProvider, keys: any SpeechKeyProvider, http: any HTTPTransport) {
        self.provider = provider; self.keys = keys; self.http = http
    }

    public func transcribe(audio: URL) async throws -> String {
        guard let key = keys.key(for: provider.id), !key.isEmpty else { throw DictationError.cloudKeyMissing(provider: provider.id) }
        let wav = try Data(contentsOf: audio)
        var req = URLRequest(url: provider.endpoint)
        req.httpMethod = "POST"
        req.timeoutInterval = 30
        switch provider.shape {
        case .rawToken:
            req.setValue("Token \(key)", forHTTPHeaderField: "Authorization")
            req.setValue("audio/wav", forHTTPHeaderField: "Content-Type")
            req.httpBody = wav
        case .multipartBearer, .multipartHeader:
            let b = "whoosh-\(UUID().uuidString)"
            if case .multipartHeader(let h) = provider.shape { req.setValue(key, forHTTPHeaderField: h) } else { req.setValue("Bearer \(key)", forHTTPHeaderField: "Authorization") }
            req.setValue("multipart/form-data; boundary=\(b)", forHTTPHeaderField: "Content-Type")
            let field = provider.id == "elevenlabs" ? "model_id" : "model"
            var body = Data("--\(b)\r\nContent-Disposition: form-data; name=\"\(field)\"\r\n\r\n\(provider.model)\r\n".utf8)
            body.append(Data("--\(b)\r\nContent-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\nContent-Type: audio/wav\r\n\r\n".utf8))
            body.append(wav); body.append(Data("\r\n--\(b)--\r\n".utf8))
            req.httpBody = body
        }
        let (data, status) = try await http.send(req)
        guard (200..<300).contains(status) else { throw DictationError.transport("\(provider.id) answered \(status)") }
        return try Self.text(from: data)
    }

    /// `{"text": ...}` (OpenAI, Groq, Mistral, ElevenLabs) or Deepgram's `results.channels[0].alternatives[0].transcript`.
    static func text(from data: Data) throws -> String {
        guard let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { throw DictationError.transport("the speech reply was not JSON") }
        if let t = o["text"] as? String { return t.trimmingCharacters(in: .whitespacesAndNewlines) }
        if let r = o["results"] as? [String: Any], let c = (r["channels"] as? [[String: Any]])?.first,
           let t = ((c["alternatives"] as? [[String: Any]])?.first?["transcript"] as? String) { return t.trimmingCharacters(in: .whitespacesAndNewlines) }
        throw DictationError.transport("the speech reply held no text")
    }
}
