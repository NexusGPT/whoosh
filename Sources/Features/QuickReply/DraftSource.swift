import Foundation

/// The header a draft carries beyond its words (round 5, board cards-s3 "En-tête ouvert"): To, Cc, Cci, Objet. A plain
/// reply passes `.plain` and the core fills To and the subject from the thread, as it always did.
public struct DraftFields: Equatable, Sendable {
    public var to: [String]
    public var cc: [String]
    public var bcc: [String]
    public var subject: String
    public static let plain = DraftFields(to: [], cc: [], bcc: [], subject: "")
    public init(to: [String], cc: [String], bcc: [String], subject: String) { self.to = to; self.cc = cc; self.bcc = bcc; self.subject = subject }
}

/// The thread's saved drafts as the core serves them (contract delta 11: `drafts(threadId:)`, `deleteDraft`). One port,
/// so the draft card, the list row's mark and the socket all read the same door and nothing keeps a copy.
/// Contract delta 12 doors used here, in this one file: `updateDraft(… bcc:)` (saveDraft takes no Bcc, so a Cci lands
/// by the update that follows the save).
@MainActor
public protocol DraftSource {
    func drafts(threadId: String, core: WhooshCore) -> [DraftSummary]
    /// The full text of one draft, to put back in the field.
    func body(draftId: String, core: WhooshCore) -> String?
    func save(threadId: String, accountId: String?, fields: DraftFields, body: String, core: WhooshCore) throws -> String?
    /// Saves new words into the same draft (same id, same thread, no undo entry; contract delta 11 item 8).
    func update(draftId: String, accountId: String?, fields: DraftFields, body: String, core: WhooshCore) throws
    /// Deletes; the core's undo entry (Z) brings it back with its id.
    func delete(draftId: String, core: WhooshCore) throws
}

public extension DraftSource {
    func save(threadId: String, accountId: String?, body: String, core: WhooshCore) throws -> String? {
        try save(threadId: threadId, accountId: accountId, fields: .plain, body: body, core: core)
    }
    func update(draftId: String, accountId: String?, body: String, core: WhooshCore) throws {
        try update(draftId: draftId, accountId: accountId, fields: .plain, body: body, core: core)
    }
}

public struct CoreDraftSource: DraftSource {
    public init() {}
    public func drafts(threadId: String, core: WhooshCore) -> [DraftSummary] { core.drafts(threadId: threadId) }

    public func body(draftId: String, core: WhooshCore) -> String? {
        guard let raw = core.state(path: "draft/\(draftId)/body"), let data = raw.data(using: .utf8) else { return nil }
        return try? JSONDecoder().decode(String.self, from: data)
    }

    public func save(threadId: String, accountId: String?, fields f: DraftFields, body: String, core: WhooshCore) throws -> String? {
        try core.run(command: .saveDraft(fromAccountId: accountId, to: f.to, cc: f.cc, subject: f.subject, body: body, replyToThreadId: threadId)).createdId
    }

    public func update(draftId: String, accountId: String?, fields f: DraftFields, body: String, core: WhooshCore) throws {
        _ = try core.run(command: .updateDraft(draftId: draftId, fromAccountId: accountId, to: f.to, cc: f.cc, bcc: f.bcc, subject: f.subject, body: body))
    }

    public func delete(draftId: String, core: WhooshCore) throws { _ = try core.run(command: .deleteDraft(draftId: draftId)) }
}
