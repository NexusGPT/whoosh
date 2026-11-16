import Foundation

/// perf-archive round 6 (item b9619482, the Mac embed latency; owner 2026-11-12: no more Swift tests): the time of each call into the
/// real meaning model (`GemmaEmbedder`), as two clock reads around the existing call, kept in a fixed ring of the last 64 per kind.
/// `measure mac-main.interaction.embed` serves them; perf/embed-latency.py judges them. A call's time includes the model's load when it
/// was not loaded yet: the script drops the first call of the run for that reason. Any thread (the embedder blocks its caller).
public enum EmbedTimes {
    public enum Kind: String { case query, document }
    /// One call: its order in the run, its kind, its milliseconds and how many texts it embedded.
    public struct Call { public let seq: Int; public let kind: Kind; public let ms: Double; public let texts: Int }

    private static let cap = 64
    nonisolated(unsafe) private static var calls: [Call] = []
    nonisolated(unsafe) private static var seq = 0
    private static let lock = NSLock()

    /// Records one finished call (two clock reads by the caller). Allocation-free once the ring is full.
    static func note(_ kind: Kind, since t0: CFAbsoluteTime, texts: Int) {
        let ms = (CFAbsoluteTimeGetCurrent() - t0) * 1000
        lock.withLock {
            seq += 1
            if calls.isEmpty { calls.reserveCapacity(cap) }
            let c = Call(seq: seq, kind: kind, ms: ms, texts: texts)
            if calls.count < cap { calls.append(c) } else { calls[(seq - 1) % cap] = c }
        }
    }

    /// The recorded calls in order, as the measure's rows.
    public static func rows() -> [[String: Any]] {
        lock.withLock { calls.sorted { $0.seq < $1.seq }.map { ["seq": $0.seq, "kind": $0.kind.rawValue, "ms": $0.ms, "texts": $0.texts] } }
    }
}
