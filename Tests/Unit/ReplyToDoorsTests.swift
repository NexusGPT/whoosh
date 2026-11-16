import Foundation
import Testing
@testable import WhooshUI

// fix-replydoors: Reply and Reply all answer to the mail's Reply-To. Written uncompiled against the code read; never run.
// `core` below stands for `core.state(path:)`: a function from a state path to its JSON text, so no engine is needed.
// Each test names the one line of ComposerContext.swift (ReplyToRule) that, broken, turns it red.

private let me = "me@whoosh.test", other = "other@whoosh.test"
private func core(replyTo: String? = nil, recipients: String? = nil) -> (String) -> String? {
    { path in
        if path == "message/m1/reply_to" { return replyTo }
        if path == "message/m1/reply_recipients" { return recipients }
        return nil
    }
}
private let listReplyTo = #"[{"name":"Team List","address":"list@x.test"}]"#

// B1: plain() ignores `reply_recipients` and answers [fromAddress].
@Test func replyGoesToTheReplyToNamedByTheCore() {
    let r = ReplyToRule.plain(messageId: "m1", fromAddress: "bounce@x.test", senderName: "Bounce", state: core(replyTo: listReplyTo, recipients: #"["list@x.test"]"#))
    #expect(r.addresses == ["list@x.test"])
    #expect(r.name == "Team List")
}

// B2: plain() takes the sender's name even when the first recipient is not the sender (the `same(first, fromAddress)` test).
@Test func theNameIsTheSendersOnlyWhenTheSenderIsAnswered() {
    let a = ReplyToRule.plain(messageId: "m1", fromAddress: "ann@x.test", senderName: "Ann", state: core(replyTo: "[]", recipients: #"["ann@x.test"]"#))
    #expect(a.addresses == ["ann@x.test"]); #expect(a.name == "Ann")
    let b = ReplyToRule.plain(messageId: "m1", fromAddress: "ann@x.test", senderName: "Ann", state: core(replyTo: "[]", recipients: #"["someone@x.test"]"#))
    #expect(b.addresses == ["someone@x.test"]); #expect(b.name == "")
}

// B3: plain() does not drop repeats (`unique`): a Reply-To of two addresses, one repeated in another case, answers each once, in order.
@Test func severalReplyToAddressesAreAllRecipientsNoneTwice() {
    let r = ReplyToRule.plain(messageId: "m1", fromAddress: "a@x.test", senderName: "A", state: core(recipients: #"["l1@x.test","L1@x.test","l2@x.test"]"#))
    #expect(r.addresses == ["l1@x.test", "l2@x.test"])
}

// B4: all() drops the `guard !asked.isEmpty` branch or builds To from the sender: To is the Reply-To addresses.
@Test func replyAllPutsTheReplyToInToAndEveryOtherToAndCcInCc() {
    let r = ReplyToRule.all(messageId: "m1", fromAddress: "bounce@x.test", to: [me, other, "third@x.test"], cc: ["cc1@x.test", other], own: [me],
                            state: core(replyTo: listReplyTo))
    #expect(r.to == ["list@x.test"])
    #expect(r.cc == [other, "third@x.test", "cc1@x.test"])
}

// B5: all() does not pass `not: own` (the owner's own address stays in Cc, or in To when the Reply-To is the owner's).
@Test func replyAllNeverHasTheOwnersAddressAndNoAddressTwice() {
    let r = ReplyToRule.all(messageId: "m1", fromAddress: "bounce@x.test", to: [me.uppercased(), "list@x.test", "a@x.test"], cc: ["A@x.test", me], own: [me],
                            state: core(replyTo: #"[{"name":"","address":"list@x.test"},{"name":"","address":"ME@whoosh.test"},{"name":"","address":"LIST@x.test"}]"#))
    #expect(r.to == ["list@x.test"])
    #expect(r.cc == ["a@x.test"])
    let all = r.to + r.cc
    #expect(Set(all.map { $0.lowercased() }).count == all.count)
    #expect(!all.contains { $0.lowercased() == me })
}

// B6: all() does not filter the To addresses out of Cc (`filter` after `unique`).
@Test func replyAllCcLeavesOutWhatIsAlreadyInTo() {
    let r = ReplyToRule.all(messageId: "m1", fromAddress: "f@x.test", to: ["list@x.test", other], cc: [], own: [me], state: core(replyTo: listReplyTo))
    #expect(r.to == ["list@x.test"]); #expect(r.cc == [other])
}

// B7: plain() answers anything but the sender when there is no Reply-To (the empty-list branch); all() ignores the contract call.
@Test func aMailWithNoReplyToBehavesAsBefore() {
    let p = ReplyToRule.plain(messageId: "m1", fromAddress: "ann@x.test", senderName: "Ann", state: core(replyTo: "[]", recipients: #"["ann@x.test"]"#))
    #expect(p.addresses == ["ann@x.test"]); #expect(p.name == "Ann")
    let a = ReplyToRule.all(messageId: "m1", fromAddress: "ann@x.test", to: [me, other], cc: ["cc@x.test", me], own: [me], state: core(replyTo: "[]"))
    let before = replyAllRecipients(fromAddress: "ann@x.test", to: [me, other], cc: ["cc@x.test", me], ownAddresses: [me])
    #expect(a.to == before.to); #expect(a.cc == before.cc)
    // a Reply-To that holds only the owner's own address says nothing
    let own = ReplyToRule.all(messageId: "m1", fromAddress: "ann@x.test", to: [me, other], cc: [], own: [me], state: core(replyTo: #"[{"name":"","address":"me@whoosh.test"}]"#))
    let same = replyAllRecipients(fromAddress: "ann@x.test", to: [me, other], cc: [], ownAddresses: [me])
    #expect(own.to == same.to); #expect(own.cc == same.cc)
}

// B8: plain() or all() crashes or answers a value on nil or junk instead of falling back (the `guard let` / `as?` lines).
@Test func aCoreThatAnswersNothingBehavesAsBefore() {
    for state in [core(), core(replyTo: "not json", recipients: "{}"), core(replyTo: "[]", recipients: "[]"), core(replyTo: #"[{"name":"x"}]"#, recipients: "[\"\"]")] {
        let p = ReplyToRule.plain(messageId: "m1", fromAddress: "ann@x.test", senderName: "Ann", state: state)
        #expect(p.addresses == ["ann@x.test"]); #expect(p.name == "Ann")
        let a = ReplyToRule.all(messageId: "m1", fromAddress: "ann@x.test", to: [me, other], cc: ["c@x.test"], own: [me], state: state)
        let before = replyAllRecipients(fromAddress: "ann@x.test", to: [me, other], cc: ["c@x.test"], ownAddresses: [me])
        #expect(a.to == before.to); #expect(a.cc == before.cc)
    }
}

// B9: the test engine serves no `reply_to` or the wrong shape (FixtureReplyTo.swift line `if p[2] == "reply_to" { return "[]" }`).
// This reads the extension directly: FixtureCore.state(path:) calls it once the lead adds the one line named in the notes.
@Test func theTestEngineAnswersBothPathsInTheEnginesShape() throws {
    let url = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("docs/design/fixtures/mac-main-sample.json")
    let c = try FixtureCore(url: url, clock: FixtureClock())
    let m = try #require(c.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil)).first, "the fixture has an inbox row")
    let id = m.messageId
    let entry = try #require(c.message(messageId: id))
    let rt = try #require(c.stateReplyToServed("message/\(id)/reply_to")?.value, "reply_to is served")
    #expect(rt == "[]")
    let rr = try #require(c.stateReplyToServed("message/\(id)/reply_recipients")?.value, "reply_recipients is served")
    let list = try #require(try JSONSerialization.jsonObject(with: Data(rr.utf8)) as? [String])
    #expect(list == [entry.fromAddress])
    #expect(c.stateReplyToServed("message/no-such-mail/reply_to")?.value == nil)
    #expect(c.stateReplyToServed("message/\(id)/subject") == nil)
}
