import Foundation

/// Two facts a thread row draws that `InboxRow` does not carry: the count pill's number and whether the owner's own
/// message is the newest in the thread (the reply arrow before the subject; docs/design/boards/message-card-r1, pick A).
/// FINDING for the lead: `InboxRow` has no "newest is mine" field. The fact is served from existing reads, `thread(threadId:)`
/// (message ids in reading order, the newest last) and `message(messageId:)` (`isOutgoing`); a core field would save both reads.
enum ThreadFacts {
    /// The pill's number: a thread (more than one message) shows its count, a single message shows no pill.
    static func pillCount(_ threadCount: UInt32) -> Int? { threadCount > 1 ? Int(threadCount) : nil }

    /// What was read for one thread and the row it was read for: a new reply changes the row's count or time, so the stamp changes.
    struct Stamp: Equatable, Sendable { let count: Int; let atMs: Int64 }
    struct Known: Sendable { let stamp: Stamp; let mine: Bool }

    @MainActor private static var known: [String: Known] = [:]
    @MainActor private static var reading: Task<Void, Never>?

    private static func stamp(_ r: InboxRow) -> Stamp { Stamp(count: Int(r.threadCount), atMs: Int64(r.receivedAtMs)) }

    /// The ids of the thread rows whose newest message is the owner's, published to the list state. Rows already read for the same
    /// stamp cost nothing; the others are read once, together, off the main actor, so no key waits on the store (list r7, A105).
    @MainActor
    static func refresh(_ m: AppModel) {
        let rows = m.rows.filter { pillCount($0.threadCount) != nil }
        publish(rows)
        let todo = rows.filter { known[$0.threadId]?.stamp != stamp($0) }.map { (id: $0.threadId, stamp: stamp($0)) }
        guard !todo.isEmpty else { return }
        let core = m.core
        reading?.cancel()
        reading = Task { @MainActor in
            let found: [(String, Known)] = await Task.detached {
                todo.compactMap { t in
                    guard let thread = core.thread(threadId: t.id), let last = thread.messageIds.last,
                          let msg = core.message(messageId: last) else { return nil }
                    return (t.id, Known(stamp: t.stamp, mine: msg.isOutgoing))
                }
            }.value
            guard !Task.isCancelled else { return }
            for (id, k) in found { known[id] = k }
            publish(m.rows.filter { pillCount($0.threadCount) != nil })
        }
    }

    @MainActor private static func publish(_ rows: [InboxRow]) {
        let mine = Set(rows.filter { known[$0.threadId]?.mine == true }.map(\.messageId))
        if InboxListState.shared.repliedLast != mine { InboxListState.shared.repliedLast = mine }
    }
}
