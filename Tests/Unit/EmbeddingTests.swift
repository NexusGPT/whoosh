import Foundation
import Testing
@testable import WhooshUI

// The Embedder contract of docs/CAPABILITY-CONTRACT.md (trait `Embedder` in Sources/CoreBridge/Generated/contracts.swift):
// `dimension()` and `embed(texts:)`; the dimension is the ledger constant EMBED_DIM (docs/CONSTANTS.md), never a literal here.
//
// The contract is written once, as `violations(of:)`, a function that RETURNS what is wrong. Three kinds of test use it:
//   1. the contract run on a good in-test embedder (positive control: the checker can say "nothing wrong"),
//   2. the contract run on one bent embedder per rule (the checker goes red on each mutant, so each rule can fail),
//   3. the contract run on the real embedder of the embedding lane. Sources/Features/Embedding exposes none yet:
//      that test fails with the reason until the lane provides `realEmbedder()`'s source.

private struct Corpus: Decodable {
    struct Passage: Decodable { let id: String; let text: String }
    struct Embedding: Decodable {
        let query: String
        let passages: [Passage]
        let must_rank_above: [[String]]
        let long_text_repeat: String
        let long_text_times_over_chunk: Int
    }
    let embedding: Embedding
}

private func corpus() throws -> Corpus {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    return try JSONDecoder().decode(Corpus.self, from: Data(contentsOf: root.appendingPathComponent("fixtures/tests-j2.json")))
}

private func cosine(_ a: [Float], _ b: [Float]) -> Float {
    var dot: Float = 0, na: Float = 0, nb: Float = 0
    for i in 0..<min(a.count, b.count) { dot += a[i] * b[i]; na += a[i] * a[i]; nb += b[i] * b[i] }
    return (na > 0 && nb > 0) ? dot / (na.squareRoot() * nb.squareRoot()) : 0
}

/// Everything the contract says about an embedder, as words. Empty = the embedder keeps the contract.
/// `pairs` = (better, worse): the passage ids that must rank above others for the query (a real model passes all of them).
private func violations(of e: any Embedder, pairs: [[String]]) throws -> [String] {
    let c = try corpus().embedding
    var bad: [String] = []
    let dim = WhooshConstants.EMBED_DIM
    if Int(e.dimension()) != dim { bad.append("dimension() is \(e.dimension()), the ledger says EMBED_DIM = \(dim)") }
    if try e.embed(texts: []).count != 0 { bad.append("embed([]) did not return no vectors") }
    let texts = [c.query] + c.passages.map(\.text)
    let vs = try e.embed(texts: texts)
    if vs.count != texts.count { bad.append("embed returned \(vs.count) vectors for \(texts.count) texts"); return bad }
    for (i, v) in vs.enumerated() {
        if v.count != dim { bad.append("vector \(i) has \(v.count) values, expected \(dim)") }
        if v.contains(where: { !$0.isFinite }) { bad.append("vector \(i) holds a NaN or infinity") }
        if v.allSatisfy({ $0 == 0 }) { bad.append("vector \(i) is all zeros") }
    }
    if try e.embed(texts: [c.query]) != [vs[0]] { bad.append("the same text gave a different vector the second time") }
    if Set(vs.map { $0.map(\.bitPattern) }).count != vs.count { bad.append("two different texts gave the same vector") }
    let byId = Dictionary(uniqueKeysWithValues: zip(c.passages.map(\.id), vs.dropFirst()))
    for p in pairs {
        guard let better = byId[p[0]], let worse = byId[p[1]] else { bad.append("pair \(p) names an unknown passage"); continue }
        if !(cosine(vs[0], better) > cosine(vs[0], worse)) { bad.append("\(p[0]) does not rank above \(p[1]) for the query") }
    }
    // A text far over CHUNK_TOKENS: a vector of the right size or a clean error, never a crash or a wrong size.
    let long = String(repeating: c.long_text_repeat, count: c.long_text_times_over_chunk)
    if let lv = try? e.embed(texts: [long]), lv.first?.count != dim { bad.append("a text over \(WhooshConstants.CHUNK_TOKENS) tokens gave a vector of the wrong size") }
    // Empty text: a vector of the right size or a clean error.
    if let ev = try? e.embed(texts: [""]), ev.first?.count != dim { bad.append("empty text gave a vector of the wrong size") }
    return bad
}

