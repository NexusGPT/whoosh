import Foundation

/// The door for a surface that shows ONE mail outside the list (the notch): the reply opens for that thread, From
/// the account that received it, whatever row the main window's list has focused. R keeps `open(_:)`, the focused
/// row. `moveSurface: false` opens the reply without switching the main window's surface to the reader (the notch
/// never moves the main window). Socket: `click mac-main.quickreply.open.<threadId>` (moves) and `…open.<threadId>.stay`.
extension QuickReplyModel {
    @discardableResult
    public func open(threadId: String, _ m: AppModel, moveSurface: Bool = true) -> Bool {
        if let row = m.rows.first(where: { $0.threadId == threadId }) {
            return openReply(threadId: threadId, messageId: row.messageId, replyTo: row.senderName, m, moveSurface: moveSurface)
        }
        // Not in the list's current rows (another tab or account): the core's thread names its newest message.
        guard let t = m.core.thread(threadId: threadId) else { return false }
        let id = t.messageIds.last ?? threadId
        let sender = m.core.message(messageId: id)?.senderName
            ?? t.participants.first.map { $0.components(separatedBy: " <").first ?? $0 } ?? ""
        return openReply(threadId: threadId, messageId: id, replyTo: sender, m, moveSurface: moveSurface)
    }
}

extension QuickReplyIds {
    public static func openThread(_ threadId: String) -> String { "mac-main.quickreply.open.\(threadId)" }
    public static func openThreadStay(_ threadId: String) -> String { "mac-main.quickreply.open.\(threadId).stay" }
}
