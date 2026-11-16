import Foundation
import Testing
@testable import WhooshUI

// The fixture core's delta 18: the mail import, leaving a mailing list, muting a thread, remind if no reply. Written uncompiled against the
// code read; never run. Each test names the mutation of the target that must turn it red. The rules themselves are the contract's
// (core/crates/contracts/tests/delta18.rs); these prove the fixture core serves them over its commands and its state paths.

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func core(_ clock: FixtureClock = FixtureClock()) throws -> FixtureCore {
    try FixtureCore(url: repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock)
}
private func json(_ c: FixtureCore, _ path: String) throws -> Any {
    let text = try #require(c.state(path: path), "no state at \(path)")
    return try JSONSerialization.jsonObject(with: Data(text.utf8), options: [.fragmentsAllowed])
}
private func refusal(_ body: () throws -> Void) -> (field: String, words: String)? {
    do { try body(); return nil } catch CoreError.Invalid(let f, let w) { return (f, w) } catch { return nil }
}
private func query(_ m: Mailbox?) -> InboxQuery { InboxQuery(tabId: "primary", accountId: nil, labelId: nil, mailbox: m, accountIds: [], filters: []) }
private let appleMailId = "apple-mail:~/Library/Mail"

// Import 1. Mutant: the scan serves nothing before `import/sources` is read, or keeps no history across a second scan.
@Test func aScanListsAppleMailAndTheFilesTheOwnerPicked() throws {
    let c = try core()
    #expect((try json(c, "import/sources") as? [Any])?.isEmpty == true, "before any scan, nothing was found")   // `[Any]` is not Equatable: `== []` does not compile
    _ = try c.run(command: .importScan(pickedPaths: ["~/Documents/Outlook.olm", "~/Mail/in.mbox"]))
    let rows = try #require(try json(c, "import/sources") as? [[String: Any]])
    #expect(rows.map { $0["id"] as? String } == [appleMailId, "olm:~/Documents/Outlook.olm", "mbox:~/Mail/in.mbox"], "Apple Mail first, then the picks in order")
    #expect(rows[0]["access"] as? String == "readable")
    #expect(rows[0]["message_count"] as? Int == 1240)
    #expect(rows[2]["message_count"] is NSNull, "an mbox has no cheap count")
    #expect(rows[0]["words"] as? String == "", "a readable source has no sentence")
    // a second scan with the same pick is the same row, not a duplicate
    _ = try c.run(command: .importScan(pickedPaths: ["~/Documents/Outlook.olm"]))
    #expect((try json(c, "import/sources") as? [[String: Any]])?.count == 2)
}

// Import 2. Mutant: a picked .txt is accepted, or a failed scan keeps half its picks.
@Test func aPickThatIsNotAMailArchiveIsRefusedInWords() throws {
    let c = try core()
    let r = refusal { _ = try c.run(command: .importScan(pickedPaths: ["~/notes.txt"])) }
    #expect(r?.field == "import_bad_pick")
    #expect(r?.words.contains(".olm") == true)
    #expect(c.state(path: "import/sources") == "[]", "a refused scan changes nothing")
}

// Import 3. Mutant: Full Disk Access is not refused at the start, or the words differ from the contract's.
@Test func aSourceNeedingFullDiskAccessCannotStartAndSaysHowToGrantIt() throws {
    let c = try core()
    try c.fixtureImportSources(["apple-mail=fda"])
    _ = try c.run(command: .importScan(pickedPaths: []))
    let row = try #require((try json(c, "import/sources") as? [[String: Any]])?.first)
    #expect(row["access"] as? String == "needs_full_disk_access")
    #expect(row["words"] as? String == importAccessWords(access: .needsFullDiskAccess))
    let r = refusal { _ = try c.run(command: .importStart(sourceId: appleMailId, into: .imported)) }
    #expect(r?.field == "import_needs_full_disk_access")
    #expect(r?.words.contains("skip this step") == true, "the step says skipping is fine")
}

// Import 4. Mutant: start accepts an unknown source or an unknown account, or allows a second run.
@Test func aStartIsRefusedForAnUnknownSourceAnUnknownAccountAndASecondRun() throws {
    let c = try core()
    #expect(refusal { _ = try c.run(command: .importStart(sourceId: appleMailId, into: .imported)) }?.field == "import_unknown_source", "no scan yet")
    _ = try c.run(command: .importScan(pickedPaths: []))
    #expect(refusal { _ = try c.run(command: .importStart(sourceId: appleMailId, into: .account(accountId: "acc-99"))) }?.field == "import_bad_target")
    let run = try #require(try c.run(command: .importStart(sourceId: appleMailId, into: .account(accountId: "acc-01"))).createdId)
    #expect(run == "run-1")
    #expect(refusal { _ = try c.run(command: .importStart(sourceId: appleMailId, into: .imported)) }?.field == "import_already_running")
}

