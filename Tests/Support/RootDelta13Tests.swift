import Foundation
import Testing
@testable import WhooshUI

/// Probes for contracts delta 13: an outside app's search through the fixture core's gate, and the full-history fixture.
private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func core(_ clock: FixtureClock = FixtureClock()) throws -> FixtureCore {
    try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock)
}

@Test func d13AnAllowedAppSearchesOnlyAfterTheOwnersYesAndAnUngrantedOneGetsNothing() throws {
    let c = try core()
    _ = try c.run(command: .setMcpServer(on: true))
    _ = try c.run(command: .setMcpGrant(appName: "Claude Desktop", allowed: true))
    #expect(throws: CoreError.self) { try c.mcpSearch(app: "Cursor", query: "quote") }
    let id = try c.mcpSearch(app: "Claude Desktop", query: "quote")
    #expect(c.state(path: "mcp_request/\(id)/state") == "\"pending\"")
    #expect(c.state(path: "mcp_request/\(id)/results") == "[]", "nothing is searched before the yes")
    _ = try c.run(command: .answerMcpApproval(requestId: id, allowed: true))
    let subjects = try #require(c.state(path: "mcp_request/\(id)/results"))
    #expect(subjects.contains("Quote for the Q1 print run"), "the approved search returns the quote: \(subjects)")
    // a revoke made while the prompt is open still wins
    let second = try c.mcpSearch(app: "Claude Desktop", query: "quote")
    _ = try c.run(command: .setMcpGrant(appName: "Claude Desktop", allowed: false))
    _ = try c.run(command: .answerMcpApproval(requestId: second, allowed: true))
    #expect(c.state(path: "mcp_request/\(second)/state") == "\"refused\"" && c.state(path: "mcp_request/\(second)/results") == "[]")
    // a server that is off takes no search
    _ = try c.run(command: .setMcpGrant(appName: "Claude Desktop", allowed: true))
    _ = try c.run(command: .setMcpServer(on: false))
    #expect(throws: CoreError.self) { try c.mcpSearch(app: "Claude Desktop", query: "quote") }
}

@Test func d13TheFullHistoryFixtureHoldsEveryAccountsWholeHistory() throws {
    let c = try core()
    try c.loadFullHistory(accounts: 2)
    let per = WhooshConstants.FIXTURE_FULL_HISTORY_PER_ACCOUNT
    let a = try #require(c.message(messageId: "h-acc-01-0"))
    #expect(a.accountId == "acc-01" && c.message(messageId: "h-acc-02-\(per - 1)") != nil && c.message(messageId: "h-acc-03-0") == nil)
    #expect(try #require(c.message(messageId: "h-acc-01-0")).receivedAtMs > c.message(messageId: "h-acc-01-\(per - 1)")!.receivedAtMs, "newest first in time")
    #expect(throws: CoreError.self) { try c.loadFullHistory(accounts: 0) }
    #expect(throws: CoreError.self) { try c.loadFullHistory(accounts: 31) }
}

private func spec(_ name: String, meaning: String, rules: [RuleCondition]) -> LabelSpec {
    LabelSpec(name: name, parentId: nil, colour: nil, description: meaning, scope: .all, rules: RuleSet(conditions: rules, matchMode: .any), pinned: false)
}

