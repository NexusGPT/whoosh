import Foundation
#if canImport(MLX) && canImport(Tokenizers)
import MLX
import MLXFast
import MLXNN

/// Laya (convaiinnovations/laya, Apache-2.0) typed-decisions checkpoint on this Mac through MLX (mlx-swift, MIT):
/// ModernBERT-large encoder, a two-layer pre-norm transformer head, a marker scorer. A Swift port of the authors'
/// inference path (rl_agent_api.py, rl_common.py) and of Hugging Face's ModernBERT forward; no Python at run time.
public final class LayaEngine: @unchecked Sendable {
    struct Config {
        var maxLen: Int, headMaxLen: Int
        var temperature: [Double]
        var temperatureByOptions: [String: Double]
        var hidden: Int, layers: Int, heads: Int, localWindow: Int
        var normEps: Float, thetaFull: Float, thetaLocal: Float
        var layerIsFull: [Bool]
        var headLayers: Int
    }
    let cfg: Config
    let w: [String: MLXArray]
    let tokenizer: LayaTokenizer
    public let checkpointName: String

    /// `folder` is the store's `typed-decisions` folder.
    public static func load(folder: URL, float32: Bool = false) async throws -> LayaEngine {
        func json(_ p: String) throws -> [String: Any] {
            let d = try Data(contentsOf: folder.appendingPathComponent(p))
            guard let o = try JSONSerialization.jsonObject(with: d) as? [String: Any] else { throw LocalModelError.failed("\(p) is not a JSON object.") }
            return o
        }
        let rl = try json("rl_agent_config.json"), enc = try json("encoder/config.json")
        func num(_ o: [String: Any], _ k: String) throws -> Double {
            guard let v = (o[k] as? NSNumber)?.doubleValue else { throw LocalModelError.failed("The decision model config has no \(k). Delete the model and download it again.") }
            return v
        }
        let rope = enc["rope_parameters"] as? [String: [String: Any]] ?? [:]
        let types = (enc["layer_types"] as? [String]) ?? []
        let cfg = Config(
            maxLen: Int(try num(rl, "max_len")), headMaxLen: Int(try num(rl, "head_max_len")),
            temperature: (rl["temperature"] as? [NSNumber])?.map { $0.doubleValue } ?? [1, 1, 1],
            temperatureByOptions: (rl["temperature_by_options"] as? [String: NSNumber])?.mapValues { $0.doubleValue } ?? [:],
            hidden: Int(try num(enc, "hidden_size")), layers: Int(try num(enc, "num_hidden_layers")),
            heads: Int(try num(enc, "num_attention_heads")), localWindow: Int(try num(enc, "local_attention")),
            normEps: Float(try num(enc, "norm_eps")),
            thetaFull: Float((rope["full_attention"]?["rope_theta"] as? NSNumber)?.doubleValue ?? 160000),
            thetaLocal: Float((rope["sliding_attention"]?["rope_theta"] as? NSNumber)?.doubleValue ?? 10000),
            layerIsFull: types.map { $0 == "full_attention" },
            headLayers: Int(try num(rl, "head_layers")))
        guard cfg.layerIsFull.count == cfg.layers else { throw LocalModelError.failed("The decision model config lists \(cfg.layerIsFull.count) layer types for \(cfg.layers) layers.") }
        let tok = try await LayaTokenizer(folder: folder.appendingPathComponent("tokenizer"))
        var weights = try loadArrays(url: folder.appendingPathComponent("model.safetensors"))
        if float32 { weights = weights.mapValues { $0.asType(.float32) } }
        return LayaEngine(cfg: cfg, w: weights, tokenizer: tok, name: (rl["model_name"] as? String) ?? "laya-typed-decisions")
    }

    init(cfg: Config, w: [String: MLXArray], tokenizer: LayaTokenizer, name: String) {
        self.cfg = cfg; self.w = w; self.tokenizer = tokenizer; self.checkpointName = name
    }

    private func p(_ k: String) -> MLXArray { w[k]! }
    private func lin(_ x: MLXArray, _ k: String, bias: Bool = false) -> MLXArray {
        let y = matmul(x, p(k + ".weight").T)
        return bias ? y + p(k + ".bias") : y
    }
    private func norm(_ x: MLXArray, _ k: String, bias: Bool = false, eps: Float) -> MLXArray {
        MLXFast.layerNorm(x, weight: p(k + ".weight"), bias: bias ? p(k + ".bias") : nil, eps: eps)
    }

