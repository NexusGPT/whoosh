import Foundation
import Testing
@testable import WhooshUI

/// Probes for contracts delta 12: Message.cc and bcc, sending a saved draft (Send), and the server code of a failure.
private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func core(_ clock: FixtureClock = FixtureClock()) throws -> FixtureCore {
    try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock)
}

@Test func d12Msg01CarriesItsCcAndNoBcc() throws {
    let c = try core()
    let m = try #require(c.message(messageId: "msg-01"))
    #expect(m.cc == ["finance@linea-supply-example.com", "ASSEM@NORTHLIGHT-EXAMPLE.DE"])
    #expect(m.bcc.isEmpty, "a received message never shows a Bcc")
    #expect(try #require(c.message(messageId: "msg-02")).cc.isEmpty)
}

@Test func d12SendingASavedDraftTakesItsRecipientsAndSubjectAndZBringsItBack() throws {
    let c = try core()
    let id = try #require(try c.run(command: .saveDraft(fromAccountId: "acc-02", to: ["a@x.example"], cc: [], subject: "Re", body: "Words", replyToThreadId: "msg-03")).createdId)
    _ = try c.run(command: .updateDraft(draftId: id, fromAccountId: nil, to: ["a@x.example", "b@x.example"], cc: ["c@x.example"], bcc: ["boss@x.example"],
                                        subject: "Re: the quote", body: "Edited words"))
    let out = try c.run(command: .send(draftId: id))
    let outboxId = try #require(out.createdId)
    #expect(out.undoEntryId != nil, "Send has the undo window, like QuickSend")
    let e = try #require(c.outbox().first { $0.id == outboxId })
    #expect(e.draft.fromAccountId == "acc-02" && e.draft.to == ["a@x.example", "b@x.example"] && e.draft.cc == ["c@x.example"])
    #expect(e.draft.bcc == ["boss@x.example"] && e.draft.subject == "Re: the quote" && e.draft.body == "Edited words")
    if case .undoable = e.state {} else { Issue.record("expected the undo window, got \(e.state)") }
    #expect(c.drafts(threadId: "msg-03").isEmpty, "a queued draft leaves the draft list")
    _ = try c.run(command: .undo)
    #expect(!c.outbox().contains { $0.id == outboxId }, "Z cancels the send: a cancelled entry leaves the outbox")
    #expect(c.drafts(threadId: "msg-03").map(\.id) == [id], "Z brings the draft back with its id")
    // the summary carries the header, so a reopened draft restores it (delta 12, quickreply round 5)
    let back = try #require(c.drafts(threadId: "msg-03").first)
    #expect(back.to == ["a@x.example", "b@x.example"] && back.cc == ["c@x.example"] && back.bcc == ["boss@x.example"] && back.subject == "Re: the quote")
    // sent again after Z, the Bcc is still there
    let again = try #require(try c.run(command: .send(draftId: id)).createdId)
    #expect(c.outbox().first { $0.id == again }?.draft.bcc == ["boss@x.example"])
    #expect(throws: CoreError.self) { try c.run(command: .send(draftId: "no-such-draft")) }
}

@Test func d12AFailedSendCarriesTheServerCodeBesideItsWords() throws {
    let c = try core()
    #expect(c.failNext("send", reason: "the mail server did not accept the message: try again later", code: "451 4.7.1"))
    let id = try #require(try c.run(command: .quickSend(threadId: "msg-01", body: "Yes")).createdId)
    guard case .failed(let f, _) = try #require(c.outbox().first { $0.id == id }).state else { Issue.record("expected a failed entry"); return }
    #expect(f.code == "451 4.7.1" && f.reasonWords.contains("try again later") && f.retryable)
    #expect(c.state(path: "outbox/\(id)/code") == "\"451 4.7.1\"")
    // the refusal is one-shot, and a failure without a server code reads null
    let next = try #require(try c.run(command: .quickSend(threadId: "msg-01", body: "Again")).createdId)
    if case .undoable = try #require(c.outbox().first { $0.id == next }).state {} else { Issue.record("the next send is not refused") }
    let plain = try c.insertFailedSend(to: ["a@x.example"], subject: "s", body: "b", reasonWords: "not reachable")
    #expect(c.state(path: "outbox/\(plain)/code") == "null")
}

