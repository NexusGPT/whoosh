import CryptoKit
import Foundation
#if canImport(MLXLLM)
import MLX
import MLXHuggingFace
import MLXLLM
import MLXLMCommon
import Tokenizers   // the tokenizer loader's macro expands to this module's names

/// A label reader on this Mac through MLX (mlx-swift-lm, MIT): a language model that reads the state and one question with lettered options, and
/// whose next-token probabilities over the letters are the answer (PelaAI KnowLine, Apache-2.0, on a Qwen 3.5 base). The text of the request is
/// `LabelReaderLayout`'s (a port of the authors' front end); this file is the authors' scoring: the model's own chat template with thinking off,
/// "Answer:" after the assistant's opening, one pass per question, a softmax over the letters only, the checkpoint's own temperatures.
/// The output layer is computed in 32-bit numbers for the letters alone, as the authors' serving setting does (--enable-fp32-lm-head): the
/// last position's hidden state times the letters' rows of the output weights. In the weights' 16-bit numbers one step of a logit near 20 is
/// 0.125, which moved a yes or no probability by up to 0.05. Nothing is generated: no token is sampled. `checks/label-reader-real-check.py` compares its numbers with the authors' on made-up mail.
public final class LabelReaderEngine: @unchecked Sendable {
    /// One question's answer: the probability of each answer key, in request order.
    public struct Answer: Sendable, Equatable {
        public let id: String
        public let keys: [String]
        public let probabilities: [Double]
    }

    private let container: ModelContainer
    public let checkpointName: String
    /// Divides the logits of one kind of question (the repository's temperatures.json; a kind not named reads at 1).
    let temperatures: [String: Double]
    private let lock = NSLock()
    private var letters: [String] = []
    private var letterIds: [Int] = []
    /// The letters' rows of the output weights in 32-bit numbers ([letters, hidden]), for the ids of `headIds`; nil when the model's
    /// parameters hold no output weights under a known name.
    private var head: MLXArray?
    private var headIds: [Int] = []

    /// The rows of the output weights for `ids`, cast to float32 once: the weights of `lm_head`, or the token embeddings when the model ties
    /// the two. Called inside the container's isolation only.
    private func headRows(_ model: any LanguageModel, ids: [Int]) -> MLXArray? {
        if headIds == ids { return head }
        let all = model.parameters().flattened()
        guard let weight = (all.first { $0.0.hasSuffix("lm_head.weight") } ?? all.first { $0.0.hasSuffix("embed_tokens.weight") })?.1, weight.ndim == 2 else {
            head = nil; headIds = ids; return nil
        }
        let rows = weight[MLXArray(ids.map { Int32($0) })].asType(.float32)
        eval(rows)
        head = rows; headIds = ids
        return rows
    }

    /// `folder` holds the checkpoint as its repository publishes it: config.json, the tokenizer's files, the weights, temperatures.json.
    public static func load(folder: URL, name: String) async throws -> LabelReaderEngine {
        let container = try await LLMModelFactory.shared.loadContainer(from: folder, using: #huggingFaceTokenizerLoader())
        var temperatures: [String: Double] = [:]
        if let data = try? Data(contentsOf: folder.appendingPathComponent("temperatures.json")),
           let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            for (k, v) in o { if let n = (v as? NSNumber)?.doubleValue, n > 0 { temperatures[k] = n } }
        }
        return LabelReaderEngine(container: container, name: name, temperatures: temperatures)
    }

    init(container: ModelContainer, name: String, temperatures: [String: Double]) {
        self.container = container; self.checkpointName = name; self.temperatures = temperatures
    }

