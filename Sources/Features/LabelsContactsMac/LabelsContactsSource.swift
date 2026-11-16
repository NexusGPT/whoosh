import Foundation
import Observation

/// One sender as the card shows them: the core's contact card and every message from any of its addresses, newest first.
struct SenderCard: Equatable, Sendable {
    var id: String
    var name: String
    var addresses: [String]
    var messages: [InboxRow]
}

/// What this screen reads that is not the core's to serve, held in ONE place. Labels come from `core.labels()`,
/// smart folders from `core.smartFolders()`, cards from `core.contact(address:)`, accounts from `AccountBook.shared`.
/// Held here only: the rule and the parent of a smart folder that already exists, because the core has no command
/// that changes a saved folder (round 3 report names the call: `updateSmartFolder(folderId, ruleWords, parentId)`).
@MainActor @Observable
final class LabelsContactsSource {
    static let shared = LabelsContactsSource()

    private var held: [String: (rule: String, parent: String?)] = [:]
    private init() {}

    nonisolated static func slug(_ s: String) -> String {
        s.lowercased().split(whereSeparator: { !$0.isLetter && !$0.isNumber }).joined(separator: "-")
    }

    func account(_ id: String) -> AccountBook.Entry? { AccountBook.shared.account(id) }

    /// Every message the core holds, across tabs and accounts.
    func allRows(_ core: WhooshCore, tabs: [String]) -> [InboxRow] {
        var seen = Set<String>()
        return tabs.flatMap { core.inbox(query: InboxQuery(tabId: $0, accountId: nil, labelId: nil)) }
            .filter { seen.insert($0.messageId).inserted }
    }

    /// The core's smart folders, with the held edits laid over the ones the owner changed.
    func folders(_ core: WhooshCore) -> [SmartFolder] {
        core.smartFolders().map { f in
            held[f.id].map { SmartFolder(id: f.id, name: f.name, ruleWords: $0.rule, parentId: $0.parent) } ?? f
        }
    }

    func hold(folderId: String, rule: String, parent: String?) { held[folderId] = (rule, parent) }
}
