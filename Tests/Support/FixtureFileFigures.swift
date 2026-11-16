import Foundation
import Testing

/// The figures the Primary list serves at the start, derived from the fixture file and never typed (the file's `flow_example` rows decide them).
/// The fixture core files every `flow_example` row whose step is "rejected" in Primary, unread (FixtureFlow.seedFlowMessagesLocked), on top of the file's
/// own messages and its `tabs` figure for Primary.
enum FixtureFileFigures {
    private static func file() throws -> [String: Any] {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let data = try Data(contentsOf: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"))
        return try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
    }
    /// The flow example's rows the flow sends to the fallback, which the Primary list shows, in the file's order.
    static func rejectedFlowIds() throws -> [String] {
        let flow = try file()["flow_example"] as? [[String: Any]] ?? []
        return flow.filter { $0["step"] as? String == "rejected" }.compactMap { $0["id"] as? String }
    }
    static func rejectedFlowRows() throws -> Int { try rejectedFlowIds().count }
    private static func flowRows(step: String) throws -> Int {
        let flow = try file()["flow_example"] as? [[String: Any]] ?? []
        return flow.filter { $0["step"] as? String == step }.count
    }
    /// The needs-reply label's flow figures (`label/needs-reply/flow/counts`) the fixture core derives from the file's `flow_example` rows, after the exit
    /// filter is given a fallback (Contract: `sorted_by_ai` = kept without an exit filter + kept by the exit filter + sent to the fallback, contracts.swift).
    /// SPEC A140: `caught_by_entry` is what step 1 PICKED: the example label's entry chain is one body rule ("Hi Assem", FixtureFlow.seedFlowExampleLocked), so it
    /// picks every `flow_example` row (each body opens so) and every corpus message whose body holds the same words. The rows whose step is "entry" (picked,
    /// ruled out by the AI) are none in the file now, and were the old meaning (the rows step 1 filed itself).
    static func flowCaughtByEntry() throws -> Int { try (file()["flow_example"] as? [[String: Any]] ?? []).count + flowWaitingForAi() }
    /// `waiting_for_ai`: the picked corpus messages the AI has not ruled on (a Re-run asks it): the file's messages whose body holds the entry chain's words.
    static func flowWaitingForAi() throws -> Int {
        let all = try #require(try file()["messages"] as? [[String: Any]])
        return all.filter { m in
            let body = (m["body"] as? [String])?.joined(separator: " ") ?? (m["body"] as? String) ?? ""
            return body.contains("Hi Assem")
        }.count
    }
    /// `kept_by_exit`: the rows whose step is "kept".
    static func flowKeptByExit() throws -> Int { try flowRows(step: "kept") }
    /// `sent_to_fallback`: the rows whose step is "rejected".
    static func flowSentToFallback() throws -> Int { try flowRows(step: "rejected") }
    /// `sorted_by_ai`: the AI's picks: the kept rows plus the rejected rows.
    static func flowSortedByAi() throws -> Int { try flowKeptByExit() + flowSentToFallback() }
    /// THE rule for the Primary list at the start, in the order a live test copy serves it: the pinned file messages, then the rejected flow rows newest
    /// first (FixtureFlow.seedFlowMessagesLocked inserts each flow row at the front of `order`, so the file's last row stands first), then the other file
    /// messages (FixtureCore.rows(_:) puts the pinned ahead and keeps every other row in place). The Python twin is journeys/lib/fixture.py ROWS.
    static func primaryIds() throws -> [String] {
        let all = try #require(try file()["messages"] as? [[String: Any]])
        let mine = all.filter { ($0["tab"] as? String ?? "primary") == "primary" && $0["outgoing"] as? Bool != true }
        let pinned = mine.filter { $0["pinned"] as? Bool == true }.compactMap { $0["id"] as? String }
        let rest = mine.filter { $0["pinned"] as? Bool != true }.compactMap { $0["id"] as? String }
        return pinned + (try rejectedFlowIds()).reversed() + rest
    }
    /// The rows Primary lists at the start.
    static func primaryRows() throws -> Int { try primaryIds().count }
    /// The Primary unread figure served at the start: the `tabs` entry's figure plus the rejected flow rows.
    static func primaryUnread() throws -> Int {
        try servedTabUnread("primary")
    }
    /// The tab's unread figure as a live test copy serves it: the file's figure, plus the flow rows seeded into that tab (the rejected rows in Primary,
    /// the kept rows in Needs Reply (A140: no row stops at the entry card any more); every flow row is unread). The Python twin is journeys/lib/fixture.py `served_unread`.
    static func servedTabUnread(_ id: String) throws -> Int {
        let tabs = try #require(try file()["tabs"] as? [[String: Any]])
        let figure = try #require(tabs.first { $0["id"] as? String == id }?["unread"] as? Int)
        let flow = try file()["flow_example"] as? [[String: Any]] ?? []
        let rejected = flow.filter { $0["step"] as? String == "rejected" }.count
        switch id {
        case "primary": return figure + rejected
        case "needs-reply": return figure + (flow.count - rejected)
        default: return figure
        }
    }
    /// The verdicts a decision preview makes: it reads every message the core holds that is not gone (FixtureCore.swift `.previewDecisions`: all of
    /// `order`, the flow rows included), at most `limit` (WhooshConstants.DECISION_PREVIEW_N).
    static func decisionPreviewCount(limit: Int) throws -> Int {
        let all = try #require(try file()["messages"] as? [[String: Any]])
        let flow = try file()["flow_example"] as? [[String: Any]] ?? []
        return min(limit, all.count + flow.count)
    }
}