// Import 5. Mutant: progress fraction is stored instead of derived, or stored + skipped passes done.
@Test func progressCountsUpAndTheBarIsDerived() throws {
    let c = try core()
    _ = try c.run(command: .importScan(pickedPaths: []))
    let run = try #require(try c.run(command: .importStart(sourceId: appleMailId, into: .imported)).createdId)
    var p = try #require(try json(c, "import/\(run)/progress") as? [String: Any])
    #expect(p["state"] as? String == "running"); #expect(p["done"] as? Int == 0); #expect(p["total"] as? Int == 1240); #expect(p["current"] as? String == "INBOX")
    #expect(c.state(path: "import/\(run)/result") == nil, "no result while it runs")
    try c.fixtureImportProgress(runId: run, step: "310")
    p = try #require(try json(c, "import/\(run)/progress") as? [String: Any])
    #expect(p["done"] as? Int == 310); #expect(p["fraction"] as? Double == 0.25)
    #expect((p["stored"] as? Int ?? 0) + (p["skipped"] as? Int ?? 0) <= 310)
    try c.fixtureImportProgress(runId: run, step: "5000")   // an estimate: past the total reads full
    p = try #require(try json(c, "import/\(run)/progress") as? [String: Any])
    #expect(p["fraction"] as? Double == 1.0)
}

// Import 6. Mutant: finish leaves the counts not adding up, drops the per-account split, or forgets the source's history.
@Test func aFinishedRunAddsUpAndJoinsTheSourcesHistory() throws {
    let c = try core()
    _ = try c.run(command: .importScan(pickedPaths: []))
    let run = try #require(try c.run(command: .importStart(sourceId: appleMailId, into: .account(accountId: "acc-01"))).createdId)
    try c.fixtureImportProgress(runId: run, step: "finish")
    let r = try #require(try json(c, "import/\(run)/result") as? [String: Any])
    let seen = r["seen"] as? Int ?? -1, stored = r["stored"] as? Int ?? -1, there = r["already_there"] as? Int ?? -1, skipped = (r["skipped"] as? [Any])?.count ?? -1
    #expect(r["state"] as? String == "finished")
    #expect(seen == stored + there + skipped, "10 seen is stored + already there + skipped, never overlapping")
    #expect((r["per_account"] as? [[String: Any]])?.first?["account_id"] as? String == "acc-01")
    #expect((r["per_account"] as? [[String: Any]])?.first?["stored"] as? Int == stored)
    let row = try #require((try json(c, "import/sources") as? [[String: Any]])?.first)
    #expect(row["imported_count"] as? Int == stored, "Settings > Accounts reads what earlier runs stored")
    #expect(!(row["last_run_ms"] is NSNull))
    // the run is over: another step is refused, and a new run may start
    #expect(refusal { try c.fixtureImportProgress(runId: run, step: "5") }?.field == "import_not_running")
    #expect(try c.run(command: .importStart(sourceId: appleMailId, into: .imported)).createdId == "run-2")
}

// Import 7. Mutant: cancel discards what was stored, accepts a finished run, or leaves the state running.
@Test func cancelKeepsWhatWasStoredAndNeedsARunningRun() throws {
    let c = try core()
    _ = try c.run(command: .importScan(pickedPaths: []))
    let run = try #require(try c.run(command: .importStart(sourceId: appleMailId, into: .imported)).createdId)
    try c.fixtureImportProgress(runId: run, step: "100")
    _ = try c.run(command: .importCancel(runId: run))
    let r = try #require(try json(c, "import/\(run)/result") as? [String: Any])
    #expect(r["state"] as? String == "cancelled"); #expect(r["seen"] as? Int == 100); #expect((r["stored"] as? Int ?? 0) > 0, "what it stored stays")
    #expect(refusal { _ = try c.run(command: .importCancel(runId: run)) }?.field == "import_not_running")
    #expect(refusal { _ = try c.run(command: .importCancel(runId: "run-404")) }?.field == "import_not_running")
}

