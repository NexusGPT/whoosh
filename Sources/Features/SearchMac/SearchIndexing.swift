import Foundation

/// While accounts are still being read in, the results are partial: the screen names those accounts (fn-search-hybrid,
/// "partial results during indexing with the accounts still loading"). Read from the core's sync states: an account whose
/// history is not fetched yet and that is not failing. A failing account is the sign-in screen's to say, not search's.
@MainActor
extension SearchMacModel {
    var loadingAccounts: [(id: String, name: String)] {
        let book = AccountBook.shared.accounts()
        return app.core.syncStates().filter { !$0.historyFetched && $0.lastError == nil }.map { s in
            (s.accountId, book.first { $0.id == s.accountId }?.name ?? s.accountId)
        }
    }

    /// "Results so far: still reading 16 accounts: Work, Personal, ..." (the full list is on the `mac-search.loading-accounts` rows)
    var loadingLine: String? {
        let names = loadingAccounts.map(\.name)
        guard !names.isEmpty else { return nil }
        return names.count == 1 ? Words.text("Results so far: still reading %@", names[0])
                                : Words.text("Results so far: still reading %lld accounts: %@", names.count, names.joined(separator: ", "))
    }
}
