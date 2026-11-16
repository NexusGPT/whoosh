import Foundation
import Testing
@testable import WhooshUI

/// Probes for contracts delta 10: the account set, views and filters, the new commands, the mailbox count rule.
private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func core() throws -> FixtureCore { try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock()) }
private func q(_ accounts: [String] = [], filters: [InboxFilter] = [], mailbox: Mailbox? = nil) -> InboxQuery {
    InboxQuery(tabId: "primary", accountId: nil, labelId: nil, mailbox: mailbox, accountIds: accounts, filters: filters)
}

@Test func d10UncheckingOneAccountHidesOnlyItsMail() throws {
    let c = try core()
    let all = c.inbox(query: q())
    let accounts = Set(c.accounts().map(\.id))
    let dropped = try #require(all.first { !$0.pinned }?.accountId)
    let shown = c.inbox(query: q(Array(accounts.subtracting([dropped]))))
    #expect(!shown.contains { $0.accountId == dropped && !$0.pinned })
    #expect(shown.count == all.filter { $0.accountId != dropped || $0.pinned }.count)
    // the older single field still reads as a set of one
    let one = c.inbox(query: InboxQuery(tabId: "primary", accountId: dropped, labelId: nil))
    #expect(one.filter { !$0.pinned }.allSatisfy { $0.accountId == dropped })
}

@Test func d10FiltersKeepOnlyMatchingRows() throws {
    let c = try core()
    #expect(c.inbox(query: q(filters: [.unread])).allSatisfy { $0.unread })
    #expect(!c.inbox(query: q(filters: [.unread])).isEmpty)
    #expect(c.inbox(query: q(filters: [.starred])).isEmpty)
    let first = c.inbox(query: q())[0].messageId
    _ = try c.run(command: .star(messageIds: [first], starred: true))
    #expect(c.inbox(query: q(filters: [.starred])).map(\.messageId) == [first])
}

@Test func d10BuiltInViewsListTheirMessages() throws {
    let c = try core()
    let rows = c.inbox(query: q())
    let a = rows[3].messageId, b = rows[4].messageId
    _ = try c.run(command: .star(messageIds: [a], starred: true))
    _ = try c.run(command: .mute(messageIds: [b], muted: true))
    #expect(c.inbox(query: q(mailbox: .starred)).map(\.messageId) == [a])
    #expect(c.inbox(query: q(mailbox: .muted)).map(\.messageId) == [b])
    #expect(c.inbox(query: q(mailbox: .allMail)).contains { $0.messageId == b })
}

@Test func d10MarkNotDoneBringsArchivedMailBack() throws {
    let c = try core()
    let id = c.inbox(query: q())[3].messageId
    _ = try c.run(command: .archive(messageIds: [id]))
    _ = try c.run(command: .unarchive(messageIds: [id]))
    #expect(c.state(path: "message/\(id)/archived") == "false")
    #expect(c.inbox(query: q()).contains { $0.messageId == id })
}

@Test func d10UnsubscribePicksOneClickThenTheMailLink() throws {
    let c = try core()
    #expect(c.unsubscribeMethod(messageId: "msg-09") == .oneClick(url: "https://github-example.com/notifications/unsubscribe/one-click"))
    #expect(c.unsubscribeMethod(messageId: "msg-11") == .link(url: "https://billing-example.com/email-preferences"))
    #expect(c.unsubscribeMethod(messageId: "msg-01") == nil)
    #expect(unsubscribeMethodFrom(urls: ["<mailto:leave@x.example?subject=stop>", "<https://x.example/u>"], oneClickOffered: false)
            == .mail(address: "leave@x.example", subject: "stop"))
    _ = try c.run(command: .unsubscribe(messageId: "msg-09"))
    #expect(c.state(path: "message/msg-09/unsubscribed") == "true")
    #expect(throws: CoreError.self) { try c.run(command: .unsubscribe(messageId: "msg-01")) }
}

