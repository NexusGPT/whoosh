import Foundation

/// What the sidebar shows, in the contract's own types (Account, MailLabel, SmartFolder).
/// Every fact comes from the core: `accounts()` (with group, paused, backfill), `labels()` (parentId, path,
/// depth, own unread) and `smartFolders()`.
struct SidebarAccount: Identifiable {
    let account: Account
    let unread: Int
    /// The core's health words when the account needs the owner ("needs you to sign in again").
    let problem: String?
    /// First-sync share 0...1 while the account is backfilling; nil when synced.
    let backfill: Double?
    var id: String { account.id }
}

struct SidebarLabelRow: Identifiable {
    let label: MailLabel
    /// 0 at the top level (the contract's depth is 1-based).
    let depth: Int
    let hasChildren: Bool
    /// Unread on this label alone, and with every descendant rolled up (what a folded row shows).
    var own = 0
    var total = 0
    var id: String { label.id }
}

@MainActor
enum SidebarData {
    /// LABEL_DEPTH_MAX (docs/CONSTANTS.md): the owner's "up to like 3-5 levels".
    static let depthMax = WhooshConstants.LABEL_DEPTH_MAX
    /// The groups in the order the core's accounts first name them.
    static func groups(core: WhooshCore = CoreHost.shared.core) -> [String] {
        var seen: [String] = []
        for g in core.accounts().compactMap(\.group) where !seen.contains(g) { seen.append(g) }
        return seen
    }

    static func accounts(core: WhooshCore = CoreHost.shared.core) -> [SidebarAccount] {
        core.accounts().enumerated().map { i, a in
            SidebarAccount(account: Account(id: a.id, address: a.address, provider: a.provider, displayName: AccountNames.display(a),
                                            colour: a.colour, order: UInt32(i), paused: a.paused, group: a.group, iconId: a.iconId),
                           unread: Int(a.unread), problem: a.health?.reasonWords, backfill: a.backfill)
        }
    }

    /// PaletteMac's call (PaletteMacModel.swift:25): health now comes with each account from the core, so `sync` is unused.
    static func accounts(sync: [SyncState]) -> [SidebarAccount] { accounts() }

    /// The core's flat rows (a parent before its children) as the tree the section draws; totals roll up to parents.
    static func labels(core: WhooshCore = CoreHost.shared.core) -> [SidebarLabelRow] {
        let flat = labelsWithoutViews(labels: core.labels())   // delta 16: Primary and All are views, never rows of this section
        let parents = Set(flat.compactMap(\.parentId))
        var rows = flat.map { l in
            SidebarLabelRow(label: l, depth: max(0, Int(l.depth) - 1), hasChildren: parents.contains(l.id) && Int(l.depth) < depthMax,
                            own: Int(l.unread))
        }
        for i in rows.indices { rows[i].total = rows[i].own }
        for i in rows.indices.reversed() {
            guard let p = rows[i].label.parentId, let j = rows.firstIndex(where: { $0.id == p }) else { continue }
            rows[j].total += rows[i].total
        }
        return rows
    }

    static func smartFolders(core: WhooshCore = CoreHost.shared.core) -> [SmartFolder] { core.smartFolders() }

    /// What a label row says: a built-in label's name is the app's word, in the app's language; the owner's own labels
    /// keep the name the owner typed.
    @MainActor static func title(_ l: MailLabel) -> String { l.kind == .builtIn ? Words.text(l.name) : l.name }

    /// A smart folder's count: the core's row count for the same query the list runs (`smart_folder/<id>/count`), so the figure
    /// and the list never disagree; nil while the core does not answer it.
    @MainActor static func smartFolderCount(_ id: String, model: AppModel) -> Int? {
        _ = model.rows   // re-read whenever the list's rows change (an archive inside the folder)
        return model.core.state(path: "smart_folder/\(id)/count").flatMap { Int($0) }
    }
}