// Import 8. Mutant: a mid-run refusal carries no words, or a failed run shows no reason.
@Test func aRunRefusedOrFailedMidwayCarriesItsReasonInWords() throws {
    let c = try core()
    _ = try c.run(command: .importScan(pickedPaths: []))
    let a = try #require(try c.run(command: .importStart(sourceId: appleMailId, into: .imported)).createdId)
    try c.fixtureImportProgress(runId: a, step: "refuse")
    let pa = try #require(try json(c, "import/\(a)/progress") as? [String: Any])
    #expect(pa["state"] as? String == "refused")
    #expect((pa["refusal"] as? [String: Any])?["code"] as? String == "import_needs_full_disk_access")
    #expect(((pa["refusal"] as? [String: Any])?["words"] as? String)?.contains("Full Disk Access") == true)
    let b = try #require(try c.run(command: .importStart(sourceId: appleMailId, into: .imported)).createdId)
    try c.fixtureImportProgress(runId: b, step: "fail")
    let rb = try #require(try json(c, "import/\(b)/result") as? [String: Any])
    #expect((rb["refusal"] as? [String: Any])?["code"] as? String == "import_store_failed")
}

// Import 9. Mutant: `fixture import-sources` ignores a spec, or accepts a spec it cannot read.
@Test func theImportSourcesVerbSetsWhatTheNextScanFinds() throws {
    let c = try core()
    try c.fixtureImportSources(["apple-mail=fda", "olm:~/a.olm=2310", "mbox:~/b.mbox=?", "olm:~/gone.olm=missing"])
    _ = try c.run(command: .importScan(pickedPaths: []))
    let rows = try #require(try json(c, "import/sources") as? [[String: Any]])
    #expect(rows.map { $0["access"] as? String } == ["needs_full_disk_access", "readable", "readable", "not_found"])
    #expect(rows[1]["message_count"] as? Int == 2310)
    #expect(refusal { try c.fixtureImportSources(["zip=3"]) }?.field == "spec")
    try c.fixtureImportSources([])
    _ = try c.run(command: .importScan(pickedPaths: []))
    #expect(c.state(path: "import/sources") == "[]", "nothing found on this Mac")
}

// Unsubscribe 1. Mutant: a state word drifts, or Done does not win.
@Test func theUnsubscribeStateReadsNoneOneClickLinkMailAndDone() throws {
    let c = try core()
    #expect(try json(c, "message/msg-01/unsubscribe") as? String == "none", "no header")
    #expect(try json(c, "message/msg-09/unsubscribe") as? String == "one_click")
    #expect(try json(c, "message/msg-11/unsubscribe") as? String == "link", "an https page and no one-click offer")
    _ = try c.run(command: .unsubscribe(messageId: "msg-09"))
    #expect(try json(c, "message/msg-09/unsubscribe") as? String == "done")
    #expect(c.state(path: "message/msg-09/unsubscribed") == "true", "the older state word agrees")
    #expect(c.state(path: "message/msg-404/unsubscribe") == nil)
}

// Unsubscribe 2. Mutant: a non-list message is unsubscribed, or a second unsubscribe goes through.
@Test func leavingAListIsRefusedForANonListAndASecondTime() throws {
    let c = try core()
    #expect(refusal { _ = try c.run(command: .unsubscribe(messageId: "msg-01")) }?.field == "unsubscribe_not_a_list")
    _ = try c.run(command: .unsubscribe(messageId: "msg-15"))
    let again = refusal { _ = try c.run(command: .unsubscribe(messageId: "msg-15")) }
    #expect(again?.field == "unsubscribe_already_done"); #expect(again?.words == "You already left this list.")
}

// Mute 1. Mutant: unmuting leaves the thread muted, or the Muted view omits it.
@Test func aMutedMessageLeavesTheInboxAndSitsInTheMutedView() throws {
    let c = try core()
    _ = try c.run(command: .mute(messageIds: ["msg-05"], muted: true))
    #expect(c.state(path: "message/msg-05/muted") == "true")
    func rows(_ m: Mailbox?) -> [String] { c.inbox(query: query(m)).map(\.messageId) }
    #expect(rows(.muted).contains("msg-05")); #expect(!rows(nil).contains("msg-05"))
    _ = try c.run(command: .mute(messageIds: ["msg-05"], muted: false))
    #expect(!rows(.muted).contains("msg-05")); #expect(rows(nil).contains("msg-05"))
}

// Mute 2. Mutant: an arrival in a muted thread stays unread or lands in the inbox; an arrival in another thread is muted.
@Test func newMailInAMutedThreadArrivesReadAndOutOfTheInbox() throws {
    let c = try core()
    let subject = try #require(c.inbox(query: query(nil)).first { $0.messageId == "msg-07" }?.subject)
    _ = try c.run(command: .mute(messageIds: ["msg-07"], muted: true))
    _ = try c.deliverArrival(id: "arr-1", account: "acc-01", from: "x@example.com", sender: "X", subject: "Re: \(subject)", body: "ok")
    #expect(c.state(path: "message/arr-1/muted") == "true"); #expect(c.state(path: "message/arr-1/read") == "true")
    _ = try c.deliverArrival(id: "arr-2", account: "acc-01", from: "x@example.com", sender: "X", subject: "Something else entirely", body: "ok")
    #expect(c.state(path: "message/arr-2/muted") == "false"); #expect(c.state(path: "message/arr-2/read") == "false")
}

