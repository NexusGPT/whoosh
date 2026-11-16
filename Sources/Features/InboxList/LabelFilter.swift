import SwiftUI

/// What a click on a label in the sidebar does to the list (owner 2026-11-10, docs/PRD.md A107): "when clicking on a label I should
/// have the 2 other window adjusting (right filter on window 2 and right messages)".
///  - a PINNED label is a tab of the strip (D1 "Labels are the tabs"): the click selects that tab, the list shows its rows;
///  - an UNPINNED label filters the list: the rows that carry it, read from the core (`InboxQuery.labelId`) on the All view;
///  - either way the focus lands on the first row of the new list, so the reader shows that message (the focus is what it follows).
/// Found 2026-11-10: `SidebarState.showInList` handled mailboxes and smart folders only, so a label click moved the highlight and
/// nothing else; and `AppModel.refresh()` read with `labelId: nil`, so an unpinned filter could only thin one page of the All view.
/// Now `AppModel.labelId` carries the label into the core's read (sidebar r8): the rows ARE the label's messages, and an empty
/// label leaves no row, so the reader's focus is nil and it shows its empty state.
@MainActor
enum LabelFilter {
    static var state: InboxListState { .shared }

    /// A label id the click carries ("label.<id>" without its prefix): select the tab, or filter. One read of the rows per change:
    /// `AppModel.labelId`'s didSet reads; the tab is set first so that read is on the right tab.
    static func show(_ id: String, _ m: AppModel) {
        if m.tabIds.contains(id) {
            state.labelFilter = nil
            state.labelReturnTab = nil
            m.tab = id
            m.focusIndex = 0
            if m.labelId != nil { m.labelId = nil } else { m.refresh() }
            return
        }
        if state.labelFilter == nil { state.labelReturnTab = m.tab }
        state.labelFilter = id
        m.tab = allView        // the label filter reads on the All view, which holds every inbox row
        m.focusIndex = 0
        if m.labelId != id { m.labelId = id } else { m.refresh() }   // the core's rows for the label: the first one takes the focus (index 0)
    }

    /// Any other sidebar selection: the filter goes, and the tab the click found comes back.
    static func clear(_ m: AppModel) {
        guard state.labelFilter != nil else { return }
        state.labelFilter = nil
        if let back = state.labelReturnTab, m.tab == allView { m.tab = back }
        state.labelReturnTab = nil
        m.focusIndex = 0
        if m.labelId != nil { m.labelId = nil } else { m.refresh() }
    }

    /// The All view's id (contract delta 16: Primary and All are views of the inbox).
    static let allView = "all"

    /// The label's name for the chip and the empty words, as the sidebar draws it.
    static func name(_ id: String) -> String {
        SidebarData.labels().first { $0.id == id }.map { SidebarData.title($0.label) } ?? id
    }

    static func emptyWords(_ id: String) -> EmptyStateWords {
        EmptyStateWords(title: Words.text("Nothing matches"), line: Words.text("Nothing in %@.", name(id)), keyHint: "")
    }
}

/// The chip above the rows while a label filters the list: its name and a cross; a click gives the sidebar back to All inboxes
/// (which clears the filter through `showInList`).
struct LabelFilterChip: View {
    let label: String
    var body: some View {
        Button { SidebarState.shared.select("mailbox.all-inboxes") } label: {
            HStack(spacing: WhooshSpacing.s6) {
                Image(systemName: "tag").font(WhooshTypography.caption.font(emphasized: true))
                Text(LabelFilter.name(label)).font(WhooshTypography.subhead.font(emphasized: true))
                Image(systemName: "xmark").font(WhooshTypography.caption.font(emphasized: false)).foregroundStyle(WhooshPalette.secondaryLabel)
            }
            .foregroundStyle(WhooshPalette.label)
            .padding(.horizontal, WhooshSpacing.s12).frame(height: WhooshMetrics.segmentHeight)
            .background(Capsule().fill(WhooshPalette.background))
            .whooshGlass(in: Capsule())
        }
        .buttonStyle(.plain)
        .padding(.horizontal, WhooshMetrics.listRowInset + WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s4)
        .frame(maxWidth: .infinity, alignment: .leading)
        .whooshAx("mac-main.list.labelchip")
    }
}
