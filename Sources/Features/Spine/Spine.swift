import Foundation
import Observation

/// The reactive spine: one @Observable store per core stream, one instance per key, fed by the
/// core's event sink. Views read stores; every change goes through `run`, which sends a Command to
/// the core. There is no other write path: store state is `private(set)`.
/// Pattern: Point-Free swift-sharing keyed single-instance state (MIT) over Apple's Observation. No code copied.
@MainActor @Observable
public final class Spine {
    public let core: WhooshCore
    public let counts: CountsStore
    public let sync = SyncStatusStore()
    public let outbox = OutboxStore()
    /// the last command failure, in words; cleared by the next success
    public private(set) var lastFailure: ActionFailed?

    @ObservationIgnored private var inboxes: [InboxKey: InboxStore] = [:]
    @ObservationIgnored private var threads: [String: ThreadStore] = [:]
    @ObservationIgnored private var subscriptions: [UInt64] = []
    @ObservationIgnored private let sink = SpineSink()

    public init(core: WhooshCore) {
        self.core = core
        counts = CountsStore(core: core)
        sink.spine = self
        for kind in [StreamKind.inbox, .counts, .thread, .syncStatus, .outbox] {
            subscriptions.append(core.subscribe(stream: kind, sink: sink))
        }
    }

    /// The app's spine over the core chosen at launch (`--fixture-core` or the Rust core).
    public static let shared = Spine(core: CoreHost.shared.core)

    /// Ends the subscriptions. The stores keep their last rows.
    public func stop() {
        for s in subscriptions { core.unsubscribe(subscription: s) }
        subscriptions = []
    }

    // MARK: selectors (reads)

    /// The store for a key. The same key always returns the same instance; a new key starts its first read.
    public func inbox(_ key: InboxKey) -> InboxStore {
        if let s = inboxes[key] { return s }
        let s = InboxStore(key: key, core: core)
        inboxes[key] = s
        Task { await s.refetch() }
        return s
    }

    public func thread(_ id: String) -> ThreadStore {
        if let s = threads[id] { return s }
        let s = ThreadStore(threadId: id, core: core)
        threads[id] = s
        Task { await s.refetch() }
        return s
    }

    public var liveInboxKeys: [InboxKey] { Array(inboxes.keys) }

    // MARK: mutations (the only write path)

    /// Sends one command to the core, then reads again what it may have changed.
    @discardableResult
    public func run(_ command: Command) async -> CommandResult {
        let core = self.core
        do {
            let outcome = try await Task.detached { try core.run(command: command) }.value
            lastFailure = nil
            await refetchAll()
            return .done(outcome)
        } catch CoreError.Action(let failure) {
            return fail(failure)
        } catch {
            return fail(ActionFailed(accountName: "Whoosh", reasonWords: "\(error)", retryable: false))
        }
    }

    private func fail(_ failure: ActionFailed) -> CommandResult {
        lastFailure = failure
        return .refused(failure)
    }

    /// Reads every live store again. Prior rows stay visible until each new read lands.
    public func refetchAll() async {
        await counts.refetch()
        for s in Array(inboxes.values) { await s.refetch() }
        for s in Array(threads.values) { await s.refetch() }
    }

    // MARK: events (the core's push)

    func apply(_ event: CoreEvent) {
        switch event {
        case .inboxChanged(let tabId, let rows):
            // The event carries the unfiltered rows of the tab; a filtered key reads again.
            for (key, store) in inboxes where key.tabId == tabId {
                if key.accountId == nil && key.labelId == nil { store.adopt(rows) } else { Task { await store.refetch() } }
            }
        case .countsChanged(let c): counts.adopt(c)
        case .threadChanged(let t):
            // body-r1: a body downloaded in the background lands with its thread's event; the reader reads it on the redraw
            ReaderBodies.shared.forget(messageIds: t.messageIds)
            threads[t.id]?.adopt(t)
        case .syncStateChanged(let s): sync.adopt(s)
        case .outboxTick(let e): outbox.adopt(e)
        case .sendCompleted(let id, let at): outbox.completed(outboxId: id, sentAtMs: at)
        case .actionFailed(let f): lastFailure = f
        case .searchResults, .agentDelta, .proposalReady, .arrivalNotice, .agentStep, .agentConsentNeeded, .agentStopped, .detectionProgress, .mcpApprovalNeeded, .replyDraftReady, .replyDraftChanged, .replySourceProgress, .replyWorkFailed, .attachmentTransferChanged: break   // delta 20: the AI replies screens subscribe for these
        }
    }
}

/// Receives the core's events on any thread and hands them to the spine on the main actor.
final class SpineSink: EventSink, @unchecked Sendable {
    weak var spine: Spine?

    func onEvent(event: CoreEvent) {
        if Thread.isMainThread {
            MainActor.assumeIsolated { spine?.apply(event) }
        } else {
            Task { @MainActor [weak spine] in spine?.apply(event) }
        }
    }
}

/// What a command came back with: done, or refused in words.
public enum CommandResult {
    case done(CommandOutcome)
    case refused(ActionFailed)
}