    /// Hidden states for one sequence (batch of one, so no padding mask).
    func encode(_ ids: [Int], qtype: Int) -> MLXArray {
        let L = ids.count, H = cfg.heads, D = cfg.hidden / cfg.heads
        var h = p("encoder.embeddings.tok_embeddings.weight")[MLXArray(ids.map { Int32($0) })]
        h = norm(h, "encoder.embeddings.norm", eps: cfg.normEps)[.newAxis]
        // sliding-window layers see keys within half the window each side
        let idx = MLXArray(0 ..< Int32(L))
        let dist = abs(idx.reshaped(L, 1) - idx.reshaped(1, L))
        let localMask = MLX.where(dist .<= Int32(cfg.localWindow / 2), MLXArray(Float(0)), MLXArray(-Float.infinity)).asType(h.dtype)
        for i in 0 ..< cfg.layers {
            let pre = "encoder.layers.\(i)"
            let a = i == 0 ? h : norm(h, pre + ".attn_norm", eps: cfg.normEps)
            let qkv = lin(a, pre + ".attn.Wqkv").reshaped(1, L, 3, H, D)
            let theta = cfg.layerIsFull[i] ? cfg.thetaFull : cfg.thetaLocal
            var q = qkv[0..., 0..., 0].transposed(0, 2, 1, 3)
            var k = qkv[0..., 0..., 1].transposed(0, 2, 1, 3)
            let v = qkv[0..., 0..., 2].transposed(0, 2, 1, 3)
            q = MLXFast.RoPE(q, dimensions: D, traditional: false, base: theta, scale: 1, offset: 0)
            k = MLXFast.RoPE(k, dimensions: D, traditional: false, base: theta, scale: 1, offset: 0)
            let o = MLXFast.scaledDotProductAttention(queries: q, keys: k, values: v, scale: 1 / Float(D).squareRoot(),
                                                      mask: cfg.layerIsFull[i] ? .none : .array(localMask))
            h = h + lin(o.transposed(0, 2, 1, 3).reshaped(1, L, H * D), pre + ".attn.Wo")
            let m = lin(norm(h, pre + ".mlp_norm", eps: cfg.normEps), pre + ".mlp.Wi")
            let parts = split(m, parts: 2, axis: -1)
            h = h + lin(gelu(parts[0]) * parts[1], pre + ".mlp.Wo")
        }
        h = norm(h, "encoder.final_norm", eps: cfg.normEps)
        h = h + p("type_emb.weight")[qtype]
        // decision head: torch TransformerEncoderLayer, norm_first, relu feed-forward, eps 1e-5
        let hd = cfg.hidden / (cfg.hidden / 64), hn = cfg.hidden / 64
        for i in 0 ..< cfg.headLayers {
            let pre = "head.layers.\(i)"
            let n1 = norm(h, pre + ".norm1", bias: true, eps: 1e-5)
            let qkvh = matmul(n1, p(pre + ".self_attn.in_proj_weight").T) + p(pre + ".self_attn.in_proj_bias")
            let t = split(qkvh, parts: 3, axis: -1).map { $0.reshaped(1, L, hn, hd).transposed(0, 2, 1, 3) }
            let ao = MLXFast.scaledDotProductAttention(queries: t[0], keys: t[1], values: t[2], scale: 1 / Float(hd).squareRoot(), mask: .none)
            h = h + lin(ao.transposed(0, 2, 1, 3).reshaped(1, L, cfg.hidden), pre + ".self_attn.out_proj", bias: true)
            let n2 = norm(h, pre + ".norm2", bias: true, eps: 1e-5)
            h = h + lin(relu(lin(n2, pre + ".linear1", bias: true)), pre + ".linear2", bias: true)
        }
        return h
    }

    /// Option logits for one question (uncalibrated).
    func logits(state: String, question q: LayaQuestion) throws -> [Double] {
        try logitsTraced(state: state, question: q).logits
    }

