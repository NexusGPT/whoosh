import Foundation
import Observation

/// The AI reply in the thread (contracts delta 20, docs/lanes/replies-contract.md rows THREAD; boards ai-replies-r4 design 1
/// and ai-replies-r5 "On a thread"). It lives in the quick reply, never in a panel: ⌘J writes a draft (one "Consigne" field
/// first when Settings > Réponses > Sur demande asks for it), the draft lands by event with the AI mark, one field under it
/// asks for a change. The state flips on the key frame; the draft itself only ever arrives by `ReplyDraftReady` /
/// `ReplyDraftChanged`, so no key waits for a model. Nothing here sends: a draft goes out only through the slab's own send.
@MainActor @Observable
public final class QuickReplyAIState {
    public enum Phase: Equatable {
        case idle
        /// The "Consigne" field is up, waiting for Return (Sur demande > "Donner une consigne avant" on).
        case asking
        /// The command went out; the draft has not landed yet (the quiet "writing" state). `rewrite`: a change asked.
        case writing(rewrite: Bool)
        /// An AI draft is in the field with its mark.
        case ready
        /// The core refused the command: its words, shown under the field; ⌘J tries again.
        case failed(words: String)
    }

    /// Which field takes the socket's `type` and the keyboard: the reply itself, the instruction, or the change.
    public enum Focus: String { case reply, instruction, change }

    public internal(set) var phase: Phase = .idle
    /// The AI draft the field shows, as the core last told it (nil once accepted, discarded or replaced by the owner's words).
    public internal(set) var draft: ReplyDraft?
    public var instruction = ""
    public var change = ""
    public var focus: Focus = .reply
    /// The owner's own words when ⌘J was pressed: they fill the instruction field, and Escape or discard brings them back.
    var held = ""
    /// The field's text when a write went out: the landing draft replaces it, and only it (words typed since win).
    var replaceable: String?
    /// The two Sur demande switches, read from `reply/on_demand` when ⌘J is pressed and when a draft lands.
    public internal(set) var options = OnDemandOptions(instructionBefore: true, changeAfter: true)
    /// Counts every write sent (⌘J, a change): the check reads that the writing state was entered even when the
    /// fixture answers inside the same main-actor turn.
    public internal(set) var writes = 0
    /// Moves when a draft lands or is replaced: the field's landing light keys on it (one fade, no timer).
    public internal(set) var landed = 0
    @ObservationIgnored private var subscription: UInt64?
    @ObservationIgnored private let sink = QuickReplyAISink()

    public var isWriting: Bool { if case .writing = phase { return true }; return false }
    public var shown: Bool { draft != nil }
    var failureWords: String? { if case .failed(let w) = phase { return w }; return nil }
    /// The change field shows under a draft the AI wrote, while Sur demande > "Demander un changement après" is on.
    public var changeFieldShown: Bool { draft != nil && options.changeAfter && phase != .asking }

    /// Listens to the thread stream once (the fixture core carries the reply events there).
    func start(_ owner: QuickReplyModel, core: WhooshCore) {
        guard subscription == nil else { return }
        sink.owner = owner
        subscription = core.subscribe(stream: .thread, sink: sink)
    }

    func clear() {
        phase = .idle; draft = nil; instruction = ""; change = ""; focus = .reply; held = ""; replaceable = nil
    }

    func show(_ d: ReplyDraft) {
        draft = d
        phase = .ready
        change = ""
        if focus == .instruction { focus = .reply }
        landed += 1
    }

    /// `reply/on_demand`; the defaults (both on, the contract's `default_on_demand_options`) when the core serves nothing.
    func readOptions(_ core: WhooshCore) {
        guard let json = core.state(path: "reply/on_demand"), let o = Self.object(json) else { return }
        options = OnDemandOptions(instructionBefore: o["instruction_before"] as? Bool ?? true, changeAfter: o["change_after"] as? Bool ?? true)
    }

    /// The thread's LIVE draft (`reply/draft/<thread id>`, the contract's `reply_draft_json`), nil when there is none.
    static func liveDraft(threadId: String, core: WhooshCore) -> ReplyDraft? {
        guard let json = core.state(path: "reply/draft/\(threadId)"), let o = object(json) else { return nil }
        return decode(o)
    }

    static func decode(_ o: [String: Any]) -> ReplyDraft? {
        guard let id = o["id"] as? String, let thread = o["thread_id"] as? String, let text = o["text"] as? String,
              let state = (o["state"] as? String).flatMap({ replyDraftStateFromKey(key: $0) }) else { return nil }
        let madeBy = (o["made_by"] as? String).flatMap { replyMadeByFromKey(key: $0) } ?? .onDemand
        let number = { (k: String) -> Int64 in (o[k] as? NSNumber)?.int64Value ?? 0 }
        return ReplyDraft(id: id, threadId: thread, messageId: o["message_id"] as? String ?? "", text: text, madeBy: madeBy,
                          excerptIds: o["excerpt_ids"] as? [String] ?? [], instruction: o["instruction"] as? String, state: state,
                          revision: UInt32(clamping: number("revision")), createdAtMs: number("created_at_ms"), updatedAtMs: number("updated_at_ms"))
    }

    private static func object(_ json: String) -> [String: Any]? {
        (try? JSONSerialization.jsonObject(with: Data(json.utf8), options: [.fragmentsAllowed])) as? [String: Any]
    }

    /// A draft the field may show: made and not touched. Edited (taken into the composer), sent and discarded never show.
    static func isProposed(_ d: ReplyDraft) -> Bool { d.state == .proposed }
}

/// One reply event, taken out of the core's event as Sendable values before it crosses to the main actor.
enum QuickReplyAIEvent: Sendable {
    case ready(threadId: String, draftId: String)
    case changed(threadId: String, draft: ReplyDraft)

    init?(_ e: CoreEvent) {
        switch e {
        case .replyDraftReady(let t, let id): self = .ready(threadId: t, draftId: id)
        case .replyDraftChanged(let t, let d): self = .changed(threadId: t, draft: d)
        default: return nil
        }
    }
}

/// Receives the thread stream on any thread and hands the reply events to the quick reply on the main actor. Only the
/// Sendable event value crosses (the Swift 6 rule of the brief: no Notification, no nested function in the closure).
final class QuickReplyAISink: EventSink, @unchecked Sendable {
    weak var owner: QuickReplyModel?

    func onEvent(event: CoreEvent) {
        guard let e = QuickReplyAIEvent(event) else { return }
        if Thread.isMainThread {
            MainActor.assumeIsolated { owner?.aiReceive(e) }
        } else {
            Task { @MainActor [weak owner] in owner?.aiReceive(e) }
        }
    }
}
