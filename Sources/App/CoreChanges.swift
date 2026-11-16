import Foundation

/// The list re-reads itself when the core changes on its own (a sync page, an IDLE arrival, an import run, the outbox tick,
/// the write-back worker). The core's store writer emits InboxChanged / Counts / SyncStatus / Outbox events; the Spine
/// refetches its stores from them, but the Mac list draws `AppModel.rows` and `AppModel.counts`, which only
/// `AppModel.refresh()` fills. This one subscriber calls it, coalesced: a burst (a 50-message page) is one refresh per window.
/// Found 2026-11-10 (realcore --slice-bye and --slice4): the core held the messages, `mac-main.list` stayed empty until a click.
@MainActor
final class CoreChanges {
    static let shared = CoreChanges()

    /// LIST_REFRESH_COALESCE_MS (ledger line proposed in docs/lanes/shell-refresh.md): the trailing window of one refresh.
    /// The first event of a quiet period waits this long, so an arrival shows within one window plus one read, never a held-back second.
    static let coalesceMs = Int(WhooshConstants.LIST_REFRESH_COALESCE_MS)   // the ledger's number (docs/CONSTANTS.md), never a literal
    /// While the owner is mid-key (the focused row's strip not yet settled) a due refresh waits one more window, at most this many times.
    static let maxDeferrals = 3

    private let sink = ChangesSink()
    private var subscriptions: [UInt64] = []
    private var pending: Task<Void, Never>?
    private var deferrals = 0

    /// What runs when a window ends. Replaceable so a test counts the refreshes without a core.
    var refresh: @MainActor () -> Void = { AppModel.shared.refreshKeepingFocus() }
    /// true while a refresh should wait (the owner is mid-key)
    var midKey: @MainActor () -> Bool = { !InboxListState.shared.stripSettled }
    /// refreshes run so far (a probe and the unit test read it)
    private(set) var refreshCount = 0

    init() { sink.owner = self }

    /// Subscribes once to the four streams that move the list. Called from AppModel's init.
    func start(core: WhooshCore) {
        guard subscriptions.isEmpty else { return }
        for kind in [StreamKind.inbox, .counts, .syncStatus, .outbox] {
            subscriptions.append(core.subscribe(stream: kind, sink: sink))
        }
    }

    /// One event arrived (main actor). Starts the window if none is open; events inside it add nothing.
    func note() {
        guard pending == nil else { return }
        deferrals = 0
        arm()
    }

    private func arm() {
        pending = Task { @MainActor [weak self] in
            try? await Task.sleep(for: .milliseconds(CoreChanges.coalesceMs))
            guard !Task.isCancelled, let self else { return }
            if self.midKey(), self.deferrals < CoreChanges.maxDeferrals {
                self.deferrals += 1
                self.arm()
                return
            }
            self.pending = nil
            self.refreshCount += 1
            self.refresh()
        }
    }

    /// The sink's entry: any thread.
    nonisolated static func relevant(_ event: CoreEvent) -> Bool {
        switch event {
        case .inboxChanged, .countsChanged, .syncStateChanged, .outboxTick, .sendCompleted: return true
        default: return false
        }
    }
}

extension AppModel {
    /// `refresh()` that keeps the focus on the same message when rows arrive above it (the index alone would slide onto another row).
    func refreshKeepingFocus() {
        let kept = focusedMessageId
        refresh()
        if let kept, focusedMessageId != kept { focus(messageId: kept) }
    }
}

/// Receives the core's events on any thread and hands the relevant ones to `CoreChanges` on the main actor.
final class ChangesSink: EventSink, @unchecked Sendable {
    weak var owner: CoreChanges?

    func onEvent(event: CoreEvent) {
        guard CoreChanges.relevant(event) else { return }
        if Thread.isMainThread {
            MainActor.assumeIsolated { owner?.note() }
        } else {
            Task { @MainActor [weak owner] in owner?.note() }
        }
    }
}