    /// `logits`, with the size of each part of the sequence (the rig's trace, `LayaTrace`).
    func logitsTraced(state: String, question q: LayaQuestion) throws -> (logits: [Double], parts: LayaSequence.Parts) {
        let seq = LayaSequence.buildTraced(tokenizer: tokenizer, state: state, question: q, maxLen: cfg.maxLen, headMaxLen: cfg.headMaxLen)
        guard seq.markers.count == q.renderedOptions().count else {
            throw LocalModelError.failed("Question \(q.id): its options do not fit in the model's \(cfg.headMaxLen)-token head. Shorten the option descriptions.")
        }
        let h = encode(seq.ids, qtype: q.kind.index)
        let m = h[0][MLXArray(seq.markers.map { Int32($0) })]
        var s = norm(m, "scorer.0", bias: true, eps: 1e-5)
        s = lin(s, "scorer.1", bias: true)
        s = lin(gelu(s), "scorer.3", bias: true)
        let out = s.reshaped(-1).asType(.float32)
        MLXFailureGuard.install()   // hid-ai round 4 (hid-search F6): a GPU timeout is recorded, never fatal
        let mark = MLXFailureGuard.failures
        eval(out)
        do { try MLXFailureGuard.check(since: mark) }   // the evaluation failed (GPU timeout): no logits from it, the caller gets the sentence
        catch let f as EmbedStepFailure { throw LocalModelError.failed(Words.text("The on-device model could not answer: %@", f.detail)) }
        return (out.asArray(Float.self).map(Double.init), seq.parts)
    }

    static func bucket(_ kind: LayaQuestion.Kind, _ k: Int) -> String {
        let size = k <= 2 ? "2" : k <= 5 ? "3-5" : k <= 10 ? "6-10" : "11+"
        return "\(kind.rawValue):\(size)"
    }

    /// One answer per question, calibrated with the checkpoint's own temperatures.
    public func answer(state: String, questions: [LayaQuestion]) throws -> [LayaAnswer] {
        try questions.map { q in
            // The owner's Mac, 2026-11-12: the installed app reached 10 to 19 GB while a batch of mail was judged (memguard.log, four times).
            // MLX keeps the buffers of every evaluated sequence length in its cache until the model unloads; they go after each question.
            // fix-aisort round 7: the hidden check's broken copy `WHOOSH_LAYA_MUTANT=keep-cache` (rig only, `LayaTrace.keepCache`) skips the release
            defer { if !LayaTrace.keepCache { MLX.Memory.clearCache() } }
            let (z, parts) = try logitsTraced(state: state, question: q)
            let k = z.count
            let t = cfg.temperatureByOptions[Self.bucket(q.kind, k)] ?? cfg.temperature[q.kind.index]
            let zt = z.map { $0 / t }
            let mx = zt.max() ?? 0
            let e = zt.map { exp($0 - mx) }
            let sum = e.reduce(0, +)
            let pr = e.map { $0 / sum }
            LayaTrace.note(id: q.id, kind: q.kind.rawValue, bucket: Self.bucket(q.kind, k), bucketFound: cfg.temperatureByOptions[Self.bucket(q.kind, k)] != nil,
                           temperature: t, logits: z, probabilities: pr, parts: parts, maxLen: cfg.maxLen, headMaxLen: cfg.headMaxLen)
            switch q.kind {
            case .choice:
                let labels = q.options.map { $0.label }
                var probs: [String: Double] = [:]
                for (l, v) in zip(labels, pr) { probs[l] = v }
                let best = pr.indices.max(by: { pr[$0] < pr[$1] })!
                return LayaAnswer(id: q.id, value: .choice(label: labels[best], probabilities: probs), confidence: Confidence.fromProbabilities(pr))
            case .score:
                let value = pr.enumerated().reduce(0.0) { $0 + Double($1.offset) * $1.element }
                return LayaAnswer(id: q.id, value: .score(value: value, probabilities: pr), confidence: Confidence.fromProbabilities(pr))
            case .noul:
                return LayaAnswer(id: q.id, value: .noul(yes: pr[1]), confidence: Confidence.fromYes(pr[1]))
            }
        }
    }
}
#else
/// Built without MLX or the tokenizer package: the Laya runtime is not in this build. The root Package.swift is frozen for
/// this lane, so the lines to add are listed in docs/lanes/ondeviceai.md "Contract deltas".
public final class LayaEngine: @unchecked Sendable {
    public let checkpointName = "laya-typed-decisions"
    public static func load(folder: URL, float32: Bool = false) async throws -> LayaEngine {
        throw LocalModelError.unavailable("This build of Whoosh does not include the on-device decision runtime yet.")
    }
    public func answer(state: String, questions: [LayaQuestion]) throws -> [LayaAnswer] {
        throw LocalModelError.unavailable("This build of Whoosh does not include the on-device decision runtime yet.")
    }
}
#endif
