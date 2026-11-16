import Foundation

/// The meaning index of the fixture core (embedding round 4, bounded in round 8): every message is embedded ONCE through the
/// contract `Embedder` (document form, its subject as the title), each query through `embedQuery`, and a message scores by cosine.
/// Vectors are unit length (MatryoshkaVector renormalises) and live as int8 in a flat `VectorStore`.
/// It holds no model of its own: the same class runs on GemmaEmbedder (the app) and on any other `Embedder` (the tests).
public final class MeaningRanker: @unchecked Sendable {
    public struct Scored: Sendable { public let id: String; public let score: Float }
    /// One message waiting to be embedded.
    public struct Row: Sendable { public let id: String; public let title: String?; public let text: String
        public init(id: String, title: String?, text: String) { self.id = id; self.title = title; self.text = text } }
    /// What the last calls cost, for docs/lanes/embedding.md and the timings file.
    public struct Timings: Sendable {
        public var embeddedMessages = 0
        public var embedMessagesMs = 0.0
        public var embedQueryMs = 0.0
        public var searchMs = 0.0
    }

    let embedder: any Embedder
    let encoder: QueryEncoder?
    let batchRows: Int
    let lock = NSLock()
    var store: VectorStore
    var pass: UInt32 = 0
    var last = Timings()

    /// With an `encoder`, `rank` reads the vector the encoder already holds for the query and stores the one it embeds,
    /// so the search box's per-keystroke vector and the ranking share one model call. `batchRows` is EMBED_BATCH: no call to the
    /// embedder carries more rows, however many are pending.
    public init(embedder: any Embedder, encoder: QueryEncoder? = nil,
                batchRows: Int = WhooshConstants.EMBED_BATCH, int8Scale: Int = WhooshConstants.EMBED_I8_SCALE) {
        self.embedder = embedder; self.encoder = encoder; self.batchRows = max(1, batchRows)
        self.store = VectorStore(dim: Int(embedder.dimension()), scale: int8Scale)
    }

    public var timings: Timings { lock.lock(); defer { lock.unlock() }; return last }
    public var indexedCount: Int { lock.lock(); defer { lock.unlock() }; return store.count }
    /// Bytes the vectors take (the figure the memory plan counts).
    public var vectorBytes: Int { lock.lock(); defer { lock.unlock() }; return store.bytes }

    /// Loads the model with one throwaway text, so the first real call does not pay for it.
    public func loadModel() { _ = try? embedder.embed(texts: ["warm"]) }

    /// What is embedded for one message: its body only, like the real core (`VectorIndex::index_message(key, body)`,
    /// core/crates/search-vector/src/index.rs:171). The subject is the text index's.
    public static func text(body: String) -> String { body }

    // MARK: passes and pending rows

    /// Starts a pass over the messages the caller holds now: a message not seen in the pass does not score (it was deleted).
    public func beginPass() -> UInt32 { lock.lock(); defer { lock.unlock() }; pass &+= 1; return pass }

    /// Marks `ids` as present in `pass` and returns those with no vector yet, in the given order. O(n), no text read.
    public func missing(in ids: [String], pass: UInt32) -> [String] {
        lock.lock(); defer { lock.unlock() }
        return ids.filter { !store.mark($0, pass: pass) }
    }

    /// Embeds `rows` in calls of at most `batchRows`, storing each batch before the next starts, so memory is one batch.
    /// A batch that fails is retried one text at a time (searchbetter Indexer.swift:312-313); a text the model refuses alone
    /// is left out. Returns how many were stored; throws the first error only when none was.
    @discardableResult
    public func embedAndStore(_ rows: [Row], pass: UInt32) throws -> Int {
        var stored = 0
        var firstError: Error?
        let t0 = DispatchTime.now()
        for start in stride(from: 0, to: rows.count, by: batchRows) {
            let batch = Array(rows[start ..< min(start + batchRows, rows.count)])
            let vectors = embedBatch(batch, error: &firstError)
            lock.lock()
            for (row, v) in zip(batch, vectors) {
                guard let v else { continue }
                store.upsert(id: row.id, stamp: Self.stamp(title: row.title, text: row.text), vector: v, pass: pass)
                stored += 1
            }
            lock.unlock()
        }
        lock.lock(); last.embeddedMessages = stored; last.embedMessagesMs = Self.ms(since: t0); lock.unlock()
        if stored == 0, !rows.isEmpty { throw firstError ?? CoreError.Unexpected(reasonWords: "the embedder returned the wrong number of vectors") }
        return stored
    }

    private func embedBatch(_ batch: [Row], error firstError: inout Error?) -> [[Float]?] {
        if let all = try? embedMany(batch), all.count == batch.count { return all }
        return batch.map { row in
            do {
                let one = try embedMany([row])
                return one.count == 1 ? one[0] : nil
            } catch {
                if firstError == nil { firstError = error }
                return nil
            }
        }
    }

    /// One call for the batch when the embedder takes a title per text; else one call per distinct title (the trait's shape).
    private func embedMany(_ batch: [Row]) throws -> [[Float]] {
        if let many = embedder as? BatchTitledEmbedder { return try many.embedDocumentBatch(batch.map { (title: $0.title, text: $0.text) }) }
        var out = [[Float]?](repeating: nil, count: batch.count)
        var groups: [String: [Int]] = [:], order: [String] = []
        for (i, r) in batch.enumerated() {
            let key = DocumentForm.cleanTitle(r.title)
            if groups[key] == nil { order.append(key); groups[key] = [] }
            groups[key]?.append(i)
        }
        for key in order {
            let idx = groups[key] ?? []
            let vs = try embedder.embedTitled(title: key == DocumentForm.noTitle ? nil : key, texts: idx.map { batch[$0].text })
            guard vs.count == idx.count else { throw CoreError.Unexpected(reasonWords: "the embedder returned the wrong number of vectors") }
            for (k, i) in idx.enumerated() { out[i] = vs[k] }
        }
        return out.compactMap { $0 }
    }
}