// MARK: in-test embedders. A hashed bag of words: lexical, deterministic, enough to give the lexical pair an order.

private class BagEmbedder: Embedder, @unchecked Sendable {
    var dim: Int { WhooshConstants.EMBED_DIM }
    func dimension() -> UInt32 { UInt32(dim) }
    func embed(texts: [String]) throws -> [[Float]] { texts.map(vector) }
    func vector(_ t: String) -> [Float] {
        var v = [Float](repeating: 0, count: dim)
        for w in t.lowercased().split(whereSeparator: { !$0.isLetter && !$0.isNumber }) {
            var h: UInt64 = 1469598103934665603
            for b in w.utf8 { h = (h ^ UInt64(b)) &* 1099511628211 }
            v[Int(h % UInt64(dim))] += 1
        }
        return v
    }
}
private final class WrongDimension: BagEmbedder, @unchecked Sendable { override func dimension() -> UInt32 { UInt32(dim - 1) } }
private final class ShortRows: BagEmbedder, @unchecked Sendable { override func vector(_ t: String) -> [Float] { Array(super.vector(t).dropLast()) } }
private final class DropsLast: BagEmbedder, @unchecked Sendable { override func embed(texts: [String]) throws -> [[Float]] { Array(try super.embed(texts: texts).dropLast()) } }
private final class AllZeros: BagEmbedder, @unchecked Sendable { override func vector(_ t: String) -> [Float] { [Float](repeating: 0, count: dim) } }
private final class SameForEverything: BagEmbedder, @unchecked Sendable { override func vector(_ t: String) -> [Float] { [Float](repeating: 1, count: dim) } }
private final class NotDeterministic: BagEmbedder, @unchecked Sendable {
    nonisolated(unsafe) var n: Float = 0
    override func vector(_ t: String) -> [Float] { n += 1; var v = super.vector(t); v[0] += n; return v }
}
private final class HasNaN: BagEmbedder, @unchecked Sendable { override func vector(_ t: String) -> [Float] { var v = super.vector(t); v[0] = .nan; return v } }
private final class PhotosLookLikeTheQuery: BagEmbedder, @unchecked Sendable {
    override func vector(_ t: String) -> [Float] { t.contains("photos") ? super.vector("supplier price quote spring") : super.vector(t) }
}

private let lexicalPair = [["msg-01", "msg-s08"]]       // a bag of words can order this one; only a real model orders ["msg-s06", "msg-s08"] too

// MARK: the contract checker, proven on good and bent embedders

@Test func goodEmbedderKeepsTheContract() throws {
    #expect(try violations(of: BagEmbedder(), pairs: lexicalPair) == [])
}

@Test func theCheckerGoesRedOnEveryBentEmbedder() throws {
    let cases: [(String, any Embedder, String)] = [
        ("wrong dimension", WrongDimension(), "dimension()"),
        ("rows one short", ShortRows(), "values, expected"),
        ("one vector missing", DropsLast(), "vectors for"),
        ("all zeros", AllZeros(), "all zeros"),
        ("same vector for everything", SameForEverything(), "same vector"),
        ("not deterministic", NotDeterministic(), "second time"),
        ("NaN in a vector", HasNaN(), "NaN"),
        ("unrelated text ranks first", PhotosLookLikeTheQuery(), "rank above"),
    ]
    for (name, e, word) in cases {
        let v = try violations(of: e, pairs: lexicalPair)
        #expect(v.contains { $0.contains(word) }, "mutant '\(name)' was not caught; violations: \(v)")
    }
}