@Test func d10RemindIfNoReplyFiresAfterItsTime() throws {
    let clock = FixtureClock()
    let c = try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock)
    let draft = try #require(try c.run(command: .saveDraft(fromAccountId: nil, to: ["a@b.example"], cc: [], subject: "Quote", body: "x", replyToThreadId: nil)).createdId)
    #expect(throws: CoreError.self) { try c.run(command: .remindIfNoReply(draftId: draft, atMs: clock.nowMs() - 1)) }
    _ = try c.run(command: .remindIfNoReply(draftId: draft, atMs: clock.nowMs() + 3_600_000))
    #expect(c.state(path: "reply_reminder/\(draft)/fired") == "false")
    clock.advance(seconds: 3601)
    #expect(c.state(path: "reply_reminder/\(draft)/fired") == "true")
}

@Test func d10QuickSendFromTheChosenAccount() throws {
    let c = try core()
    let id = try #require(try c.run(command: .quickSendFrom(threadId: "msg-01", body: "Thanks.", fromAccountId: "acc-02")).createdId)
    #expect(c.outbox().first { $0.id == id }?.draft.fromAccountId == "acc-02")
    #expect(throws: CoreError.self) { try c.run(command: .quickSendFrom(threadId: "msg-01", body: "x", fromAccountId: "acc-99")) }
}

@Test func d10JunkTrashArchiveCarryNoCountAtZero() throws {
    let c = try core()
    #expect(c.counts().mailboxes["junk"] == nil && c.counts().mailboxes["trash"] == nil && c.counts().mailboxes["archive"] == nil)
    _ = try c.run(command: .archive(messageIds: [c.inbox(query: q())[3].messageId]))
    #expect(c.counts().mailboxes["archive"] == 1)
    #expect(mailboxCountShown(mailbox: .drafts, count: 0) == 0)
}

private final class Hits: EventSink, @unchecked Sendable {
    var last: [SearchHit] = []
    func onEvent(event: CoreEvent) { if case .searchResults(_, let h) = event { last = h } }
}

@Test @MainActor func d10AWordReadOutOfAnAttachmentFindsItsMessage() throws {
    let c = try core()
    try c.loadCorpus(from: root.appendingPathComponent("fixtures/tests-j2.json"))
    let sink = Hits()
    _ = c.subscribe(stream: .search, sink: sink)
    _ = try c.run(command: .search(query: "forklift"))
    let hit = sink.last.first { $0.messageId == "msg-s02" }
    #expect(hit?.matchedIn == .attachmentContent, "\(sink.last.map { ($0.messageId, $0.matchedIn) })")
    #expect(hit?.attachmentName == "warehouse-photo.jpg")
    _ = try c.run(command: .search(query: "scan-notes"))
    #expect(sink.last.first { $0.messageId == "msg-s06" }?.attachmentNote == AttachmentIndexBook.notDownloadedWords)
}

@Test func d10RemoveLabelTakesItOffAndOneZBringsItBack() throws {
    let c = try core()
    let ids = c.inbox(query: q()).prefix(6).map(\.messageId).filter { $0 != "msg-03" }.prefix(2).map { $0 }
    let made = try #require(try c.run(command: .label(messageIds: ids, labelName: "Clients > Provanta", createIfMissing: true)).createdId)
    _ = try c.run(command: .removeLabel(messageIds: ids, labelId: made))
    #expect(ids.allSatisfy { c.state(path: "label_assignment/\($0)") == "[]" })
    _ = try c.run(command: .undo)
    #expect(ids.allSatisfy { c.state(path: "label_assignment/\($0)") == #"["Provanta"]"# })
}

