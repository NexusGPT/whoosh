import SwiftUI

/// docs/MOTION.md rule 11: the selection highlight slides from the old row to the new one only when the two are at most
/// SLIDE_MAX_ROWS rows apart. Further than that, the old highlight fades out in place and the new one fades in at its
/// destination (HOVER_TINT_MS). State lands on the click's frame; the motion follows and never gates the next click.
enum SelectionMotion: String { case none, slide, fade }

struct SidebarSelection: Equatable {
    /// Selection ids: "mailbox.<slug>", "smart.<id>", "label.<path>".
    var id = "mailbox.all-inboxes"
    var motion = SelectionMotion.none
    var distance = 0
    /// Bumped on every fade: the highlight's geometry id changes, so SwiftUI removes the old one and inserts the new.
    var epoch = 0

    static func motion(distance: Int) -> SelectionMotion {
        distance <= WhooshConstants.SLIDE_MAX_ROWS ? .slide : .fade
    }

    func animation(reduce: Bool) -> Animation {
        motion == .slide && !reduce ? WhooshMotion.tabThumb : WhooshMotion.hover
    }
}

extension SidebarState {
    /// Every row the sidebar shows, top to bottom, as selection ids: the distance the rule counts is in these rows.
    func visibleOrder() -> [String] {
        let labels = SidebarData.labels()
        return sections.flatMap { s -> [String] in
            if collapsedSections.contains(s) { return [] }
            switch s {
            case "mailboxes":
                return (Self.outboxWaiting() > 0 ? [Self.outboxId] : []) + (visibleMailboxes + visibleViews).map { "mailbox.\(Self.slug($0.name))" }
            case "smart-folders": return SidebarData.smartFolders().map { "smart.\($0.id)" }
            case "labels": return labels.filter { labelVisible($0) }.map { "label.\($0.id)" }
            default: return []
            }
        }
    }

    /// Moves the highlight. The old and new rows' distance picks slide or fade; the animation is the rule's own.
    @discardableResult
    func select(_ id: String, reduce: Bool = false) -> Bool {
        let order = visibleOrder()
        guard let to = order.firstIndex(of: id) else { return false }
        let from = order.firstIndex(of: selection.id) ?? to
        var next = selection
        next.distance = abs(to - from)
        next.motion = id == selection.id ? .none : SidebarSelection.motion(distance: next.distance)
        next.id = id
        if next.motion == .fade { next.epoch += 1 }
        withAnimation(next.animation(reduce: reduce)) { selection = next }
        showInList(id)
        return true
    }

    /// The list shows what the sidebar selects: a mailbox row writes AppModel.mailbox (round 8), a smart folder AppModel.smartFolderId
    /// (round 9: the saved search's results); All inboxes clears both; a label: `LabelFilter` (list r7).
    /// Same frame as the highlight: each model field's didSet refreshes the rows (docs/MOTION.md rule 1).
    func showInList(_ id: String, model: AppModel = .shared) {
        let want = Self.mailbox(for: id)
        let folder = id.hasPrefix("smart.") ? String(id.dropFirst("smart.".count)) : nil   // round 9: the saved search's results
        // A label (list r7, A107): a pinned one selects its tab, an unpinned one filters the list, and the focus lands on the first
        // row so the reader follows. Set BEFORE the mailbox and the smart folder clear, so the rows are read once, on the right tab.
        if id.hasPrefix("label.") {
            if model.smartFolderId != nil { model.smartFolderId = nil }
            if model.mailbox != nil { model.mailbox = nil }
            let labelId = String(id.dropFirst("label.".count))
            if model.tabIds.contains(labelId) {   // a pinned label: its tab; remember the tab the list was on before the first one
                if labelTab == nil { tabBeforeLabel = model.tab }
                labelTab = labelId
            }
            LabelFilter.show(labelId, model)
            return
        }
        LabelFilter.clear(model)
        // Any other row gives the tab back (found main63: All inboxes left the list on the pinned label's tab, empty, and "Primary" never came back).
        if let back = tabBeforeLabel, let on = labelTab, model.tab == on, model.tabIds.contains(back) { model.tab = back; model.focusIndex = 0 }
        tabBeforeLabel = nil; labelTab = nil
        if model.smartFolderId != folder { model.smartFolderId = folder }
        if model.mailbox != want { model.mailbox = want }
    }

    /// Selection id to the contract's Mailbox; nil for All inboxes, a label, a smart folder or an unknown id.
    static func mailbox(for id: String) -> Mailbox? {
        switch id {
        case "mailbox.outbox": return .outbox
        case "mailbox.vips": return .vips
        case "mailbox.pinned": return .pinned
        case "mailbox.snoozed": return .snoozed
        case "mailbox.drafts": return .drafts
        case "mailbox.sent": return .sent
        case "mailbox.junk": return .junk
        case "mailbox.trash": return .trash
        case "mailbox.archive": return .archive
        case "mailbox.starred": return .starred
        case "mailbox.muted": return .muted
        case "mailbox.all-mail": return .allMail
        default: return nil
        }
    }
}

/// The highlight under the selected row. One per sidebar; a slide keeps its geometry id, a fade changes it.
struct SidebarSelectionHighlight: View {
    let ns: Namespace.ID
    let epoch: Int
    var body: some View {
        RoundedRectangle(cornerRadius: WhooshRadius.control)
            .fill(WhooshPalette.hover)
            .matchedGeometryEffect(id: "sidebar.selection.\(epoch)", in: ns)
            .transition(.opacity)
    }
}
