import Foundation
import Observation

/// The inbox rows for one key. Rows are replaced only when newer rows arrive; a refetch never blanks them.
/// Pattern: TanStack Query keepPreviousData (MIT), re-expressed on Apple's Observation. No code copied.
@MainActor @Observable
public final class InboxStore {
    public let key: InboxKey
    public private(set) var rows: [InboxRow] = []
    public private(set) var phase: LoadPhase = .idle
    public private(set) var hasLoaded = false
    /// how many times new rows arrived; a test and a view can tell "same rows" from "new rows"
    public private(set) var arrivals = 0

    private let core: WhooshCore
    private let coalescer = Coalescer()

    init(key: InboxKey, core: WhooshCore) {
        self.key = key
        self.core = core
    }

    /// Reads the rows again off the main thread. Prior rows stay until the new rows replace them.
    public func refetch() async {
        await coalescer.run { [self] in
            phase = hasLoaded ? .refetching : .loading
            let core = self.core
            let query = key.query
            let fresh = await Task.detached { core.inbox(query: query) }.value
            land(fresh)
        }
    }

    /// Rows pushed by the core (an inboxChanged event for exactly this key).
    func adopt(_ fresh: [InboxRow]) {
        coalescer.markDirty()
        land(fresh)
    }

    private func land(_ fresh: [InboxRow]) {
        rows = fresh
        hasLoaded = true
        phase = .idle
        arrivals += 1
    }

    public func row(messageId: String) -> InboxRow? { rows.first { $0.messageId == messageId } }
}