// MARK: the real embedder (the embedding lane's contract)

/// The embedding lane provides the Embedder. At this commit `Sources/Features/Embedding/EmbeddingFeature.swift` is a registration
/// stub with no embedder and no factory, so this throws. DESIGN FINDING for the lane: expose one construction point,
/// for example `EmbeddingFeature.makeEmbedder() throws -> any Embedder`, and return it here.
private struct NoEmbedderYet: Error, CustomStringConvertible {
    var description: String { "Sources/Features/Embedding provides no Embedder yet (EmbeddingFeature is a registration stub with no factory)" }
}
/// The real embedder is the lane's GemmaEmbedder over the weights in the support folder. Without the weights on this Mac the two
/// tests below are SKIPPED (not judged, never green): a unit run does not download a model.
private let gemmaWeights = GemmaWeights.inSupportFolder()
private let weightsPresent = GemmaWeights.files.allSatisfy { FileManager.default.fileExists(atPath: gemmaWeights.directory.appendingPathComponent($0).path) }
private func realEmbedder() throws -> any Embedder { GemmaEmbedder(weights: gemmaWeights, dimension: WhooshConstants.EMBED_DIM, autoSweep: false) }

@Test(.enabled(if: weightsPresent, "EmbeddingGemma weights are not in the support folder: not judged"))
func realEmbedderReturnsVectorsOfTheLedgerDimension() throws {
    let e = try realEmbedder()
    let c = try corpus().embedding
    #expect(Int(e.dimension()) == WhooshConstants.EMBED_DIM)
    let vs = try e.embed(texts: c.passages.map(\.text))
    #expect(vs.count == c.passages.count)
    for (i, v) in vs.enumerated() { #expect(v.count == WhooshConstants.EMBED_DIM, "passage \(c.passages[i].id)") }
}

@Test(.enabled(if: weightsPresent, "EmbeddingGemma weights are not in the support folder: not judged"))
func realEmbedderKeepsTheWholeContract() throws {
    let c = try corpus().embedding
    #expect(try violations(of: try realEmbedder(), pairs: c.must_rank_above) == [])
}

/// The highest physical footprint a sampler thread saw (every 20 ms) while the embedder worked.
private final class FootprintPeak: @unchecked Sendable {
    private let lock = NSLock()
    private var peakMB = 0, stop = false
    func start() {
        let t = Thread { [self] in
            while true {
                lock.lock(); let done = stop; peakMB = max(peakMB, Footprint.physicalMB() ?? 0); lock.unlock()
                if done { return }
                Thread.sleep(forTimeInterval: 0.02)
            }
        }
        t.start()
    }
    func finish() -> Int { lock.lock(); stop = true; peakMB = max(peakMB, Footprint.physicalMB() ?? 0); let p = peakMB; lock.unlock(); return p }
}

/// The memory check: while the real model embeds the fixture passages (three rounds), the process footprint stays under the ledger's
/// RSS_MODEL_MAX_MB (resident memory with the embedding model loaded for a meaning search). The figure is the whole test process
/// (runner included), so a pass is a stricter reading than the app's own. It replaces the 200 MB line written before the model was chosen,
/// which this model cannot meet (docs/lanes/gap-perf.md). BROKEN COPY: `peak < WhooshConstants.RSS_MODEL_MAX_MB` -> `peak < 1`.
@Test(.enabled(if: weightsPresent, "EmbeddingGemma weights are not in the support folder: not judged"))
func realEmbedderStaysUnderTheMemoryBudgetWhileEmbedding() throws {
    let e = try realEmbedder()
    let texts = try corpus().embedding.passages.map(\.text)
    let sampler = FootprintPeak()
    sampler.start()
    for _ in 0..<3 { _ = try e.embed(texts: texts) }
    let peak = sampler.finish()
    #expect(peak > 0, "the footprint could not be read: not a pass")
    #expect(peak < WhooshConstants.RSS_MODEL_MAX_MB, "peak footprint \(peak) MB while embedding, ledger RSS_MODEL_MAX_MB \(WhooshConstants.RSS_MODEL_MAX_MB)")
}

// MARK: a measurement for the lead (2026-11-12), not a gate: the hidden search check's six mails against its two queries

/// On main86 the hidden check's query "freelance expert who gives guidance" ranked the dinner mail (0.622) above the consultants mail
/// (0.592); on main85 the pair was first. This prints the model's own cosines for the same strings, in the mail form the engine uses
/// (`title: <subject> | text: <body>`) and with no title, so the engine's order can be compared with the model's. It asserts only that
/// the numbers were read; the lines start with `PROBE-RANK`.
@Test(.enabled(if: weightsPresent, "EmbeddingGemma weights are not in the support folder: not judged"))
func probeTheModelsOwnOrderForTheHiddenSearchChecksMails() throws {
    let e = try realEmbedder()
    let mails: [(String, String, String)] = [
        ("consultants", "Visit from the consultants", "The consultants arrive on Monday to review the plan."),
        ("advisor", "An outside advisor", "An outside advisor will look at our budget next week."),
        ("parking", "Parking permit renewal", "Your parking permit for the north car park runs out at the end of the month. Renew it at the office desk."),
        ("salmon", "Salmon recipe for Sunday", "Bake the salmon for twelve minutes with lemon and dill. Serve with boiled potatoes."),
        ("football", "Football match on Saturday", "The match starts at three. Bring the blue scarf and the tickets."),
        ("dinner", "Dinner on Friday", "Fay, a table for four on Friday at eight. Tell me if the time does not suit you."),
    ]
    var read = 0
    for query in ["freelance expert who gives guidance", "consultant"] {
        let q = try e.embedQuery(text: query)
        var titled: [(String, Float)] = [], untitled: [(String, Float)] = []
        for (name, subject, body) in mails {
            let withTitle = try e.embedTitled(title: subject, texts: [body])[0]
            let noTitle = try e.embedTitled(title: nil, texts: [body])[0]
            titled.append((name, cosine(q, withTitle)))
            untitled.append((name, cosine(q, noTitle)))
            read += 2
        }
        let show: ([(String, Float)]) -> String = { rows in rows.sorted { $0.1 > $1.1 }.map { "\($0.0)=\(String(format: "%.4f", $0.1))" }.joined(separator: " ") }
        print("PROBE-RANK query=\"\(query)\" form=subject-title: \(show(titled))")
        print("PROBE-RANK query=\"\(query)\" form=no-title: \(show(untitled))")
    }
    #expect(read == 24, "the probe read \(read) of 24 cosines")
}

// MARK: the embedder's own idle unload (item 0c272b3c, piece 1)

/// A clock the tests move by hand: nothing sleeps.
private final class IdleClock: @unchecked Sendable {
    private let lock = NSLock(); private var t: TimeInterval = 1_000
    var now: TimeInterval { lock.withLock { t } }
    func advance(_ seconds: Int) { lock.withLock { t += TimeInterval(seconds) } }
}
private final class UnloadHook: @unchecked Sendable {
    private let lock = NSLock(); private var n = 0
    func bump() { lock.withLock { n += 1 } }
    var count: Int { lock.withLock { n } }
}

/// The holder is built exactly as `GemmaEmbedder.init` builds it (GemmaEmbedder.swift:39): the ledger's idle time, the clock, no sweeping task, a hook that runs
/// after the model is dropped (the embedder clears the GPU cache there). No weights, nothing loads: the value is a number.
/// BROKEN COPY: IdleHolder.swift `now() - lastUse >= idleSeconds` -> `now() - lastUse >= 0` (it unloads before the time) or `> idleSeconds + 1_000_000` (never).
@Test func theEmbedderUnloadsItsModelAfterTheLedgerIdleTimeAndNotBefore() async throws {
    let idle = WhooshConstants.EMBED_IDLE_UNLOAD_S
    #expect(idle >= 2, "the ledger's idle time is a number of seconds this test can step around: \(idle)")
    let clock = IdleClock(), hook = UnloadHook()
    let holder = IdleHolder<Int>(idleSeconds: idle, now: { clock.now }, autoSweep: false, onUnload: { hook.bump() })
    let before = await holder.isLoaded
    #expect(before == false, "nothing loads before the first use")
    let first = try await holder.get(load: { 7 })
    #expect(first == 7)
    let loaded = await holder.isLoaded
    #expect(loaded, "the first use loads the model")
    clock.advance(idle - 1)
    let early = await holder.unloadIfIdle()
    let stillLoaded = await holder.isLoaded
    #expect(early == false && stillLoaded && hook.count == 0, "one second short of the ledger's idle time the model stays")
    _ = try await holder.get(load: { 7 })                     // a use: the idle time starts again
    clock.advance(idle - 1)
    let afterUse = await holder.unloadIfIdle()
    #expect(afterUse == false, "a use one second ago keeps the model, whatever the time since the load")
    clock.advance(1)
    let dropped = await holder.unloadIfIdle()
    let gone = await holder.isLoaded
    let unloads = await holder.unloadCount
    #expect(dropped && gone == false && unloads == 1 && hook.count == 1, "at the ledger's idle time the model goes and the memory hook runs once")
    let again = try await holder.get(load: { 8 })
    #expect(again == 8, "the next use loads it again")
    let loads = await holder.loadCount
    #expect(loads == 2)
}

/// `GemmaEmbedder.init` takes the idle time from the ledger unless a caller names one (GemmaEmbedder.swift:31), and hands it to the holder. The embedder cannot be
/// asked its idle time without loading the weights, so this reads the two lines of its source (the file is compiled only with the MLX packages).
/// BROKEN COPY: GemmaEmbedder.swift:31 `idleUnloadSeconds: Int = WhooshConstants.EMBED_IDLE_UNLOAD_S` -> `idleUnloadSeconds: Int = 3600`.
@Test func theEmbedderTakesItsIdleTimeFromTheLedgerConstantByDefault() throws {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let source = try String(contentsOf: root.appendingPathComponent("Sources/Features/Embedding/GemmaEmbedder.swift"), encoding: .utf8)
    #expect(source.contains("idleUnloadSeconds: Int = WhooshConstants.EMBED_IDLE_UNLOAD_S"), "the default is the ledger constant")
    #expect(source.contains("IdleHolder(idleSeconds: idleUnloadSeconds"), "and it is the number the holder gets")
}

/// The real embedder's own holder, with the weights (as the neighbouring tests, under the same ledger memory budget: one model, loaded once): loads on the first embed,
/// stays one second short of the ledger's idle time, goes at it. Not judged without the weights on this Mac.
@Test(.enabled(if: weightsPresent, "EmbeddingGemma weights are not in the support folder: not judged"))
func theRealEmbedderUnloadsItsOwnModelAtTheLedgerIdleTime() async throws {
    let idle = WhooshConstants.EMBED_IDLE_UNLOAD_S
    let clock = IdleClock()
    let e = GemmaEmbedder(weights: gemmaWeights, dimension: WhooshConstants.EMBED_DIM, now: { clock.now }, autoSweep: false)
    let before = await e.isModelLoaded
    #expect(before == false, "the embedder holds no model until the first embed")
    _ = try await Task.detached { try e.embed(texts: ["the model loads on this text"]) }.value      // off the cooperative pool: the trait is synchronous and blocks
    let loaded = await e.isModelLoaded
    #expect(loaded)
    clock.advance(idle - 1)
    let early = await e.unloadIfIdle()
    let stillLoaded = await e.isModelLoaded
    #expect(early == false && stillLoaded, "one second short: the model stays")
    clock.advance(1)
    let dropped = await e.unloadIfIdle()
    let gone = await e.isModelLoaded
    #expect(dropped && gone == false, "at the ledger's idle time the embedder lets its model go")
}