private func draftReplying(_ c: FixtureCore, to thread: String) throws -> String {
    try #require(try c.run(command: .saveDraft(fromAccountId: nil, to: ["laura@example.com"], cc: [], subject: "Re: x", body: "Hello", replyToThreadId: thread)).createdId)
}

// Reminder 1. Mutant: waiting reads fired early, a reply before the time does not answer it, a reply after it unfires it.
@Test func aReminderWaitsFiresOrIsAnsweredByTheRepliesTimeline() throws {
    let clock = FixtureClock(); let c = try core(clock)
    let d = try draftReplying(c, to: "msg-03")
    let at = clock.nowMs() + 60_000
    #expect(try c.run(command: .remindIfNoReply(draftId: d, atMs: at)).createdId == d)
    var s = try #require(try json(c, "thread/msg-03/reminder") as? [String: Any])
    #expect(s["state"] as? String == "waiting"); #expect(s["at_ms"] as? Int64 == at); #expect(s["draft_id"] as? String == d)
    clock.advance(seconds: 61)
    s = try #require(try json(c, "thread/msg-03/reminder") as? [String: Any]); #expect(s["state"] as? String == "fired")
    #expect(c.state(path: "reply_reminder/\(d)/fired") == "true", "the older path agrees")
    try c.fixtureReplyArrived(threadId: "msg-03")   // a reply AFTER the time: it does not unfire it
    s = try #require(try json(c, "thread/msg-03/reminder") as? [String: Any]); #expect(s["state"] as? String == "fired")
    // a thread where the reply comes first
    let d2 = try draftReplying(c, to: "msg-12")
    _ = try c.run(command: .remindIfNoReply(draftId: d2, atMs: clock.nowMs() + 60_000))
    #expect(try c.fixtureReplyArrived(threadId: "msg-12") == "answered")
    clock.advance(seconds: 120)
    #expect((try json(c, "thread/msg-12/reminder") as? [String: Any])?["state"] as? String == "answered", "it never fires after an in-time reply")
}

// Reminder 2. Mutant: a past time is accepted, clearing a missing reminder passes, or a cleared reminder stays in the view.
@Test func aPastTimeIsRefusedAndClearingDismissesAFiredOne() throws {
    let clock = FixtureClock(); let c = try core(clock)
    let d = try draftReplying(c, to: "msg-03")
    #expect(refusal { _ = try c.run(command: .remindIfNoReply(draftId: d, atMs: clock.nowMs() - 1)) }?.field == "reminder_in_the_past")
    #expect(refusal { _ = try c.run(command: .clearReplyReminder(draftId: d)) }?.field == "reminder_nothing_to_clear")
    _ = try c.run(command: .remindIfNoReply(draftId: d, atMs: clock.nowMs() + 1_000))
    clock.advance(seconds: 2)
    func viewIds() -> [String] { c.inbox(query: query(.reminders)).map(\.messageId) }
    #expect(viewIds() == ["msg-03"], "a fired reminder is a row of the Reminders view")
    _ = try c.run(command: .clearReplyReminder(draftId: d))
    #expect(viewIds() == [], "cleared, the row goes")
    #expect((try json(c, "thread/msg-03/reminder") as? [String: Any])?["state"] as? String == "none")
}

// Reminder 3. Mutant: a waiting or answered reminder shows in the Reminders view, or a second reminder on a thread is kept beside the first.
@Test func onlyAFiredReminderIsInTheViewAndAThreadHoldsOne() throws {
    let clock = FixtureClock(); let c = try core(clock)
    let d1 = try draftReplying(c, to: "msg-03"), d2 = try draftReplying(c, to: "msg-03")
    _ = try c.run(command: .remindIfNoReply(draftId: d1, atMs: clock.nowMs() + 1_000))
    _ = try c.run(command: .remindIfNoReply(draftId: d2, atMs: clock.nowMs() + 5_000))
    #expect(c.state(path: "reply_reminder/\(d1)/at_ms") == nil, "the second reminder replaced the first")
    func viewIds() -> [String] { c.inbox(query: query(.reminders)).map(\.messageId) }
    clock.advance(seconds: 2)
    #expect(viewIds() == [], "still waiting for d2's time")
    clock.advance(seconds: 5)
    #expect(viewIds() == ["msg-03"])
}
