import Foundation
#if canImport(MLXLLM)
import MLX
import MLXHuggingFace
import MLXLLM
import MLXLMCommon
import Tokenizers
#endif

/// The MLX language-model backend: runs the installed instruct model on Apple silicon through mlx-swift-lm.
/// It answers only from files the install step verified; until then it says so in words.
public final class MlxBackend: TextBackend, @unchecked Sendable {
    public let name = Words.text("Language model")
    public let installer: ModelInstaller
    private let runtime = MlxRuntime()

    public init(installer: ModelInstaller) { self.installer = installer }

    public func availability() -> LocalModelAvailability {
        let s = installer.state
        switch s {
        case .ready: return .ready
        case .notInstalled: return .unavailable(reason: Words.text("On-device model not installed yet. Set up on-device AI once and it runs on this Mac."))
        case .downloading, .verifying, .paused: return .unavailable(reason: Words.text("On-device model not installed yet. %@", s.words))
        case .failed(let r), .corrupt(let r): return .unavailable(reason: Words.text("On-device model not installed yet. %@", r))
        }
    }

    public func respond(prompt: String, maxTokens: Int) async throws -> String {
        let weights = installer.directory.appendingPathComponent("model.safetensors")
        if let problem = SafetensorsCheck.problem(in: weights) {
            let reason = Words.text("%@ Set up on-device AI again to download a fresh copy.", problem)
            installer.markCorrupt(reason)
            throw LocalModelError.unavailable(reason)
        }
        do { return try await runtime.generate(directory: installer.directory, prompt: prompt, maxTokens: maxTokens) }
        catch let e as LocalModelError { throw e }
        catch { throw LocalModelError.failed(Words.text("The on-device model could not answer: %@", error.localizedDescription)) }
    }
}

/// Loads the model on the first question and lets it go after the idle time (ModelResidency); nothing loads at launch.
final class MlxRuntime: @unchecked Sendable {
    #if canImport(MLXLLM)
    private struct Loaded: @unchecked Sendable { let container: ModelContainer }
    private let residency = ModelResidency<Loaded>(onUnload: { MLX.Memory.clearCache() })
    func generate(directory: URL, prompt: String, maxTokens: Int) async throws -> String {
        let load: @Sendable () async throws -> Loaded = {
            Loaded(container: try await LLMModelFactory.shared.loadContainer(from: directory, using: #huggingFaceTokenizerLoader()))
        }
        MLXFailureGuard.install()   // hid-ai round 4 (hid-search F6): a GPU timeout is recorded, never fatal
        return try await residency.use(load: load) { loaded in
            let session = ChatSession(loaded.container, generateParameters: GenerateParameters(maxTokens: maxTokens, temperature: 0.3))
            let mark = MLXFailureGuard.failures
            let answer = try await session.respond(to: prompt).trimmingCharacters(in: .whitespacesAndNewlines)
            do { try MLXFailureGuard.check(since: mark) }   // the library failed while answering (a Metal command buffer, GPU timeout): this answer is not kept
            catch let f as EmbedStepFailure { throw LocalModelError.failed(Words.text("The on-device model could not answer: %@", f.detail)) }
            return answer
        }
    }
    #else
    func generate(directory: URL, prompt: String, maxTokens: Int) async throws -> String {
        throw LocalModelError.unavailable(Words.text("This build of Whoosh does not include the on-device language runtime yet."))
    }
    #endif
}
