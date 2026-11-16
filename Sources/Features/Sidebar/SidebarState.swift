import Foundation
import Observation

/// The sidebar's own UI state: which groups are open, which inboxes the Mailboxes picker ticked, what the owner
/// hid, the section order and the light variant. Data stays in the core (AppModel.accountFilter is the filter).
@MainActor @Observable
final class SidebarState {
    static let shared = SidebarState()

    /// The fixed mailbox list and its SF Symbols (board `mbIcon`).
    static let mailboxes: [(name: String, symbol: String)] = [
        ("All inboxes", "tray"), ("VIPs", "star"), ("Pinned", "pin"), ("Snoozed", "clock"), ("Drafts", "doc"), ("Sent", "paperplane"),
        ("Junk", "xmark.bin"), ("Trash", "trash"), ("Archive", "archivebox"),
    ]
    /// The contract's built-in views (delta 10: Mailbox.starred, .muted, .allMail; G then S, M, A), drawn under the mailboxes.
    /// They keep their own socket region (`rows mac-main.sidebar.views`), so the mailbox list stays the fixture's.
    static let views: [(name: String, symbol: String)] = [("Starred", "star.circle"), ("Muted", "bell.slash"), ("All mail", "tray.2")]
    static let defaultSections = ["mailboxes", "smart-folders", "labels", "accounts"]

    var openGroups: Set<String> = ["Work"]
    var openLabels: Set<String> = []
    var pickerOpen = false
    /// Accounts the owner unchecked in the Mailboxes picker (none = every account checked, the default), and the filter this wrote.
    var unchecked: Set<String> = []
    var appliedFilter: String?
    var hidden: Set<String> = []
    var lightSidebar = false
    var sections = SidebarState.defaultSections
    /// The account whose sign-in line shows after its ring was clicked.
    var problemShown: String?
    /// The accounts menu at the sidebar foot: false = the one-line bar, true = the panel pulled up over the sidebar.
    var accountsMenuOpen = false
    /// True while the pulled-up panel holds more accounts below its edge (the edge then fades).
    var menuMoreBelow = false
    /// The highlighted row and how the highlight got there (docs/MOTION.md rule 11; SidebarSelection.swift).
    var selection = SidebarSelection()
    /// The label row the arrow keys act on (right expands, left collapses); set by a click on a label.
    var labelFocus: String?
    /// A pinned label is a tab (list r7): the sidebar remembers the tab it left and the label tab it put the list on, so All inboxes
    /// (or any other row) gives the list back to the tab it came from instead of leaving it on the label's tab (SidebarSelection.showInList).
    var tabBeforeLabel: String?
    var labelTab: String?
    /// Round 5: the text prompt (new label, rename, new smart folder), a delete waiting for its yes, the header whose "+" shows,
    /// and a counter the views read so a core write redraws the sections.
    var prompt: SidebarPrompt?
    var promptText = ""
    var promptRule = ""
    var deleteAsk: SidebarDeleteAsk?
    var hoverHeader: String?
    var revision = 0
    /// Moves when the label rows are due a redraw because the mail changed (`mailChanged`). Only the Labels section reads it: the rest of the sidebar
    /// follows the counts it observes.
    var labelsRevision = 0
    /// The redraw `mailChanged` has asked for and not yet done; a newer change replaces it.
    @ObservationIgnored var mailRedraw: DispatchWorkItem?
    /// The count each label row drew at its last draw (label id to count). A check reads it beside the core's own figure: the two differ when the
    /// row was not drawn again after the mail changed.
    @ObservationIgnored var drawnLabelCounts: [String: Int] = [:]
    /// Round 6: the sections folded from their chevron and whether All inboxes shows one row per account (both remembered, SidebarStructure.swift),
    /// the label whose pin the drive forces visible, the drag in flight and where each label row sits (for the drop).
    var collapsedSections: Set<String> = Set(UserDefaults.standard.stringArray(forKey: "whoosh.sidebar.collapsedSections") ?? [])
    var allInboxesOpen = UserDefaults.standard.bool(forKey: "whoosh.sidebar.allInboxesOpen")
    var hoverLabel: String?
    /// Round 11: the last refusal shown in the contract's words (a view treated as a label, SidebarViews.swift).
    var refusal: SidebarRefusal?
    var drag: SidebarLabelDragState?
    @ObservationIgnored var labelFrames: [String: CGRect] = [:]

