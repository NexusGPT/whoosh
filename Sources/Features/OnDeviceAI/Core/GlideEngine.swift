import Foundation
#if canImport(MLXLLM)
import MLX
import MLXFast
import MLXHuggingFace
import MLXLLM
import MLXLMCommon
import Tokenizers   // the tokenizer loader's macro expands to this module's names

/// A cache entry for one full-attention layer of the Qwen 3.5 text model that keeps nothing and asks for no mask: the layer then attends
/// to every token of the prompt, before and after, which is how GLiDE's authors run those layers (decision_config.json `attention_mode`
/// "noncausal_full_attention"; glide/model.py `attention_masks`). The package's model asks its cache entry for the mask
/// (`createAttentionMask(h:cache:)` calls `makeMask`) and hands it the keys and values (`attentionWithCacheUpdate` calls `update`), so the
/// package's own forward pass runs unchanged. The linear-attention layers keep the package's own cache entry and their recurrence.
final class GlideTwoWayCache: BaseKVCache {
    override func update(keys: MLXArray, values: MLXArray) -> (MLXArray, MLXArray) { (keys, values) }
    override func makeMask(n: Int, windowSize: Int?, returnArray: Bool) -> MLXFast.ScaledDotProductAttentionMaskMode { .none }
    override func copy() -> any KVCache { GlideTwoWayCache() }
}

/// GLiDE on this Mac through MLX (mlx-swift-lm, MIT): a Qwen 3.5 text model that reads one prompt per question, whose last token's hidden
/// state goes through a 255-way readout; a softmax over the question's options is the answer (fastino/GLiDE, Fastino Community License).
/// The text of the request is `GlideLayout`'s (a port of the authors' prompt code); this file is the authors' scoring (glide/model.py): one
/// pass per question, the full-attention layers with no causal mask, the readout in 32-bit numbers on the last position, the
/// checkpoint's temperature. Nothing is generated: no token is sampled.
/// `checks/corpora/glide` holds the authors' answers on made-up mail and the measure of this route in Python (worst difference 0.039 with
/// the published 16-bit weights, 0.003 with 32-bit ones, 0.54 when the mask is left causal).
public final class GlideEngine: @unchecked Sendable {
    /// One question's answer: the probability of each answer key, in request order.
    public struct Answer: Sendable, Equatable {
        public let id: String
        public let keys: [String]
        public let probabilities: [Double]
    }

    private let container: ModelContainer
    public let checkpointName: String
    /// The option codes in order (decision_config.json `codes`: A, B, C ...), as many as the readout has outputs.
    let codes: [String]
    /// Divides the readout's logits (decision_config.json `temperature`).
    let temperature: Double
    /// The readout's weights in 32-bit numbers ([codes, hidden]).
    private let readout: MLXArray

