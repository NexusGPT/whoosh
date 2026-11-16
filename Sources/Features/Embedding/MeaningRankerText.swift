import Foundation

/// The text-carrying API of the meaning ranker: small corpora, the tests and the old callers (split from MeaningRanker.swift).
extension MeaningRanker {
    // MARK: text-carrying API (small corpora, tests, the old callers)

    @discardableResult
    public func index(_ rows: [(id: String, text: String)]) throws -> Int {
        try index(titled: rows.map { (id: $0.id, title: nil, text: $0.text) })
    }

    /// Embeds every row whose title or text changed or is new, in bounded batches.
    @discardableResult
    public func index(titled rows: [(id: String, title: String?, text: String)]) throws -> Int {
        let p = beginPass()
        return try embedChanged(rows, pass: p)
    }

    @discardableResult
    func embedChanged(_ rows: [(id: String, title: String?, text: String)], pass p: UInt32) throws -> Int {
        lock.lock()
        let pending = rows.filter { r in
            if store.stamp(of: r.id) == Self.stamp(title: r.title, text: r.text) { store.mark(r.id, pass: p); return false }
            return true
        }
        lock.unlock()
        return try embedAndStore(pending.map { Row(id: $0.id, title: $0.title, text: $0.text) }, pass: p)
    }

    public func rank(query: String, rows: [(id: String, text: String)]) throws -> [Scored] {
        try rank(query: query, titledRows: rows.map { (id: $0.id, title: nil, text: $0.text) })
    }

    /// Embeds the rows not yet embedded (bounded batches), embeds the query in the query form, and returns every row by cosine,
    /// best first. The caller decides how many to show and from which score on (constants in docs/CONSTANTS.md).
    public func rank(query: String, titledRows rows: [(id: String, title: String?, text: String)], limit: Int? = nil) throws -> [Scored] {
        let p = beginPass()
        try embedChanged(rows, pass: p)
        return try score(query: query, pass: p, limit: limit)
    }
}
