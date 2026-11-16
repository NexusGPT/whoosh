import Foundation
import Observation

/// Tab counts. Swift never computes a count; this holds what the core last said.
@MainActor @Observable
public final class CountsStore {
    public private(set) var counts = Counts(tabs: [], mailboxes: [:])
    public private(set) var hasLoaded = false
    private let core: WhooshCore
    private let coalescer = Coalescer()

    init(core: WhooshCore) { self.core = core }

    public func unread(tab: String) -> UInt32 { counts.tabs.first { $0.tabId == tab }?.unread ?? 0 }
    public func waiting(tab: String) -> UInt32 { counts.tabs.first { $0.tabId == tab }?.waiting ?? 0 }

    public func refetch() async {
        await coalescer.run { [self] in
            let core = self.core
            let fresh = await Task.detached { core.counts() }.value
            counts = fresh
            hasLoaded = true
        }
    }

    func adopt(_ fresh: Counts) {
        coalescer.markDirty()
        counts = fresh
        hasLoaded = true
    }
}

/// One thread by id. The thread stays shown while a newer read is in flight.
@MainActor @Observable
public final class ThreadStore {
    public let threadId: String
    public private(set) var thread: MailThread?
    public private(set) var phase: LoadPhase = .idle
    private let core: WhooshCore
    private let coalescer = Coalescer()

    init(threadId: String, core: WhooshCore) {
        self.threadId = threadId
        self.core = core
    }

    public func refetch() async {
        await coalescer.run { [self] in
            phase = thread == nil ? .loading : .refetching
            let core = self.core
            let id = threadId
            let fresh = await Task.detached { core.thread(threadId: id) }.value
            if let fresh { thread = fresh }
            phase = .idle
        }
    }

    func adopt(_ fresh: MailThread) {
        coalescer.markDirty()
        thread = fresh
        phase = .idle
    }
}

/// Sync status per account, fed by the core's sync stream.
@MainActor @Observable
public final class SyncStatusStore {
    public private(set) var byAccount: [String: SyncState] = [:]
    init() {}
    public func state(account: String) -> SyncState? { byAccount[account] }
    func adopt(_ state: SyncState) { byAccount[state.accountId] = state }
}

/// Outbox entries by id, fed by the core's outbox stream; a completed send marks its entry sent.
@MainActor @Observable
public final class OutboxStore {
    public private(set) var entries: [OutboxEntry] = []
    init() {}

    func adopt(_ entry: OutboxEntry) {
        if let i = entries.firstIndex(where: { $0.id == entry.id }) { entries[i] = entry } else { entries.append(entry) }
    }

    func completed(outboxId: String, sentAtMs: Int64) {
        guard let i = entries.firstIndex(where: { $0.id == outboxId }) else { return }
        entries[i].state = .sent(atMs: sentAtMs)
    }
}
