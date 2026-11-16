// Newest-query-wins coalescing and a bounded query cache.
// Pattern from searchbetter (MIT, Copyright (c) 2026 Krishna Goutham), app/Sources/Embedder.swift:132-160 (QueryEncoder):
// while one query is being embedded, a newer one replaces the waiting one, so typing fast never queues stale work;
// finished queries are cached. Rewritten as an actor with an injected encode function; no server process, no main-thread rule.
import Foundation

/// The finished queries, readable from any thread without waiting for the actor (the synchronous `MeaningRanker.rank` reads it).
final class QueryCache: @unchecked Sendable {
    private let lock = NSLock()
    private let limit: Int
    private var map: [String: [Float]] = [:]
    init(limit: Int) { self.limit = limit }
    var count: Int { lock.lock(); defer { lock.unlock() }; return map.count }
    func get(_ q: String) -> [Float]? { lock.lock(); defer { lock.unlock() }; return map[q] }
    func put(_ q: String, _ v: [Float]) {
        lock.lock(); defer { lock.unlock() }
        if map.count >= limit { map.removeAll() }
        map[q] = v
    }
}

public actor QueryEncoder {
    private let encode: @Sendable (String) throws -> [Float]
    private let cache: QueryCache
    private var running = false
    private var waiting: (query: String, continuation: CheckedContinuation<[Float]?, Never>)?
    public private(set) var encodeCalls = 0

    /// `cacheLimit` is a ledger value (docs/CONSTANTS.md, proposed row QUERY_CACHE_ENTRIES); this file holds no number.
    public init(cacheLimit: Int, encode: @escaping @Sendable (String) throws -> [Float]) {
        self.cache = QueryCache(limit: cacheLimit)
        self.encode = encode
    }

    public var cachedCount: Int { cache.count }

    /// The cached vector for a query already embedded, read without waiting. Nil when it was not.
    public nonisolated func cachedVector(for query: String) -> [Float]? { cache.get(query) }

    /// Stores a vector embedded elsewhere, so the next `vector(for:)` and `cachedVector(for:)` find it.
    public nonisolated func remember(_ vector: [Float], for query: String) { cache.put(query, vector) }

    /// The query vector, or nil when a newer query took this one's place, or the embedder failed.
    public func vector(for query: String) async -> [Float]? {
        if let hit = cache.get(query) { return hit }
        if running {
            waiting?.continuation.resume(returning: nil)   // the older waiting query is superseded
            return await withCheckedContinuation { waiting = (query, $0) }
        }
        running = true
        encodeCalls += 1
        let encode = self.encode
        let made = await Task.detached { try? encode(query) }.value
        running = false
        if let made { cache.put(query, made) }
        if let next = waiting {
            waiting = nil
            Task { next.continuation.resume(returning: await self.vector(for: next.query)) }
        }
        return made
    }
}