    /// `folder` holds the checkpoint as its repository publishes it: config.json, the tokenizer's files, the weights and their index,
    /// decision_config.json, readout.safetensors. A checkpoint whose decision_config asks for what this engine does not do (another
    /// attention mode or pooling, a loop) is refused: its answers would not be the authors'.
    public static func load(folder: URL, name: String) async throws -> GlideEngine {
        func refuse(_ detail: String) -> LocalModelError { .failed(Words.text("The on-device model could not answer: %@", detail)) }
        guard let data = try? Data(contentsOf: folder.appendingPathComponent("decision_config.json")),
              let config = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let codes = config["codes"] as? [String], !codes.isEmpty else { throw refuse("decision_config=unreadable") }
        guard (config["attention_mode"] as? String) == "noncausal_full_attention", (config["pooling"] as? String) == "last",
              config["loop"] == nil, config["midloop"] == nil else { throw refuse("decision_config=unsupported") }
        let temperature = (config["temperature"] as? NSNumber)?.doubleValue ?? 1
        guard temperature > 0 else { throw refuse("temperature=\(temperature)") }
        guard let weight = try loadArrays(url: folder.appendingPathComponent("readout.safetensors"))["weight"], weight.ndim == 2,
              weight.dim(0) == codes.count else { throw refuse("readout=unreadable") }
        let readout = weight.asType(.float32)
        eval(readout)
        // the weights are the files the checkpoint's own index names (the package's default): readout.safetensors is not among them
        let container = try await LLMModelFactory.shared.loadContainer(from: folder, using: #huggingFaceTokenizerLoader())
        return GlideEngine(container: container, name: name, codes: codes, temperature: temperature, readout: readout)
    }

    init(container: ModelContainer, name: String, codes: [String], temperature: Double, readout: MLXArray) {
        self.container = container; self.checkpointName = name; self.codes = codes; self.temperature = temperature; self.readout = readout
    }

    /// One answer per question, in request order: one prompt and one pass each.
    public func answer(state: String, questions: [GlideLayout.Question]) async throws -> [Answer] {
        let most = min(GlideLayout.maxOptions, codes.count)
        for q in questions where !(1...most).contains(q.keys.count) {
            throw LocalModelError.failed(Words.text("The on-device model could not answer: %@", "question=\(q.id),options=\(q.keys.count),limit=\(most)"))
        }
        return try await container.perform { [self] context in
            let tokenizer = context.tokenizer
            MLXFailureGuard.install()   // a GPU timeout is recorded, never fatal (as LayaEngine)
            var out: [Answer] = []
            for q in questions {
                // MLX keeps the buffers of every evaluated sequence length until the model unloads: they go after each question (as LayaEngine)
                defer { MLX.Memory.clearCache() }
                let ids = tokenizer.encode(text: GlideLayout.prompt(state: state, question: q, codes: codes), addSpecialTokens: false)
                // a new set of cache entries per question: the package's own for the linear-attention layers, the two-way one for the others
                let caches: [KVCache] = try context.model.newCache(parameters: nil).map { $0 is MambaCache ? $0 : GlideTwoWayCache() }
                var ask = LMOutput.State()
                ask[mtpEmitFlagKey] = true   // the model gives its last hidden states beside its logits (the key the package's own drafter reads)
                let output = context.model(LMInput.Text(tokens: MLXArray(ids.map { Int32($0) })[.newAxis]), cache: caches, state: ask)
                guard let hidden = output.state?[mtpLastHiddenStatesKey] else {
                    throw LocalModelError.failed(Words.text("The on-device model could not answer: %@", "no-hidden-state,question=\(q.id)"))
                }
                let logits = matmul(readout[0 ..< q.keys.count], hidden[0, -1].asType(.float32))
                let mark = MLXFailureGuard.failures
                eval(logits)
                do { try MLXFailureGuard.check(since: mark) }
                catch let f as EmbedStepFailure { throw LocalModelError.failed(Words.text("The on-device model could not answer: %@", f.detail)) }
                let row = logits.asArray(Float.self).map(Double.init)
                guard let p = GlideLayout.probabilities(row, temperature: temperature) else {
                    throw LocalModelError.failed(Words.text("The on-device model could not answer: %@", "no-probability,question=\(q.id)"))
                }
                LabelReaderTrace.note(id: q.id, kind: q.kind.rawValue, twice: false, onceTokens: ids.count, ids: ids, labelIds: [], logprobs: row,
                                      temperature: temperature, probabilities: p)
                out.append(Answer(id: q.id, keys: q.keys, probabilities: p))
            }
            return out
        }
    }
}
#else
/// Built without the MLX language runtime: this build cannot run GLiDE.
public final class GlideEngine: @unchecked Sendable {
    public struct Answer: Sendable, Equatable {
        public let id: String
        public let keys: [String]
        public let probabilities: [Double]
    }
    public let checkpointName = ""
    public static func load(folder: URL, name: String) async throws -> GlideEngine {
        throw LocalModelError.unavailable("This build of Whoosh does not include the on-device language runtime yet.")
    }
    public func answer(state: String, questions: [GlideLayout.Question]) async throws -> [Answer] {
        throw LocalModelError.unavailable("This build of Whoosh does not include the on-device language runtime yet.")
    }
}
#endif