    static func slug(_ s: String) -> String { s.lowercased().replacingOccurrences(of: " ", with: "-") }

    var visibleMailboxes: [(name: String, symbol: String)] {
        Self.mailboxes.filter { !hidden.contains(Self.slug($0.name)) }
    }
    var visibleViews: [(name: String, symbol: String)] {
        Self.views.filter { !hidden.contains(Self.slug($0.name)) }
    }

    func toggleGroup(_ g: String) { if openGroups.contains(g) { openGroups.remove(g) } else { openGroups.insert(g) } }
    func toggleLabel(_ id: String) { if openLabels.contains(id) { openLabels.remove(id) } else { openLabels.insert(id) } }

    /// A label shows when every ancestor is open (the core's parentId chain).
    func labelVisible(_ row: SidebarLabelRow, in all: [SidebarLabelRow] = []) -> Bool {
        let rows = all.isEmpty ? SidebarData.labels() : all
        let byId = Dictionary(rows.map { ($0.id, $0.label) }, uniquingKeysWith: { a, _ in a })
        let filter = AppModel.shared.accountFilter
        // An account's own label shows only under that account's filter; a shared label always (every ancestor too).
        guard Self.scopeShows(row.label, filter: filter) else { return false }
        var parent = row.label.parentId
        while let p = parent {
            guard openLabels.contains(p) else { return false }
            if let up = byId[p], !Self.scopeShows(up, filter: filter) { return false }
            parent = byId[p]?.parentId
        }
        return true
    }

    /// One click filters the list to that account (every other account unchecked); a second click on it checks them all again.
    func clickAccount(_ a: SidebarAccount, model: AppModel) {
        problemShown = a.problem != nil ? a.id : nil
        if model.accountFilter == a.id { checkAll(model: model) }
        else { apply(unchecked: Set(SidebarData.accounts(core: model.core).map(\.id)).subtracting([a.id]), model: model) }
    }

    /// The Mailboxes picker: every account starts CHECKED ("All" means all selected). Unchecking one turns "All" off; "All" checks every
    /// account again. The list query takes one account (InboxQuery.accountId): the list follows only while exactly one account stays
    /// checked; several checked with some unchecked is a reported gap (docs/lanes/sidebar.md "## Round 6").
    @discardableResult func togglePick(_ id: String, model: AppModel) -> Bool {
        let ids = Set(SidebarData.accounts(core: model.core).map(\.id))
        var next = uncheckedNow(model)
        if next.contains(id) { next.remove(id) } else {
            guard ids.subtracting(next).subtracting([id]).isEmpty == false else { return false }   // the last checked account stays checked
            next.insert(id)
        }
        apply(unchecked: next, model: model)
        return true
    }

    func checkAll(model: AppModel) { problemShown = nil; apply(unchecked: [], model: model) }
    func clearAccounts(model: AppModel) { checkAll(model: model) }

    private func apply(unchecked next: Set<String>, model: AppModel) {
        let ids = SidebarData.accounts(core: model.core).map(\.id)
        let checked = ids.filter { !next.contains($0) }
        unchecked = next
        model.accountFilter = checked.count == 1 && !next.isEmpty ? checked.first : nil
        appliedFilter = model.accountFilter
        model.refresh()
    }

    /// The unchecked accounts as the list sees them: a filter set elsewhere (the list's own chip) wins over what was last written here.
    func uncheckedNow(_ model: AppModel) -> Set<String> {
        let ids = SidebarData.accounts(core: model.core).map(\.id)
        if model.accountFilter != appliedFilter { return model.accountFilter.map { Set(ids).subtracting([$0]) } ?? [] }
        return unchecked
    }
    func isChecked(_ id: String, model: AppModel) -> Bool { !uncheckedNow(model).contains(id) }

    func moveUp(_ section: String) -> Bool {
        guard let i = sections.firstIndex(of: section), i > 0 else { return false }
        sections.swapAt(i, i - 1)
        return true
    }
}
