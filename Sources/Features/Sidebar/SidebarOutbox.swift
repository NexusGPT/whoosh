import Foundation

/// The Outbox row (owner 2026-11-08 09:00: "at the top the outbox just like mail"): the first row of Mailboxes while
/// something waits to leave, gone when nothing does. The count is the core's own outbox read, re-read whenever the
/// spine's outbox stream ticks (the undo bar reads the same entries); nothing here counts on its own.
extension SidebarState {
    static let outboxId = "mailbox.outbox"
    static let outboxSymbol = "tray.and.arrow.up"

    /// Entries still on their way out: undo window counting, sending, retry pending, offline, late. A scheduled entry
    /// waits in Scheduled, a sent or cancelled one is done.
    static func isWaiting(_ s: OutboxState) -> Bool {
        switch s {
        case .undoable, .sending, .failed, .late, .waitingForNetwork: return true
        case .scheduled, .sent, .cancelled: return false
        }
    }

    /// Reading `Spine.shared.outbox.entries` registers the view for the stream's ticks; the count itself comes from the core.
    @MainActor static func outboxWaiting(core: WhooshCore = CoreHost.shared.core) -> Int {
        _ = Spine.shared.outbox.entries
        return core.outbox().filter { isWaiting($0.state) }.count
    }
}
