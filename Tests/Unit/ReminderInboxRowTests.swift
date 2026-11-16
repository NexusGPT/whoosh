import Foundation
import Testing
@testable import WhooshUI

// gap-remind: "Remind me if no reply" returns the thread to the inbox LIST marked with the reminder. The fixture core serves the row's
// `reminderFired` by the contract's own rule (the real core's twin is core/crates/engine/tests/engine_reminder_inbox.rs); the row draws the
// chip from it (InboxRowView.reminderChip) and its words are the catalogue's "Reminder". Written uncompiled; the lead runs it. Each test
// names its one-line broken copy.

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func core(_ clock: FixtureClock) throws -> FixtureCore {
    try FixtureCore(url: repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock)
}
private func primary() -> InboxQuery { InboxQuery(tabId: "primary", accountId: nil, labelId: nil, mailbox: nil, accountIds: [], filters: []) }
private func draftReplying(_ c: FixtureCore, to thread: String) throws -> String {
    try #require(try c.run(command: .saveDraft(fromAccountId: nil, to: ["laura@example.com"], cc: [], subject: "Re: x", body: "Hello", replyToThreadId: thread)).createdId)
}
private func listed(_ c: FixtureCore, _ id: String) throws -> InboxRow {
    try #require(c.inbox(query: primary()).first { $0.messageId == id }, "\(id) is not in the inbox list")
}

// Broken copy: FixtureCore.threadReminderFiredLocked returns false.
@Test func aFiredReminderMarksItsThreadsRowInTheInboxListAndAWaitingOrAnsweredOneDoesNot() throws {
    let clock = FixtureClock(); let c = try core(clock)
    let d = try draftReplying(c, to: "msg-03")
    _ = try c.run(command: .remindIfNoReply(draftId: d, atMs: clock.nowMs() + 60_000))
    #expect(try listed(c, "msg-03").reminderFired == false, "a reminder not yet due marks nothing")
    clock.advance(seconds: 61)
    #expect(try listed(c, "msg-03").reminderFired == true, "no reply by its time: the row carries the reminder")
    // a thread where the reply came first: never marked
    let d2 = try draftReplying(c, to: "msg-12")
    _ = try c.run(command: .remindIfNoReply(draftId: d2, atMs: clock.nowMs() + 60_000))
    #expect(try c.fixtureReplyArrived(threadId: "msg-12") == "answered")
    clock.advance(seconds: 120)
    #expect(try listed(c, "msg-12").reminderFired == false, "an answered reminder marks nothing")
    // dismissed: the mark goes with the reminder
    _ = try c.run(command: .clearReplyReminder(draftId: d))
    #expect(try listed(c, "msg-03").reminderFired == false, "clearing the reminder clears the mark")
}

// Broken copy: InboxRowView.RowKey.init does not copy `reminderFired` (the key stays equal, the row is never redrawn with the chip).
@Test func theRowsKeyChangesWhenTheReminderFiresSoTheChipIsDrawn() throws {
    let clock = FixtureClock(); let c = try core(clock)
    let before = try listed(c, "msg-03")
    var fired = before; fired.reminderFired = true
    let acc = RowAccount(id: "acc-01", name: "Work", address: "me@example.com", colour: "#0A84FF", iconId: nil, needsAttention: false)
    func key(_ r: InboxRow) -> RowKey {
        RowKey(row: r, index: 0, account: acc, focused: false, selected: false, starred: false, repliedLast: false, compact: false, showEmblem: true,
               highlightKey: 0, animation: nil, why: nil, whyTag: nil, stripAllowed: true,
               muted: false, vip: false, snoozeWords: nil, pointerRested: false)   // the four marks of the picked board (row-marks, 15 Nov)
    }
    #expect(key(before).reminderFired == false)
    #expect(key(fired).reminderFired == true, "the key serves the mark the chip is drawn from")
    #expect(key(before) != key(fired), "the row is redrawn when the reminder fires")
}

// Broken copy: InboxRowView.showsReminderChip returns false (the row draws no chip).
@Test func theRowDrawsTheReminderChipOnlyWhenItsReminderFired() throws {
    let clock = FixtureClock(); let c = try core(clock)
    var r = try listed(c, "msg-03")
    #expect(InboxRowView.showsReminderChip(r) == false)
    r.reminderFired = true
    #expect(InboxRowView.showsReminderChip(r) == true)
}

// Broken copy: the catalogue entry "Reminder" is removed (the chip would show the key, not a word in the app's language).
@Test func theChipsWordIsTheCataloguesNotAnInlineSentence() {
    let word = Words.text("Reminder")
    #expect(!word.isEmpty)
    #expect(Words.text("Reminders") != word, "the chip is the singular, the view's name stays the plural")
}
