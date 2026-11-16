import Foundation

/// One tab of the row: a view (Primary, All: contract delta 16) or a pinned label (delta 9). A label's name is its own,
/// built-in names through the catalogue; a view's name is the catalogue's too.
struct PinnedTab: Equatable, Sendable {
    let id: String
    let name: String
    let unread: Int
    let waiting: Int
    /// a view of the inbox, not a label: it has no label control (no menu, no pin glyph, no colour dot, no drag)
    let isView: Bool

    /// The context-menu entries of the tab, the one list the view draws and the probe reports: a label tab offers
    /// "unpin"; a view is not a label, so it offers none.
    var menu: [String] { isView ? [] : ["unpin"] }
}

/// The tab row is `inbox/tabs` (contract delta 16): the two views first (Primary, All), then the pinned labels in the
/// sidebar's order (`label.order`, tree order); VIPs is a mailbox, not a tab (docs/PRD.md "Labels: direction picked").
/// The views always show; the row shows the first TAB_PINNED_MAX labels and the rest fold into a "+N" chip.
/// Pin and unpin are the label record's `PinLabel`; a view has none. The list keeps no tab list of its own.
@MainActor
enum PinnedTabs {
    /// at most this many tabs show; the rest fold into the "+N" chip
    static var cap: Int { WhooshConstants.TAB_PINNED_MAX }

    static func all(_ m: AppModel) -> [PinnedTab] {
        let labels = Dictionary(m.core.pinnedLabels().map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
        let kinds = stripKinds(m)
        return m.tabIds.map { id in
            let c = m.counts.tabs.first { $0.tabId == id }
            // a view's name is the core's (`view/<id>/name`) through the catalogue, as a built-in label's is
            let view = kinds[id] ?? (CoreText.value("view/\(id)/name", in: m) != nil)
            let l = labels[id]
            let name = view ? Words.text(CoreText.value("view/\(id)/name", in: m) ?? id)
                : l.map { $0.kind == .builtIn ? Words.text($0.name) : $0.name } ?? Words.text(CoreText.tabName(id, in: m))
            return PinnedTab(id: id, name: name, unread: Int(c?.unread ?? 0), waiting: Int(c?.waiting ?? 0), isView: view)
        }
    }

    /// Which ids of the strip are views, read from `inbox/tabs` (`[{id,name,kind:"view"|"label",unread,waiting}]`); empty when
    /// the core serves no strip, and the per-tab `view/<id>/name` read answers instead.
    private static func stripKinds(_ m: AppModel) -> [String: Bool] {
        guard let raw = m.core.state(path: "inbox/tabs"), let data = raw.data(using: .utf8),
              let rows = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]] else { return [:] }
        var out: [String: Bool] = [:]
        for r in rows { if let id = r["id"] as? String, let kind = r["kind"] as? String { out[id] = kind == "view" } }
        return out
    }

    /// A tab's name as the tab row draws it (built-in labels in the app's language). Every other place that names a tab
    /// (the why line, the end-of-list line) reads this, so the words never disagree with the tab.
    static func name(_ id: String, _ m: AppModel) -> String {
        all(m).first { $0.id == id }?.name ?? Words.text(CoreText.tabName(id, in: m))
    }

    /// The tabs the row draws, in order, and the ones folded into "+N". The views always show; the cap counts the pinned
    /// labels. The chip reads as open while a folded tab is.
    static func split(_ tabs: [PinnedTab]) -> (shown: [PinnedTab], folded: [PinnedTab]) {
        let labels = tabs.filter { !$0.isView }
        return (tabs.filter(\.isView) + labels.prefix(cap), Array(labels.dropFirst(cap)))
    }

    /// Unpin or pin a label through its record; the row follows the core (the tab leaves or comes back at its tree place).
    static func setPinned(_ id: String, _ pinned: Bool, _ m: AppModel) -> Bool {
        guard !(all(m).first { $0.id == id }?.isView ?? false) else { return false }   // a view is not a label: nothing to pin or unpin
        guard m.perform(.pinLabel(labelId: id, pinned: pinned)) else { return false }
        if !pinned, m.tab == id, let next = m.tabIds.first { m.tab = next; m.focusIndex = 0; m.refresh() }
        return true
    }

    /// Tab and ⇧Tab: the next or previous tab, wrapping. While a mailbox is shown, the key returns to the inbox first.
    static func step(_ m: AppModel, _ delta: Int) -> Bool {
        LabelFilter.clear(m)   // a tab key ends a label's filter first (the tab it found comes back)
        guard m.mailbox != nil || m.smartFolderId != nil else { return m.switchTab(delta) }
        let ids = m.tabIds
        guard let i = ids.firstIndex(of: m.tab), !ids.isEmpty else { m.mailbox = nil; return true }
        return open(ids[(i + delta + ids.count) % ids.count], m)
    }

    /// A tab click: a tab shows the inbox, so a mailbox the sidebar chose is cleared first (AppModel.mailbox = nil), and
    /// the current tab clicked again also returns from the mailbox. One read: the tab is set before the mailbox clears.
    static func open(_ id: String, _ m: AppModel) -> Bool {
        guard let at = m.tabIds.firstIndex(of: id) else { return false }
        LabelFilter.clear(m)   // a tab click ends a label's filter
        if m.mailbox != nil || m.smartFolderId != nil {
            m.tab = id
            m.smartFolderId = nil    // a tab shows the inbox: a saved folder and a mailbox both give way
            m.mailbox = nil          // each didSet resets the focus and reads the rows once when it changes
            return true
        }
        guard let now = m.tabIds.firstIndex(of: m.tab) else { return false }
        return at == now ? true : m.switchTab(at - now)
    }
}
