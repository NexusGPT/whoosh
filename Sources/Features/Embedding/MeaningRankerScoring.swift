import Foundation

/// Scoring and the changed-text stamp of the meaning ranker (split from MeaningRanker.swift, module size).
extension MeaningRanker {
    // MARK: scoring

    /// The best messages of `pass` by cosine to `query` (embedded here, or read from the encoder's cache).
    public func score(query: String, pass p: UInt32, limit: Int? = nil) throws -> [Scored] {
        let t0 = DispatchTime.now()
        let q: [Float]
        if let hit = encoder?.cachedVector(for: query) {
            q = hit
        } else {
            q = try embedder.embedQuery(text: query)
            encoder?.remember(q, for: query)
        }
        let tQuery = Self.ms(since: t0)
        return scoreVector(q, pass: p, limit: limit, queryMs: tQuery)
    }

    /// Same, with a query vector already in hand.
    public func scoreVector(_ q: [Float], pass p: UInt32, limit: Int? = nil, queryMs: Double = 0) -> [Scored] {
        let t1 = DispatchTime.now()
        lock.lock(); defer { lock.unlock() }
        let out = store.top(query: q, pass: p, limit: limit).map { Scored(id: $0.id, score: $0.score) }
        last.embedQueryMs = queryMs
        last.searchMs = Self.ms(since: t1)
        return out
    }

    static func ms(since t: DispatchTime) -> Double { Double(DispatchTime.now().uptimeNanoseconds - t.uptimeNanoseconds) / 1e6 }

    /// Changed-text detector: the title, the byte length and 64 bytes at each end, not every byte (300,000 mails are hashed in
    /// the time of one batch). A body edited in the middle to the same length is not seen; the real core keys on its own version.
    static func stamp(title: String?, text: String) -> UInt64 {
        var h: UInt64 = 0xcbf29ce484222325
        func mix(_ b: UInt8) { h ^= UInt64(b); h = h &* 0x100000001b3 }
        for b in DocumentForm.cleanTitle(title).utf8 { mix(b) }
        mix(0x1F)
        let u = text.utf8
        var n = UInt64(u.count)
        for _ in 0 ..< 8 { mix(UInt8(n & 0xFF)); n >>= 8 }
        for b in u.prefix(64) { mix(b) }
        for b in u.suffix(64) { mix(b) }
        return h
    }
}
