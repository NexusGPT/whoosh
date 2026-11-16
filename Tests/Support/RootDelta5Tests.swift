import Foundation
import Testing
@testable import WhooshUI

/// Probes for root delta 5: account group / paused / backfill, smartFolders(), label unread counts.
private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func core() throws -> FixtureCore {
    try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}

@Test func accountsCarryGroupPausedAndBackfill() throws {
    let a = try core().accounts()
    #expect(a.count == 30)
    #expect(a.first { $0.id == "acc-01" }?.group == "Work")
    #expect(a.allSatisfy { !$0.paused })
    #expect(a.allSatisfy { ($0.backfill ?? -1) >= 0 && ($0.backfill ?? 2) <= 1 })
    #expect(a.filter { $0.backfill == 1.0 }.count == 29)
    #expect(a.first { $0.id == "acc-27" }?.backfill == 0.47)
}

@Test func anAddedAccountHasNoBackfillYet() throws {
    let c = try core()
    _ = try c.run(command: .addAccount(address: "new@example.org", provider: .imap))
    let added = c.accounts().last
    #expect(added?.id == "acc-31" && added?.backfill == nil && added?.paused == false && added?.group == nil)
}

@Test func smartFoldersComeFromTheFixture() throws {
    let f = try core().smartFolders()
    #expect(f.map(\.name) == ["Supplier quotes", "Waiting on me", "Travel this month"])
    #expect(f.map(\.id) == ["sf-1", "sf-2", "sf-3"])
    #expect(f.allSatisfy { !$0.ruleWords.isEmpty && $0.parentId == nil })
}

@Test func labelsCarryTheirOwnUnreadAndADepthFiveBranch() throws {
    // delta 13: a label's unread is the live rows filed under it; the fixture file's fixed numbers (Nordlicht 4, Q4 5...)
    // backed no message and are gone, so at load every owner label reads 0
    let c = try core()
    let l = c.labels()
    func row(_ path: String, _ rows: [MailLabel]) -> MailLabel? { rows.first { $0.path == path } }
    #expect(l.filter { $0.kind == .owner }.allSatisfy { $0.unread == 0 })
    let q4 = row("Work > Clients > Provanta > RFP 2026 > Q4", l)
    #expect(q4?.depth == 5)
    #expect(row("Work > Clients > Provanta > RFP 2026", l)?.depth == 4)
    // one unread message filed under Q4: Q4 reads 1, and the rollup over Provanta is own + children, counted once
    let id = try #require(c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).first { $0.unread }).messageId
    _ = try c.run(command: .label(messageIds: [id], labelName: "Work > Clients > Provanta > RFP 2026 > Q4", createIfMissing: false))
    let after = c.labels()
    func rolled(_ id: String) -> UInt32 { (after.first { $0.id == id }?.unread ?? 0) + after.filter { $0.parentId == id }.map { rolled($0.id) }.reduce(0, +) }
    #expect(row("Work > Clients > Provanta > RFP 2026 > Q4", after)?.unread == 1)
    #expect(rolled(row("Work > Clients > Provanta", after)!.id) == 1)
    #expect(after.allSatisfy { $0.depth <= UInt8(WhooshConstants.LABEL_DEPTH_MAX) })
}

@Test func labellingAnUnreadMessageRaisesItsLabelOnly() throws {
    let c = try core()
    let id = try #require(c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).first { $0.unread }).messageId
    let before = c.labels()
    _ = try c.run(command: .label(messageIds: [id], labelName: "Work > Suppliers", createIfMissing: false))
    let after = c.labels()
    let sup = { (rows: [MailLabel]) in rows.first { $0.path == "Work > Suppliers" }!.unread }
    #expect(sup(after) == sup(before) + 1)
    #expect(after.filter { $0.path != "Work > Suppliers" }.map(\.unread) == before.filter { $0.path != "Work > Suppliers" }.map(\.unread))
}
