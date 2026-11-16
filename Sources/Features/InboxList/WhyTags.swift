import Foundation

/// The why tag on the list's rows: the switch is the label inspector's « Pourquoi ici ? » (read here, never owned), the reads are
/// the core's `message/<id>/why/<view>`, taken ONCE per load of the rows (never per row on scroll, never while the switch is off).
@MainActor
enum WhyTags {
    static var state: InboxListState { .shared }

    /// The inspector is open and its switch is on (labelscontactsmac lane: `LabelInspectorModel.whyOpen`).
    static var switchOn: Bool { LabelInspector.model.isOpen && LabelInspector.model.whyOpen }

    /// The view the list shows, as the why read names it: Primary or a label. All, a mailbox and a smart folder have no placing card.
    static func scope(_ m: AppModel) -> String? {
        guard m.mailbox == nil, m.smartFolderId == nil, m.tab != "all" else { return nil }
        return m.tab
    }

    /// One read per loaded row, once per change of the rows or of the switch. Off, or on a list with no placing card: nothing is read.
    static func refresh(_ m: AppModel) {
        guard switchOn, let view = scope(m) else {
            if !state.whyReads.isEmpty { state.whyReads = [:] }
            if state.whyScope != nil { state.whyScope = nil }   // perf-archive r4: no observed write when nothing changes (runs after every rows change)
            return
        }
        var reads: [String: WhyRead] = [:]
        for r in m.rows {
            if let w = WhyRead.parse(m.core.state(path: "message/\(r.messageId)/why/\(view)")) { reads[r.messageId] = w }
        }
        state.whyCoreReads += m.rows.count
        state.whyLoads += 1
        state.whyScope = view
        if state.whyReads != reads { state.whyReads = reads }
    }

    /// The tag this row draws, nil while there is no read for it (switch off, or the view does not hold the message).
    static func tag(_ messageId: String, _ m: AppModel) -> WhyTag? {
        guard let r = state.whyReads[messageId] else { return nil }
        return WhyTagWords.make(r, listName: { PinnedTabs.name(m.tab, m) }, accountName: { RowAccount.of($0, in: m).name })
    }

    /// The `mac-main.list.state.why_tags` probe: the switch, the view read, and how many core reads the tags cost so far.
    static func probe(_ m: AppModel) -> ControlRow {
        ["on": switchOn, "scope": state.whyScope ?? NSNull(), "shown": state.whyReads.count,
         "core_reads": state.whyCoreReads, "loads": state.whyLoads]
    }
}
