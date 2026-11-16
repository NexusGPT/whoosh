import Foundation

/// One cache entry per key: a tab plus an account filter plus a label is one inbox key.
/// Two views that ask for the same key get the same store instance.
public struct InboxKey: Hashable, Sendable {
    public var tabId: String
    public var accountId: String?
    public var labelId: String?

    public init(tabId: String, accountId: String? = nil, labelId: String? = nil) {
        self.tabId = tabId
        self.accountId = accountId
        self.labelId = labelId
    }

    public var query: InboxQuery { InboxQuery(tabId: tabId, accountId: accountId, labelId: labelId) }
}

/// Where a store stands. A refetch is `.refetching`: the rows already shown stay shown.
public enum LoadPhase: Sendable, Equatable {
    case idle
    case loading      // first read, nothing to show yet
    case refetching   // newer read in flight, prior rows still visible
}
