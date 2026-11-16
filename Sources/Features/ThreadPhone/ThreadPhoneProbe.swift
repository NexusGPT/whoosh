import Foundation

/// The `rows` answers: the same facts the screen draws, in the order it draws them.
@MainActor
enum ThreadPhoneProbe {
    private static func up(_ m: AppModel) -> Bool { m.overlay == ThreadPhoneState.overlayId || m.surface == ThreadPhoneState.overlayId }

    /// The thread the screen shows, or would show for the focused row (the Mac reader answers the same way at rest):
    /// the rows are readable before a tap, `iphone-thread.open` says whether the screen is up.
    private static func thread(_ m: AppModel) -> ReaderThread? { ThreadPhoneState.shared.thread(m) }

    /// The open thread's id and what the core says about its message: read flag, archived flag, still listed in the inbox.
    static func open(_ m: AppModel) -> [ControlRow] {
        guard let t = thread(m) else { return [] }
        let served = m.core.message(messageId: t.messageId)
        let row = m.rows.first { $0.messageId == t.messageId }
        return [["ax": "iphone-thread.open", "thread": t.threadId, "message": t.messageId, "surface": m.surface, "up": up(m), "count": t.messageCount,
                 "read": served?.read ?? NSNull(), "archived": served?.archived ?? NSNull(), "in_inbox": row != nil,
                 "pinned": row?.pinned ?? false, "unread": row?.unread ?? false]]
    }

    static func subject(_ m: AppModel) -> [ControlRow] {
        guard let t = thread(m) else { return [] }
        return [["ax": "iphone-thread.subject", "subject": t.subject, "thread": t.threadId]]
    }

    static func participants(_ m: AppModel) -> [ControlRow] {
        guard let t = thread(m) else { return [] }
        return [["ax": "iphone-thread.participants", "names": t.participants, "count_words": ReaderWords.count(t.messageCount)]]
    }

    static func account(_ m: AppModel) -> [ControlRow] {
        guard let t = thread(m) else { return [] }
        let a = PhoneAccounts.of(t.accountId, in: m)
        return [["ax": "iphone-thread.account", "id": a.id, "name": a.name, "address": a.address, "mark_colour": a.colour, "mark_icon": a.markIcon]]
    }

    /// Every message in reading order: who, when, folded or open, its first words, what its body is.
    static func messages(_ m: AppModel) -> [ControlRow] {
        guard let t = thread(m) else { return [] }
        let s = ThreadPhoneState.shared
        return t.entries(earlierOpen: false).map { e -> ControlRow in
            let isOpen = e.index == t.earlierCount
            let state: String
            if isOpen { state = t.shownBody(m) == nil ? "not-downloaded" : "shown" }
            else { state = (e.fact?.preview.isEmpty ?? true) ? "not-downloaded" : "preview" }
            return ["ax": "iphone-thread.messages.\(e.index)", "id": e.fact?.id ?? NSNull(), "from": e.from ?? "", "time": e.time ?? "",
                    "folded": !isOpen && !s.expanded.contains(e.index), "first_line": e.fact?.preview ?? "", "body_state": state]
        }
    }

    /// The pill: the code, its word ("Copy" / "Copied"), the clipboard this screen wrote, and the message's own flags.
    static func codePill(_ m: AppModel) -> [ControlRow] {
        guard let t = thread(m), let code = t.open.code else { return [] }
        let copied = ThreadPhoneState.shared.copiedFor == t.messageId
        let served = m.core.message(messageId: t.messageId)
        return [["ax": "iphone-thread.code-pill", "code": code, "copied": copied, "text": copied ? ThreadPhoneWords.copied : ThreadPhoneWords.copy,
                 "clipboard": ThreadClipboard.text ?? NSNull(), "read": served?.read ?? NSNull(), "archived": served?.archived ?? NSNull(),
                 "in_inbox": m.rows.contains { $0.messageId == t.messageId }]]
    }

    /// The open message's body: shown (its first words) or not downloaded (the sentence the screen says).
    static func body(_ m: AppModel) -> [ControlRow] {
        guard let t = thread(m) else { return [] }
        return [bodyRow(message: t.messageId, content: t.shownBody(m))]
    }

    /// The body row for what the core holds: no content is "not-downloaded" with the sentence the screen says.
    /// Split out so a unit test can hand it the "core holds no body" case directly (the fixture core withholds a body too, while `fixture offline on`).
    static func bodyRow(message: String, content: MessageBodyContent?) -> ControlRow {
        guard let content else {
            return ["ax": "iphone-thread.body", "message": message, "state": "not-downloaded", "text": ThreadPhoneWords.notDownloaded]
        }
        return ["ax": "iphone-thread.body", "message": message, "state": "shown", "text": String(content.plainText.prefix(ThreadPhoneProbe.textPrefix))]
    }

    /// How many characters of a shown body the row carries.
    private static let textPrefix = 160

    static func attachments(_ m: AppModel) -> [ControlRow] {
        guard let t = thread(m) else { return [] }
        return t.attachmentNames.enumerated().map { (i, name) -> ControlRow in ["ax": "iphone-thread.attachments.\(i)", "name": name] }
    }

    static func more(_ m: AppModel) -> [ControlRow] {
        guard thread(m) != nil else { return [] }
        return [["ax": "iphone-thread.more", "open": ThreadPhoneState.shared.moreOpen, "items": ["note", "unread", "pin", "snooze"]]]
    }

    static func note(_ m: AppModel) -> [ControlRow] {
        guard thread(m) != nil else { return [] }
        let s = ThreadPhoneState.shared
        return [["ax": "iphone-thread.note", "text": s.note(m)?.text ?? "", "open": s.noteOpen, "draft": s.noteDraft]]
    }
}
