import Foundation
import Testing
@testable import WhooshUI

/// Logic probes for the fixture core: the invariants J1 leans on, proven below the UI.
/// The fixture path is read from the repo, never from a built screen.
private func fixture() throws -> FixtureCore {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    return try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}

private func primary(_ c: FixtureCore) -> Int { Int(c.counts().tabs.first { $0.tabId == "primary" }?.unread ?? 0) }
private func rows(_ c: FixtureCore) -> [InboxRow] { c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)) }

@Test func primaryRowsComeInFixtureOrder() throws {
    let c = try fixture()
    #expect(rows(c).count == (try FixtureFileFigures.primaryRows()))
    #expect(rows(c).first?.senderName == "Linea Supply")
    #expect(primary(c) == (try FixtureFileFigures.primaryUnread()))
}

@Test func archiveDropsTheRowAndOneUnread() throws {
    let c = try fixture()
    let first = rows(c)[0].messageId
    _ = try c.run(command: .archive(messageIds: [first]))
    #expect(primary(c) == (try FixtureFileFigures.primaryUnread()) - 1)
    #expect(!rows(c).contains { $0.messageId == first })
    #expect(c.state(path: "message/\(first)/archived") == "true")
}

@Test func archivingAReadMessageLeavesTheUnreadCount() throws {
    let c = try fixture()
    let readRow = try #require(rows(c).first { !$0.unread })
    _ = try c.run(command: .archive(messageIds: [readRow.messageId]))
    #expect(primary(c) == (try FixtureFileFigures.primaryUnread()))
}

@Test func undoBringsTheRowBackAtItsPlace() throws {
    let c = try fixture()
    let before = rows(c).map(\.messageId)
    _ = try c.run(command: .archive(messageIds: [before[0]]))
    _ = try c.run(command: .undo)
    #expect(rows(c).map(\.messageId) == before)
    #expect(primary(c) == (try FixtureFileFigures.primaryUnread()))
    #expect(c.state(path: "undo_entry/consumed_last") == "true")
}

@Test func theFailingAccountRefusesInWords() throws {
    let c = try fixture()
    let id = try #require(rows(c).first { $0.accountId == c.failingAccountId }?.messageId)
    #expect(throws: CoreError.self) { try c.run(command: .archive(messageIds: [id])) }
    #expect(rows(c).contains { $0.messageId == id })
}

@Test func pinnedRowsSortFirstUnderEveryAccountFilter() throws {
    let c = try fixture()
    let r = rows(c)
    let target = try #require(r.first { !$0.pinned })
    _ = try c.run(command: .pin(messageId: target.messageId, pinned: true))
    let other = try #require(r.first { $0.accountId != target.accountId })
    let filtered = c.inbox(query: InboxQuery(tabId: "primary", accountId: other.accountId, labelId: nil))
    #expect(filtered.contains { $0.messageId == target.messageId })
    #expect(filtered.prefix { $0.pinned }.contains { $0.messageId == target.messageId })
}

@Test func quickSendIsSentOnlyAfterTheUndoWindow() throws {
    let clock = FixtureClock()
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let c = try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock)
    _ = try c.run(command: .quickSend(threadId: "msg-01", body: "Thanks."))
    #expect(c.state(path: "outbox_entry/0/state") == "\"undoable\"")
    clock.advance(seconds: Double(WhooshConstants.UNDO_SEND_DEFAULT_S + 1))
    #expect(c.state(path: "outbox_entry/0/state") == "\"sent\"")
}
