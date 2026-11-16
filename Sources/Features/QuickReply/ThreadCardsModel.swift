import Foundation

/// What the bottom of a conversation shows for my own mail: the outgoing sends of this thread (from the spine's outbox
/// stream, which keeps an entry after it is sent) and the actions on them. Board thread-cards B, owner pick 17:57.
@MainActor
public enum ThreadCards {
    /// A refused send carries the server's words, its code ("451 4.7.1", contract delta 12 `ActionFailed.code`) and the
    /// account's display name, for the card's reason block.
    public enum OutState: Equatable { case waiting(untilMs: Int64), sending, offline, sent(atMs: Int64), failed(words: String, code: String?, account: String) }

    public struct Outgoing: Equatable, Identifiable {
        public let id: String
        public let text: String
        public let accountId: String
        /// The Cc the send carries (an expanded reply's header, round 5; empty for a plain reply).
        public let cc: [String]
        public let state: OutState
        /// A failed send the engine tries again by itself (a connection that broke before the mail left): when the next try starts, in ms.
        /// Nil for every other state and for a failure that is final (a refusal with a code, the last try).
        public var retryAtMs: Int64? = nil
    }

    /// fix-realcore round 15: `outbox/<id>/send_state` says "retrying" and when (`next_try_ms`) for a send whose connection broke before the end of
    /// the body. The contract's state for it is still `failed`, so the card asks this path to say "Trying again in N s" and not a plain "Not sent".
    static func retryAt(_ id: String, _ core: WhooshCore) -> Int64? {
        guard let json = core.state(path: "outbox/\(id)/send_state"),
              let d = (try? JSONSerialization.jsonObject(with: Data(json.utf8))) as? [String: Any],
              d["state"] as? String == "retrying", let next = (d["next_try_ms"] as? NSNumber)?.int64Value else { return nil }
        return next
    }

    /// This thread's sends in the order they were made; a cancelled send is gone (Undo removed it).
    /// The STATE of each is the engine's own, read now (`core.outbox()`), never the stream's last copy: the stream's entry stayed `undoable`
    /// after the engine had marked the send `failed` (owner 2026-11-13 07:28, main115: "Sending in 0 s" and "Cancel Z" on a send whose entry
    /// read failed, "Connection reset by peer"). The stream still says which sends exist and in what order, and its ticks redraw the stack.
    public static func outgoing(threadId: String, core: WhooshCore = AppModel.shared.core) -> [Outgoing] {
        let live = Dictionary(core.outbox().map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        return Spine.shared.outbox.entries.compactMap { streamed -> Outgoing? in
            let e = live[streamed.id] ?? streamed
            guard e.draft.replyToThreadId == threadId else { return nil }
            let state: OutState
            var retryAt: Int64?
            switch e.state {
            case .undoable(let until): state = .waiting(untilMs: until)
            case .scheduled(let at): state = .waiting(untilMs: at)
            case .sending, .late: state = .sending
            case .waitingForNetwork: state = .offline
            case .sent(let at): state = .sent(atMs: at)
            case .failed(let failure, _):
                state = .failed(words: failure.reasonWords, code: failure.code, account: failure.accountName)
                retryAt = Self.retryAt(e.id, core)
            case .cancelled: return nil
            }
            return Outgoing(id: e.id, text: e.draft.body, accountId: e.draft.fromAccountId, cc: e.draft.cc, state: state, retryAtMs: retryAt)
        }
    }

    /// Annuler on a card: the quick reply's own send hands its text back to the field; any other send is cancelled.
    public static func undo(outboxId: String, _ m: AppModel) -> Bool {
        let quick = QuickReplyModel.shared
        if quick.outboxId == outboxId { return quick.undo(m) }
        guard (try? m.core.run(command: .cancelSend(outboxId: outboxId))) != nil else { return false }
        m.refresh()
        return true
    }

    /// The thread on screen, for the socket rows (the reader's focused message's thread).
    static func shownThread(_ m: AppModel) -> String? {
        if let t = QuickReplyModel.shared.threadId { return t }
        return m.shownRow?.threadId
    }
}
