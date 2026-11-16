import Foundation

/// A folder's sync reports by event (`CoreEvent::ReplySourceProgress` on the sync stream): while Sources is on screen the pane listens, and each
/// event re-reads `reply/sources` only. No timer, no polling; the subscription ends when the pane leaves (the speed rule: free what is not shown).
@MainActor
enum RepliesEvents {
    private static var subscription: UInt64?
    private static let sink = RepliesSink()

    static func start() {
        guard subscription == nil else { return }
        subscription = RepliesStore.shared.core.subscribe(stream: .syncStatus, sink: sink)
    }

    static func stop() {
        guard let id = subscription else { return }
        RepliesStore.shared.core.unsubscribe(subscription: id)
        subscription = nil
    }
}

/// Receives the core's events on any thread; only a source's progress matters here. Nothing but the event's kind crosses to the main actor.
final class RepliesSink: EventSink, @unchecked Sendable {
    func onEvent(event: CoreEvent) {
        guard case .replySourceProgress = event else { return }
        if Thread.isMainThread {
            MainActor.assumeIsolated { RepliesStore.shared.reloadSources() }
        } else {
            Task { @MainActor in RepliesStore.shared.reloadSources() }
        }
    }
}