@Test func d10RemoveAllLabelsClearsEveryLabelAndUndoRestoresThem() throws {
    let c = try core()
    let id = c.inbox(query: q())[3].messageId
    _ = try c.run(command: .label(messageIds: [id], labelName: "Suppliers", createIfMissing: true))
    _ = try c.run(command: .label(messageIds: [id], labelName: "Clients > Nordlicht", createIfMissing: true))
    _ = try c.run(command: .removeAllLabels(messageIds: [id]))
    #expect(c.state(path: "label_assignment/\(id)") == "[]")
    _ = try c.run(command: .undo)
    #expect(c.state(path: "label_assignment/\(id)").map { $0.contains("Suppliers") && $0.contains("Nordlicht") } == true)
}

@Test func d10DeletingThreeIsUndoneByOneZ() throws {
    let c = try core()
    let ids = Array(c.inbox(query: q()).filter { !$0.pinned && $0.accountId != "acc-18" }.prefix(3).map(\.messageId))
    #expect(ids.count == 3)
    _ = try c.run(command: .deleteMessages(messageIds: ids))
    #expect(ids.allSatisfy { c.state(path: "message/\($0)/deleted") == "true" })
    _ = try c.run(command: .undo)
    #expect(ids.allSatisfy { c.state(path: "message/\($0)/deleted") == "false" })
    #expect(ids.allSatisfy { id in c.inbox(query: q()).contains { $0.messageId == id } })
}

@Test func d10AttachmentsCarryNameMimeAndAPathWhenTheFileIsThere() throws {
    let dir = FileManager.default.temporaryDirectory.appendingPathComponent("d10-att-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    try Data(repeating: 7, count: 1234).write(to: dir.appendingPathComponent("brand-book-v3.pdf"))
    setenv("WHOOSH_ATTACHMENTS_DIR", dir.path, 1); defer { unsetenv("WHOOSH_ATTACHMENTS_DIR") }
    let c = try core()
    let a = c.attachments(messageId: "msg-02")
    #expect(a.map(\.fileName) == ["brand-book-v3.pdf", "wordmark.svg"])
    #expect(a[0].mimeType == "application/pdf" && a[0].sizeBytes == 1234 && a[0].localPath == dir.appendingPathComponent("brand-book-v3.pdf").path)
    #expect(a[1].localPath == nil && a[1].sizeBytes == 0 && a[1].mimeType == "image/svg+xml")
    #expect(c.attachments(messageId: "no-such").isEmpty)
}

@Test func d10AMessageCarriesItsListUnsubscribeHeader() throws {
    let c = try core()
    let m = try #require(c.message(messageId: "msg-15"))
    #expect(m.listUnsubscribe.first == "<https://running-belgium-example.be/unsubscribe?u=1>" && m.listUnsubscribeOneClick)
    #expect(c.message(messageId: "msg-01")?.listUnsubscribe.isEmpty == true)
}

@Test func d10MoveToLabelLabelsAndArchivesAndOneZUndoesBoth() throws {
    let c = try core()
    let rows = c.inbox(query: q()).filter { !$0.pinned && $0.accountId != "acc-18" }
    let (a, b) = (rows[0].messageId, rows[1].messageId)
    let label = try #require(try c.run(command: .label(messageIds: [b], labelName: "Suppliers", createIfMissing: true)).createdId)
    _ = try c.run(command: .moveToLabel(messageIds: [a, b], labelId: label))
    #expect(c.state(path: "message/\(a)/archived") == "true" && c.state(path: "message/\(b)/archived") == "true")
    #expect(c.state(path: "label_assignment/\(a)") == #"["Suppliers"]"#)
    _ = try c.run(command: .undo)
    #expect(c.state(path: "message/\(a)/archived") == "false" && c.state(path: "message/\(b)/archived") == "false")
    #expect(c.state(path: "label_assignment/\(a)") == "[]", "the label leaves where the move added it")
    #expect(c.state(path: "label_assignment/\(b)") == #"["Suppliers"]"#, "and stays where it already was")
    #expect(throws: CoreError.self) { try c.run(command: .moveToLabel(messageIds: [a], labelId: "no-such-label")) }
}