    /// The letters that are ONE token right after "Answer:", with their token ids (the authors' find_labels): A to Z, then AA and on. Found
    /// once per count asked; a question with more options than the tokenizer has such letters is refused in words.
    func labels(_ tokenizer: any MLXLMCommon.Tokenizer, count: Int) throws -> (letters: [String], ids: [Int]) {
        lock.lock(); defer { lock.unlock() }
        if letters.count >= count { return (Array(letters.prefix(count)), Array(letterIds.prefix(count))) }
        let base = tokenizer.encode(text: LabelReaderLayout.answer, addSpecialTokens: false)
        var found: [String] = [], ids: [Int] = []
        for c in LabelReaderLayout.labelCandidates {
            let full = tokenizer.encode(text: LabelReaderLayout.answer + " " + c, addSpecialTokens: false)
            guard full.count == base.count + 1, Array(full.prefix(base.count)) == base, let id = full.last, !ids.contains(id),
                  tokenizer.decode(tokenIds: [id], skipSpecialTokens: false).trimmingCharacters(in: .whitespacesAndNewlines) == c else { continue }
            found.append(c); ids.append(id)
            if found.count == count { break }
        }
        guard found.count == count else {
            throw LocalModelError.failed(Words.text("The on-device model could not answer: %@", "options=\(count),letters=\(found.count)"))
        }
        letters = found; letterIds = ids
        return (found, ids)
    }

    /// One answer per question, in request order. The request is read twice when, read once, it costs at most
    /// `LabelReaderLayout.twiceMaxTokens` tokens (the authors' setting of their published runs); a longer one is read once.
    public func answer(state: String, questions: [LabelReaderLayout.Question]) async throws -> [Answer] {
        guard (1...LabelReaderLayout.maxQuestions).contains(questions.count) else {
            throw LocalModelError.failed(Words.text("The on-device model could not answer: %@", "questions=\(questions.count),limit=\(LabelReaderLayout.maxQuestions)"))
        }
        for q in questions where !(2...LabelReaderLayout.maxLabels).contains(q.keys.count) {
            throw LocalModelError.failed(Words.text("The on-device model could not answer: %@", "question=\(q.id),options=\(q.keys.count),limit=\(LabelReaderLayout.maxLabels)"))
        }
        let temperatures = temperatures
        return try await container.perform { [self] context in
            let tokenizer = context.tokenizer
            let most = questions.map { $0.keys.count }.max() ?? 2
            let (letters, letterIds) = try labels(tokenizer, count: most)
            let tail = tokenizer.encode(text: LabelReaderLayout.answer, addSpecialTokens: false)
            func ids(_ turns: [[String: String]]) throws -> [Int] {
                try tokenizer.applyChatTemplate(messages: turns.map { $0 as [String: any Sendable] }, tools: nil, additionalContext: ["enable_thinking": false]) + tail
            }
            let blocks = questions.map { LabelReaderLayout.block($0, labels: letters) }
            let once = try blocks.map { try ids(LabelReaderLayout.once(state: state, block: $0)) }
            // the shared start is counted once: its length is where two prompts with different one-letter blocks first differ
            var prefix = 0
            if questions.count > 1 {
                let a = try ids(LabelReaderLayout.once(state: state, block: "A")), b = try ids(LabelReaderLayout.once(state: state, block: "B"))
                prefix = zip(a, b).prefix { $0 == $1 }.count
            }
            let onceTokens = LabelReaderLayout.onceTokens(prefix: prefix, prompts: once.map { $0.count })
            let twice = onceTokens <= LabelReaderLayout.twiceMaxTokens
            let prompts = twice ? try blocks.map { try ids(LabelReaderLayout.twice(state: state, block: $0)) } : once

            MLXFailureGuard.install()   // a GPU timeout is recorded, never fatal (as LayaEngine)
            var out: [Answer] = []
            for (q, prompt) in zip(questions, prompts) {
                // MLX keeps the buffers of every evaluated sequence length until the model unloads: they go after each question (as LayaEngine)
                defer { MLX.Memory.clearCache() }
                let wanted = Array(letterIds.prefix(q.keys.count))
                // the model is asked for its last hidden state beside its logits (the key the package's own drafter reads); a model that
                // gives none is read from its logits
                var ask = LMOutput.State()
                ask[mtpEmitFlagKey] = true
                let output = context.model(LMInput.Text(tokens: MLXArray(prompt.map { Int32($0) })[.newAxis]), cache: nil, state: ask)
                let logprobs: MLXArray
                if let hidden = output.state?[mtpLastHiddenStatesKey], let rows = headRows(context.model, ids: letterIds) {
                    logprobs = matmul(rows[0 ..< wanted.count], hidden[0, -1].asType(.float32))   // the letters' logits: one constant from their log-probabilities
                } else {
                    let row = output.logits[0, -1].asType(.float32)
                    logprobs = (row - row.logSumExp())[MLXArray(wanted.map { Int32($0) })]
                }
                let mark = MLXFailureGuard.failures
                eval(logprobs)
                do { try MLXFailureGuard.check(since: mark) }
                catch let f as EmbedStepFailure { throw LocalModelError.failed(Words.text("The on-device model could not answer: %@", f.detail)) }
                let lp = logprobs.asArray(Float.self).map(Double.init)
                let t = temperatures[q.kind.rawValue] ?? 1
                guard let p = LabelReaderLayout.probabilities(lp, temperature: t) else {
                    throw LocalModelError.failed(Words.text("The on-device model could not answer: %@", "no-probability,question=\(q.id)"))
                }
                LabelReaderTrace.note(id: q.id, kind: q.kind.rawValue, twice: twice, onceTokens: onceTokens, ids: prompt, labelIds: wanted, logprobs: lp,
                                      temperature: t, probabilities: p)
                out.append(Answer(id: q.id, keys: q.keys, probabilities: p))
            }
            return out
        }
    }
}
#else
/// Built without the MLX language runtime: this build cannot run a label reader.
public final class LabelReaderEngine: @unchecked Sendable {
    public struct Answer: Sendable, Equatable {
        public let id: String
        public let keys: [String]
        public let probabilities: [Double]
    }
    public let checkpointName = ""
    public static func load(folder: URL, name: String) async throws -> LabelReaderEngine {
        throw LocalModelError.unavailable("This build of Whoosh does not include the on-device language runtime yet.")
    }
    public func answer(state: String, questions: [LabelReaderLayout.Question]) async throws -> [Answer] {
        throw LocalModelError.unavailable("This build of Whoosh does not include the on-device language runtime yet.")
    }
}
#endif

