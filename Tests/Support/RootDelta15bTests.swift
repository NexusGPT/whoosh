import Foundation
import Testing
@testable import WhooshUI

/// Delta 15b (2026-11-09): the two regressions the main43/44 bundles showed. A sent copy is listed by its thread and by Sent
/// only (never Archive, Primary or a count); a core search for words in a record's attachment name answers with that record.
private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func core(_ clock: FixtureClock = FixtureClock()) throws -> FixtureCore {
    try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock)
}
private func ids(_ rows: [InboxRow]) -> [String] { rows.map(\.messageId) }
private final class Hits: EventSink, @unchecked Sendable {
    let lock = NSLock(); var last: [SearchHit] = []; var answered: [String] = []
    func onEvent(event: CoreEvent) { if case .searchResults(let q, let h) = event { lock.withLock { last = h; answered.append(q) } } }
}

@MainActor @Test func d15bASentReplyLeavesTheArchiveListThePrimaryListAndTheCountsAsTheyWere() throws {
    let clock = FixtureClock()
    let c = try core(clock)
    let primary = InboxQuery(tabId: "primary", accountId: nil, labelId: nil)
    let archive = InboxQuery(tabId: "primary", accountId: nil, labelId: nil, mailbox: .archive)
    let before = (ids(c.inbox(query: primary)), ids(c.inbox(query: archive)), c.counts())
    let outbox = try #require(try c.run(command: .quickSend(threadId: "msg-03", body: "Thanks, see you Thursday.")).createdId)
    #expect(clock.advance(seconds: Double(WhooshConstants.UNDO_SEND_DEFAULT_S) + 1))
    let sentId = "sent-\(outbox)"
    #expect(c.thread(threadId: "msg-03")?.messageIds.contains(sentId) == true, "the thread still lists the reply")
    #expect(ids(c.inbox(query: primary)) == before.0, "Primary unchanged by a sent reply")
    #expect(ids(c.inbox(query: archive)) == before.1, "Archive unchanged by a sent reply (main43: ['msg-02', 'sent-outbox-2'])")
    let after = c.counts()
    #expect(after.tabs == before.2.tabs, "tab counts unchanged")
    #expect(after.mailboxes[mailboxKey(mailbox: .archive)] == before.2.mailboxes[mailboxKey(mailbox: .archive)], "no Archive count for a sent copy")
    for m in [Mailbox.allMail, .starred, .pinned, .vips, .junk, .trash] {
        #expect(!ids(c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil, mailbox: m))).contains(sentId), "\(m) lists no sent copy")
    }
    #expect(c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil, mailbox: .sent)).count >= 1, "Sent lists the send")
}

@MainActor @Test func d15bArchivingEveryPrimaryRowEmptiesPrimaryAfterASentReply() throws {
    let clock = FixtureClock()
    let c = try core(clock)
    _ = try c.run(command: .quickSend(threadId: "msg-03", body: "Thanks."))
    #expect(clock.advance(seconds: Double(WhooshConstants.UNDO_SEND_DEFAULT_S) + 1))
    let primary = InboxQuery(tabId: "primary", accountId: nil, labelId: nil)
    _ = try c.run(command: .archive(messageIds: ids(c.inbox(query: primary))))
    #expect(c.inbox(query: primary).isEmpty, "J1.50: Primary is empty once every row is archived (main43: 4 rows left)")
}

@Test func d15bACoreSearchForAnAttachmentsWordAnswersWithThatRecordFirstInItsThread() throws {
    let c = try core()
    try c.loadCorpus(from: root.appendingPathComponent("fixtures/tests-j2.json"))
    let sink = Hits()
    _ = c.subscribe(stream: .search, sink: sink)
    _ = try c.run(command: .search(query: "quote"))
    #expect(sink.lock.withLock { sink.answered } == ["quote"], "the core answers an unquoted search on the Search stream")
    let hits = sink.lock.withLock { sink.last }
    let inThread = hits.filter { c.message(messageId: $0.messageId)?.threadId == "thr-01" }
    #expect(inThread.first?.messageId == "msg-01", "record 1 is its thread's best hit: \(hits.map(\.messageId))")
    #expect(c.message(messageId: "msg-01")?.attachmentNames == ["quote-Q1-print-run.pdf"], "the row's attachment name is served")
    let row = c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).first { $0.messageId == "msg-01" }
    #expect(row.map { $0.attachmentCount > 0 } ?? true, "a listed record 1 counts its attachment")
}

@Test func d15bAnAttachmentNameHitNamesItsFileWithAMeaningRankerAnswering() throws {
    let c = try core()
    try c.loadCorpus(from: root.appendingPathComponent("fixtures/tests-j2.json"))
    let saved = FixtureCore.meaningRanker
    defer { FixtureCore.meaningRanker = saved }
    // a ranker that answers (weights present): its meaning-only hits must not take the attachment name off the record
    FixtureCore.meaningRanker = { _, ids, _ in Array(ids.prefix(5)).map { (id: $0, score: 0.9) } }
    let sink = Hits()
    _ = c.subscribe(stream: .search, sink: sink)
    _ = try c.run(command: .search(query: "pdf"))   // only in the file name, not the subject or body
    let hit = sink.lock.withLock { sink.last }.first { $0.messageId == "msg-01" }
    #expect(hit?.attachmentName == "quote-Q1-print-run.pdf", "\(String(describing: hit))")
}

/// SPEC A87: each armed refusal comes back with its cause, its raw words kept, and the Rust classifier agreeing.
@MainActor @Test func a87EveryArmedRefusalNamesItsCauseAndKeepsTheServersWords() throws {
    let c = try core()
    let cfg = AccountConfig(imapHost: "imap.srv.example.test", imapPort: 993, imapSecurity: .ssl, smtpHost: "smtp.srv.example.test", smtpPort: 587,
                            smtpSecurity: .starttls, username: "a@srv.example.test", smtpUsername: "a@srv.example.test", authMethod: .password, source: .manual)
    let all: [RefusalCause] = [.basicAuthDisabled, .badPassword, .hostUnreachable, .tlsFailed, .smtpRefused, .unknown]
    for cause in all {
        #expect(c.armRefusal(refusalCauseId(cause: cause)))
        let r = c.validateAccount(config: cfg, password: SecretText(value: "fixture-password"))
        #expect(!r.ok && r.cause == cause, "\(cause): \(String(describing: r.cause))")
        #expect(r.summaryWords == FixtureAddAccountBook.rawWords(cause, cfg).1, "the raw words are kept for Details")
        #expect(refusalCause(steps: r.steps) == cause, "the contract's classifier agrees")
        #expect(!AddAccountWords.causeSentence(cause).contains(r.summaryWords), "the sentence is not the raw text")
    }
    #expect(c.validateAccount(config: cfg, password: SecretText(value: "fixture-password")).ok, "armed once only")
    #expect(!c.armRefusal("no-such-cause"))
}
