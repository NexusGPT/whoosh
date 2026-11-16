import Foundation

/// The three marks of board message-card-r1 (design A) that the contract's `InboxRow` does not carry: 13 Muted, 14 VIP sender, 10 Snoozed.
/// Each comes the way the star comes (ListFilters.refreshStarred): read from the core once per change of the rows, off the key's frame
/// (ListFilters.rowsChanged), kept in InboxListState, and read by the row and the `rows` probe alike. No new field on the contract:
///   muted:   `message/<id>/muted` ("true"), served by the test engine (FixtureCore.swift `case "muted"`) and the real one (core/crates/engine/src/mute.rs `state`);
///   VIP:     the sender's address is among the rows of the VIPs view (`InboxQuery.mailbox = .vips`: FixtureLabels.swift `case .vips`,
///            core/crates/engine/src/reads.rs `Mailbox::Vips`), the view `counts().mailboxes["vips"]` counts;
///   snoozed: `snooze/<id>/return_local_time` ("HH:mm" in this Mac's time, null when the mail is not snoozed), served by FixtureCore.swift
///            (`return_local_time`) and core/crates/engine/src/message_state.rs.
@MainActor
enum RowMarks {
    static var state: InboxListState { .shared }

    /// A list of the inbox (a tab, no mailbox, label or smart folder). The core keeps a muted thread and a snoozed mail out of every inbox
    /// list (core/crates/contracts/src/command.rs `Mute`: "a muted thread leaves the inbox and stays out"; a snoozed mail leaves until its
    /// time), so no row there can carry either mark and no read is spent on them there.
    static func inboxList(_ m: AppModel) -> Bool {
        m.mailbox == nil && m.labelId == nil && m.smartFolderId == nil && state.labelFilter == nil
    }

    /// The muted ids and the snooze times among the rows, and the VIP senders. Each set is written only when it changes (perf-archive r4).
    static func refresh(_ m: AppModel) {
        let ids = m.rows.map(\.messageId)
        let inbox = inboxList(m)
        let muted = inbox ? [] : Set(ids.filter { m.core.state(path: "message/\($0)/muted") == "true" })
        var times: [String: String] = [:]
        if !inbox { for id in ids { if let t = CoreText.value("snooze/\(id)/return_local_time", in: m) { times[id] = t } } }
        let vips = vipSenders(m)
        if muted != state.muted { state.muted = muted }
        if times != state.snoozeTimes { state.snoozeTimes = times }
        if vips != state.vipSenders { state.vipSenders = vips }
    }

    /// The senders of the VIPs view's rows, lower-cased (the view holds the VIP senders' mail on both engines; an empty address is no sender).
    static func vipSenders(_ m: AppModel) -> Set<String> {
        let q = InboxQuery(tabId: m.tab, accountId: nil, labelId: nil, mailbox: .vips, accountIds: [], filters: [], smartFolderId: nil)
        return Set(m.core.inbox(query: q).map { $0.senderAddress.lowercased() }.filter { !$0.isEmpty })
    }

    /// The ids that left the rows drop from the marks at once, with no read (as the star does in rowsChanged).
    static func prune(_ live: Set<String>) {
        if !state.muted.isSubset(of: live) { state.muted.formIntersection(live) }
        if state.snoozeTimes.keys.contains(where: { !live.contains($0) }) { state.snoozeTimes = state.snoozeTimes.filter { live.contains($0.key) } }
    }

    static func muted(_ r: InboxRow) -> Bool { state.muted.contains(r.messageId) }
    static func vip(_ r: InboxRow) -> Bool { !r.senderAddress.isEmpty && state.vipSenders.contains(r.senderAddress.lowercased()) }

    /// The words the snoozed row draws in the time's place (the board's « revient 18:00 »), or nil when the mail is not snoozed.
    static func snoozeWords(_ r: InboxRow) -> String? { state.snoozeTimes[r.messageId].map { Words.text("back %@", $0) } }
}