@Test func d12IndexingLeavesNAccountsIndexedAndTheRestLoading() throws {
    let c = try core()
    let total = c.syncStates().count
    let failing = c.syncStates().filter { $0.lastError != nil }.map(\.accountId)
    try c.setIndexing(accounts: 5)
    let s = c.syncStates()
    #expect(s.prefix(5).allSatisfy { $0.historyFetched || failing.contains($0.accountId) })
    let loading = s.dropFirst(5).filter { !failing.contains($0.accountId) }
    #expect(!loading.isEmpty && loading.allSatisfy { !$0.historyFetched && $0.messagesIndexed == 0 && $0.providerStatusWords == "indexing your mail" })
    #expect(s.filter { !$0.historyFetched && $0.lastError == nil }.count == loading.count, "none of the first 5 is loading")
    try c.setIndexing(accounts: nil)
    #expect(c.syncStates().allSatisfy { $0.historyFetched || $0.lastError != nil }, "all: every account indexed again")
    #expect(throws: CoreError.self) { try c.setIndexing(accounts: total + 1) }
    #expect(throws: CoreError.self) { try c.setIndexing(accounts: -1) }
}

@Test func d12ArchivingAListMessageOffersUnsubscribeAndAcceptingLeavesTheList() throws {
    let c = try core()
    let out = try c.run(command: .archive(messageIds: ["msg-09"]))
    let sid = try #require(out.createdId, "an archive of a list message offers a suggestion")
    let s = try #require(c.suggestions().first { $0.id == sid })
    #expect(s.kind == .unsubscribe && !s.subject.isEmpty)
    _ = try c.run(command: .acceptSuggestion(suggestionId: sid))
    #expect(c.state(path: "message/msg-09/unsubscribed") == "true")
    // a plain message offers nothing
    #expect(try c.run(command: .archive(messageIds: ["msg-02"])).createdId == nil)
}

@Test func d12TheThirdSnoozeOfAThreadOffersMuteThread() throws {
    let c = try core()
    #expect(try c.run(command: .snooze(messageIds: ["msg-04"], when: .tomorrowMorning)).createdId == nil)
    _ = try c.run(command: .undo)
    #expect(try c.run(command: .snooze(messageIds: ["msg-04"], when: .tomorrowMorning)).createdId == nil)
    _ = try c.run(command: .undo)
    let sid = try #require(try c.run(command: .snooze(messageIds: ["msg-04"], when: .tomorrowMorning)).createdId, "the third snooze offers")
    #expect(c.suggestions().first { $0.id == sid }?.kind == .muteThread)
}

@Test func d12TheSameLabelTwiceOnOneSenderOffersAlwaysLabelAndResetClearsOffers() throws {
    let c = try core()
    let label = try #require(try c.run(command: .label(messageIds: ["msg-02"], labelName: "Suppliers", createIfMissing: true)).createdId)
    #expect(!c.suggestions().contains { $0.kind == .alwaysLabel }, "once is not a pattern")
    _ = try c.run(command: .removeLabel(messageIds: ["msg-02"], labelId: label))
    _ = try c.run(command: .label(messageIds: ["msg-02"], labelName: "Suppliers", createIfMissing: false))
    let s = try #require(c.suggestions().first { $0.kind == .alwaysLabel })
    #expect(s.labelId == label && !s.subject.isEmpty)
    c.clearSuggestions()
    #expect(c.suggestions().isEmpty, "fixture reset forgets the offers")
    _ = try c.run(command: .label(messageIds: ["msg-02"], labelName: "Suppliers", createIfMissing: false))
    #expect(!c.suggestions().contains { $0.kind == .alwaysLabel }, "and the counts behind them")
}

@Test func d12TheFixtureEmbedderPutsTheSubjectInTheTitleSlot() throws {
    let e = FixtureEmbedder()
    let titled = try e.embedTitled(title: "Quote for the Q1 print run", texts: ["twelve thousand units"])[0]
    let plain = try e.embed(texts: ["twelve thousand units"])[0]
    #expect(titled != plain, "the subject changes the document vector")
    #expect(try e.embedTitled(title: "  ", texts: ["twelve thousand units"])[0] == plain, "a blank title is none")
    #expect(try e.embedTitled(title: nil, texts: ["twelve thousand units"])[0] == plain)
}