/// Rig copies only: one JSON line per question in the file `WHOOSH_LABEL_READER_TRACE` names (the hidden check
/// `checks/label-reader-real-check.py`): the question's id and kind, whether the request was read twice, the prompt's token count and the hash
/// of its token ids, the letters' token ids, their log-probabilities, the temperature, the probabilities. No text of the question or the mail.
/// The owner's copy writes nothing.
enum LabelReaderTrace {
    static let path: String? = {
        let env = ProcessInfo.processInfo.environment
        guard CorePlace.current(env: env).rig, let p = env["WHOOSH_LABEL_READER_TRACE"], !p.isEmpty else { return nil }
        return p
    }()
    private static let lock = NSLock()

    /// The hash of a prompt's token ids, written as Python's `json.dumps` writes a list of numbers ("[1, 2, 3]"): the reference's `ids_sha256`.
    static func hash(_ ids: [Int]) -> String {
        let text = "[" + ids.map(String.init).joined(separator: ", ") + "]"
        return SHA256.hash(data: Data(text.utf8)).map { String(format: "%02x", $0) }.joined()
    }

    static func note(id: String, kind: String, twice: Bool, onceTokens: Int, ids: [Int], labelIds: [Int], logprobs: [Double], temperature: Double,
                     probabilities: [Double]) {
        guard let path else { return }
        let line: [String: Any] = ["id": id, "kind": kind, "read": twice ? "twice" : "once", "once_tokens": onceTokens, "tokens": ids.count,
                                   "ids_sha256": hash(ids), "last_ids": Array(ids.suffix(12)), "label_ids": labelIds, "logprobs": logprobs,
                                   "temperature": temperature, "probabilities": probabilities]
        guard let data = try? JSONSerialization.data(withJSONObject: line) else { return }
        lock.lock(); defer { lock.unlock() }
        if !FileManager.default.fileExists(atPath: path) { FileManager.default.createFile(atPath: path, contents: nil) }
        guard let h = FileHandle(forWritingAtPath: path) else { return }
        defer { try? h.close() }
        _ = try? h.seekToEnd()
        try? h.write(contentsOf: data + Data("\n".utf8))
    }
}