@Test func d13ADeliveredMessageIsAnInboxRowFiledByARuleFirstThenByTheFixtureModel() throws {
    let c = try core()
    let ruled = try #require(try c.run(command: .createLabelFull(spec: spec("Suppliers", meaning: "", rules: [.domain(domain: "linea-example.com")]))).createdId)
    let meant = try #require(try c.run(command: .createLabelFull(spec: spec("Travel", meaning: "flights hotels itinerary bookings", rules: []))).createdId)
    // a rule matches: the rule files it, the meaning is never asked
    #expect(try c.deliverArrival(id: "arr-1", account: "acc-04", from: "orders@linea-example.com", sender: "Linea", subject: "Your itinerary", body: "flights") == [ruled])
    #expect(c.state(path: "message/arr-1/label/\(ruled)/decided_by") == "\"rule:rule-\(ruled)\"")
    // no rule: the fixture model files it by meaning, decided by laya
    #expect(try c.deliverArrival(id: "arr-2", account: "acc-04", from: "desk@kestrel-example.com", sender: "Kestrel", subject: "Hotels for Lisbon", body: "your itinerary") == [meant])
    #expect(c.state(path: "message/arr-2/label/\(meant)/decided_by") == "\"laya\"")
    // nothing fits: no label
    #expect(try c.deliverArrival(id: "arr-3", account: "acc-04", from: "a@b-example.com", sender: "A", subject: "Lunch?", body: "at noon") == [])
    #expect(c.message(messageId: "arr-3") != nil, "the arrival is a row")
    #expect(throws: CoreError.self) { try c.deliverArrival(id: "arr-3", account: "acc-04", from: "a@b-example.com", sender: "A", subject: "x", body: "y") }
}

@Test func d13ARerunOfADescribedLabelLetsTheFixtureModelSpeak() throws {
    let c = try core()
    let meant = try #require(try c.run(command: .createLabelFull(spec: spec("Travel", meaning: "flights hotels itinerary", rules: []))).createdId)
    _ = try c.deliverArrival(id: "arr-9", account: "acc-04", from: "desk@kestrel-example.com", sender: "Kestrel", subject: "Hotels", body: "itinerary")
    _ = try c.run(command: .updateLabel(labelId: meant, spec: spec("Travel", meaning: "invoices payments", rules: [])))
    _ = try c.run(command: .rerunLabel(labelId: meant))
    #expect(c.state(path: "message/arr-9/label/\(meant)/decided_by") == nil, "the new meaning no longer fits: the model unfiles it")
    #expect(c.state(path: "label/\(meant)/rerun/model_pending") == "false", "the fixture model spoke: nothing pending")
}

@Test func d13EveryPlacedRowHasItsDeciderInTheStoreAndTheRowReadsIt() throws {
    let c = try core()
    // the fixture's placements: msg-02 Laya, msg-03 Owner, msg-06 Jev, msg-09 a rule; msg-01 the default
    let want = ["msg-02": "\"laya\"", "msg-03": "\"owner\"", "msg-04": "\"laya\"", "msg-06": "\"jev\"", "msg-09": "\"rule:GitHub\""]
    for (id, d) in want {
        let cat = try #require(c.state(path: "message/\(id)/category")).replacingOccurrences(of: "\"", with: "")
        #expect(c.state(path: "message/\(id)/label/\(cat)/decided_by") == d, "\(id) in the store")
    }
    #expect(c.state(path: "message/msg-01/label/primary/decided_by") == nil, "a default placement writes nothing")
    let rows = c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil))
    #expect(rows.first { $0.messageId == "msg-06" }?.decidedBy == .jev && rows.first { $0.messageId == "msg-01" }?.decidedBy == nil, "the row reads the store")
    // the owner moves msg-06 to Updates: the store says owner, and so does the row (the fixture file still says Jev)
    _ = try c.run(command: .label(messageIds: ["msg-06"], labelName: "Updates", createIfMissing: false))
    let moved = c.inbox(query: InboxQuery(tabId: "updates", accountId: nil, labelId: nil)).first { $0.messageId == "msg-06" }
    #expect(c.state(path: "message/msg-06/label/updates/decided_by") == "\"owner\"" && moved?.decidedBy == .owner)
}

@Test func d13ALabelsUnreadIsItsLiveRowsOnly() throws {
    let c = try core()
    let sup = { c.labels().first { $0.name == "Suppliers" }!.unread }
    #expect(sup() == 0, "the fixture file's fixed 3 backs no message")
    let lid = try #require(c.labels().first { $0.name == "Suppliers" }).id
    _ = try c.run(command: .rerunLabel(labelId: lid))   // its rules (linea-supply domain, "quote") catch msg-01, unread
    #expect(sup() == 1)
    let listed = c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: lid)).filter(\.unread).count
    #expect(listed == 1, "the list under the label agrees with the sidebar count")
}
