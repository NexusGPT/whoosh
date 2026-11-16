import Foundation

/// The meaning index's vectors: one flat int8 array (EMBED_DIM bytes per message, ledger "i8"), not an array of floats per
/// message. 300,000 messages at 512 values are 154 MB here; as `[Float]` per message they were about 620 MB plus per-array
/// overhead, which is what grew the first meaning search past 1 GB (round 8). Not thread-safe: the ranker locks around it.
/// A vector is unit length, so `score = sum(query_i * stored_i / scale)` is the cosine up to the int8 rounding (at most
/// 0.5/scale per value).
struct VectorStore {
    let dim: Int
    let scale: Float
    private(set) var data: [Int8] = []
    private var ids: [String] = []
    private var stamps: [UInt64] = []
    private var seen: [UInt32] = []
    private var slot: [String: Int] = [:]

    init(dim: Int, scale: Int) { self.dim = dim; self.scale = Float(scale) }

    var count: Int { ids.count }
    func contains(_ id: String) -> Bool { slot[id] != nil }
    func stamp(of id: String) -> UInt64? { slot[id].map { stamps[$0] } }
    var bytes: Int { data.count }

    /// Marks `id` as present in the current pass; returns false when it is not in the store.
    @discardableResult
    mutating func mark(_ id: String, pass: UInt32) -> Bool {
        guard let s = slot[id] else { return false }
        seen[s] = pass
        return true
    }

    mutating func upsert(id: String, stamp: UInt64, vector: [Float], pass: UInt32) {
        guard vector.count == dim else { return }
        let q = vector.map { Int8(max(-127, min(127, ($0 * scale).rounded()))) }
        if let s = slot[id] {
            data.replaceSubrange(s * dim ..< (s + 1) * dim, with: q)
            stamps[s] = stamp; seen[s] = pass
        } else {
            slot[id] = ids.count
            ids.append(id); stamps.append(stamp); seen.append(pass); data.append(contentsOf: q)
        }
    }

    /// The `limit` best messages seen in `pass`, best first (ties by id). A bounded insertion list, not a sort of everything.
    func top(query: [Float], pass: UInt32, limit: Int?) -> [(id: String, score: Float)] {
        guard query.count == dim, !ids.isEmpty else { return [] }
        var best: [(slot: Int, score: Float)] = []
        let keep = limit ?? ids.count
        data.withUnsafeBufferPointer { d in
            query.withUnsafeBufferPointer { q in
                for s in 0 ..< ids.count where seen[s] == pass {
                    var sum: Float = 0
                    let base = s * dim
                    for i in 0 ..< dim { sum += q[i] * Float(d[base + i]) }
                    let score = sum / scale
                    if limit != nil {
                        if best.count == keep, let worst = best.last, score <= worst.score { continue }
                        var at = best.count
                        while at > 0 && best[at - 1].score < score { at -= 1 }
                        best.insert((s, score), at: at)
                        if best.count > keep { best.removeLast() }
                    } else {
                        best.append((s, score))
                    }
                }
            }
        }
        if limit == nil { best.sort { $0.score != $1.score ? $0.score > $1.score : ids[$0.slot] < ids[$1.slot] } }
        return best.map { (ids[$0.slot], $0.score) }
    }
}
