import Foundation

/// Which mail the notch's reply answers. The quick reply opens on the main window's focused row, and the notch's
/// open mail may not be that row (a mail outside the inbox page): the reply is drawn, sent and keyed only when it
/// answers the open mail itself: the same thread AND the same message.
extension NotchModel {
    /// The quick reply is open and answers the mail open in the notch, never another one.
    public var replyTargetsOpen: Bool {
        guard let item = openItem else { return false }
        let qr = QuickReplyModel.shared
        return qr.isOpen && qr.threadId == Self.thread(of: item) && qr.replyMessageId == item.id
    }

    /// A fixture mail without a `thread` field is its own thread (FixtureCore `m.thread ?? id`).
    static func thread(of item: NotchItem) -> String { item.threadId.isEmpty ? item.id : item.threadId }

    /// The reply for the notch's open mail, through the quick reply's own door with the open mail's id (quickreply r6):
    /// it answers THAT message, From the account that received it, whatever row the main window's list focuses.
    /// The thread comes from the arrival (or the mail's list row); never from a core lookup by thread id, which on a
    /// thread the core does not list answers the thread the main window has open (main55: reply_to stayed on the
    /// focused mail). `moveSurface: false` keeps the main window where it is: the notch never moves it.
    @discardableResult
    func openReply(for item: NotchItem, _ m: AppModel) -> Bool {
        let row = m.rows.first { $0.messageId == item.id }
        let thread = item.threadId.isEmpty ? (row?.threadId ?? item.id) : item.threadId
        let opened = QuickReplyModel.shared.openReply(threadId: thread, messageId: item.id, replyTo: row?.senderName ?? item.sender, m, moveSurface: false)
        if opened { replyStartedFor = item.id }   // the notch started it: it ends when this mail leaves the notch (NotchModel.endOpenReply)
        return opened
    }
}
