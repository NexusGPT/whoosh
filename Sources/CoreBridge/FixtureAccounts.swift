import Foundation

/// Delta 11, item 2: rename, pause, remove an account, and its signature, on the fixture core's in-memory copy.
/// The fields live on FixtureCore (`accountNames`, `pausedAccounts`, `removedAccounts`, `accountSignatures`); these are
/// called with the core's lock held, from `run(command:)`.
extension FixtureCore {
    func renameAccountLocked(_ id: String, _ name: String) throws {
        guard accountIdsLocked().contains(id), !removedAccounts.contains(id) else { throw CoreError.NotFound(kind: "account", id: id) }
        let n = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !n.isEmpty else { throw CoreError.Invalid(field: "name", reasonWords: "an account needs a name") }
        accountNames[id] = n
    }

    func pauseAccountLocked(_ id: String, _ paused: Bool) throws {
        guard accountIdsLocked().contains(id), !removedAccounts.contains(id) else { throw CoreError.NotFound(kind: "account", id: id) }
        if paused { pausedAccounts.insert(id) } else { pausedAccounts.remove(id) }
    }

    func setSignatureLocked(_ id: String, _ text: String) throws {
        guard accountIdsLocked().contains(id), !removedAccounts.contains(id) else { throw CoreError.NotFound(kind: "account", id: id) }
        let t = text.trimmingCharacters(in: .whitespacesAndNewlines)
        accountSignatures[id] = t.isEmpty ? nil : t
    }

    /// What removing the account takes off this Mac: its messages, its drafts, what waits to send from it, its own labels.
    func accountRemovalLocked(_ id: String) -> AccountRemoval? {
        guard accountIdsLocked().contains(id), !removedAccounts.contains(id) else { return nil }
        let name = accountNames[id] ?? fx.accounts.first { $0.id == id }?.name ?? added.first { $0.id == id }?.address ?? id
        let msgs = UInt32(order.filter { (account[$0] ?? fxMessage($0)?.account) == id && !deleted.contains($0) }.count)
        let drafts = UInt32(draftRows.values.filter { $0.account == id }.count)
        let outbox = UInt32(waitingEntriesLocked().filter { $0.draft.fromAccountId == id }.count)
        let labels = UInt32(labelRows.filter { if case .account(let a) = $0.scope { return a == id } else { return false } }.count)
        return AccountRemoval(accountId: id, accountName: name, messages: msgs, drafts: drafts, outbox: outbox, labels: labels,
                              words: accountRemovalWords(accountName: name, messages: msgs, drafts: drafts, outbox: outbox, labels: labels))
    }

    func removeAccountLocked(_ id: String) throws -> AccountRemoval {
        guard let r = accountRemovalLocked(id) else { throw CoreError.NotFound(kind: "account", id: id) }
        removedAccounts.insert(id)
        for m in order where (account[m] ?? fxMessage(m)?.account) == id { deleted.insert(m) }
        draftRows = draftRows.filter { $0.value.account != id }
        entries.removeAll { $0.draft.fromAccountId == id && { if case .sent = $0.state { return false } else { return true } }($0) }
        labelRows.removeAll { if case .account(let a) = $0.scope { return a == id } else { return false } }
        lastRemoval = r
        return r
    }

    public func accountRemoval(accountId: String) -> AccountRemoval? { lock.withLock { accountRemovalLocked(accountId) } }
}
