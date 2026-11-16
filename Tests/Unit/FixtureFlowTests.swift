import Foundation
import Testing
@testable import WhooshUI

// The fixture core's labelling flow (delta 17), lane corebridge-d17. Written uncompiled against the code read; never run.
// Each test names the mutation of the target that must turn it red.

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func fixtureCore() throws -> FixtureCore {
    try FixtureCore(url: repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}
private func json(_ core: FixtureCore, _ path: String) throws -> Any {
    let text = try #require(core.state(path: path), "no state at \(path)")
    return try JSONSerialization.jsonObject(with: Data(text.utf8), options: [.fragmentsAllowed])
}
private func labelSpec(_ name: String, rules: [RuleCondition]) -> LabelSpec {
    LabelSpec(name: name, parentId: nil, colour: nil, description: "", scope: .all, rules: RuleSet(conditions: rules, matchMode: .all), pinned: false)
}

// Gap 1. Mutant: applySpecFields writes `[spec.rules]` again (the extra chains vanish) or renumbers the chains.
@Test func updatingALabelKeepsEveryEntryChainWithItsNumber() throws {
    let c = try fixtureCore()
    let id = try #require(try c.run(command: .createLabelFull(spec: labelSpec("Flow keep", rules: [.from(words: "first@example.com")]))).createdId)
    _ = try c.run(command: .addLabelChain(labelId: id, placement: .entry, chain: RuleSet(conditions: [.from(words: "second@example.com")], matchMode: .all)))
    _ = try c.run(command: .addLabelChain(labelId: id, placement: .entry, chain: RuleSet(conditions: [.from(words: "third@example.com")], matchMode: .all)))
    func numbers() throws -> [Int] { ((try json(c, "label/\(id)/chains/entry")) as? [[String: Any]] ?? []).compactMap { $0["chain_id"] as? Int } }
    #expect(try numbers() == [0, 1, 2])
    // the sheet saves: chain 0 changes, chains 1 and 2 are not in the sheet and must survive with their numbers
    _ = try c.run(command: .updateLabel(labelId: id, spec: labelSpec("Flow keep", rules: [.from(words: "changed@example.com")])))
    #expect(try numbers() == [0, 1, 2], "every chain survives an update, numbers unchanged")
    #expect(c.state(path: "label/\(id)/chains/entry/count") == "3")
    // an emptied sheet rule removes chain 0 only; the next new chain takes a number above the survivors
    _ = try c.run(command: .updateLabel(labelId: id, spec: labelSpec("Flow keep", rules: [])))
    #expect(try numbers() == [1, 2])
    let added = try #require(try c.run(command: .addLabelChain(labelId: id, placement: .entry, chain: RuleSet(conditions: [.from(words: "fourth@example.com")], matchMode: .all))).createdId)
    #expect(added == "3", "a freed number is not handed out again while a higher one lives")
}

// Gap 2. Mutant: the path is not served, a fact reads from the wrong field, or Cc me ignores the To check.
@Test func aMessageServesTheEightFactsOnItsStatePath() throws {
    let c = try fixtureCore()
    let all = try #require(try json(c, "message/msg-01/facts") as? [String: Bool])
    #expect(Set(all.keys) == ["unread", "unanswered", "has_attachment", "starred", "vip_sender", "cc_me", "mailing_list", "replied_by_me"])
    #expect(all["has_attachment"] == true && all["unread"] == true && all["starred"] == false && all["replied_by_me"] == false)
    #expect(all["cc_me"] == false, "msg-01 names the owner in To as well as Cc: not a Cc-only copy")
    #expect(try json(c, "message/msg-09/facts/mailing_list") as? Bool == true, "msg-09 carries a List-Unsubscribe header")
    #expect(try json(c, "message/msg-01/facts/mailing_list") as? Bool == false)
    _ = try c.run(command: .star(messageIds: ["msg-01"], starred: true))
    #expect(try json(c, "message/msg-01/facts/starred") as? Bool == true)
    #expect(c.state(path: "message/msg-01/facts/not_a_fact") == nil || c.state(path: "message/msg-01/facts/not_a_fact") == "null")
}

// Gap 3. Mutant: fixtureReply flips the facts but does not ask the exit filter again, or flips only one of the two facts.
@Test func fixtureReplyFlipsTheFactsAndMovesTheMessageToTheFallbackAtOnce() throws {
    let c = try fixtureCore()
    let before = try #require(try json(c, "message/flow-kept-1/facts") as? [String: Bool])
    #expect(before["unanswered"] == true && before["replied_by_me"] == false)
    #expect((try json(c, "message/flow-kept-1/why/needs-reply") as? [String: Any])?["card"] as? String == "exit")
    let r = try c.fixtureReply(messageId: "flow-kept-1")
    #expect(r.repliedByMe == true && r.unanswered == false && r.endedIn == nil, "answered: the exit filter (unanswered: yes) rejects it, Primary holds no label")
    let after = try #require(try json(c, "message/flow-kept-1/facts") as? [String: Bool])
    #expect(after["unanswered"] == false && after["replied_by_me"] == true)
    #expect((try json(c, "message/flow-kept-1/why/primary") as? [String: Any])?["card"] as? String == "fallback", "re-run at once, no other command")
    #expect((try json(c, "message/flow-kept-1/flow_rejection") as? [String: Any])?["sent_to"] as? String == "primary")
    // flipping again brings it back through the same filter
    let back = try c.fixtureReply(messageId: "flow-kept-1")
    #expect(back.repliedByMe == false && back.unanswered == true && back.endedIn == "needs-reply")
    #expect(c.state(path: "message/flow-kept-1/flow_rejection") == nil || c.state(path: "message/flow-kept-1/flow_rejection") == "null")
    #expect(throws: CoreError.self) { try c.fixtureReply(messageId: "no-such-message") }
}

// The À répondre rows: every card has a message (SPEC A140: the entry chain picks, the AI accepts, the exit filter keeps or sends to Primary).
// Mutant: the flow_example rows are not seeded or filed, or step 1 files a mail itself.
@Test func theNeedsReplyLabelHasRowsThatExerciseEveryCard() throws {
    let c = try fixtureCore()
    func card(_ id: String, _ label: String = "needs-reply") throws -> String? { (try json(c, "message/\(id)/why/\(label)") as? [String: Any])?["card"] as? String }
    // A140: every example row was picked by step 1 and accepted by the AI, so a mail the exit filter keeps carries the exit card (the old test had flow-entry-1 on the entry card)
    #expect(try card("flow-entry-1") == "exit" && card("flow-kept-1") == "exit" && card("flow-kept-2") == "exit" && card("flow-kept-3") == "exit")
    #expect(try card("flow-rejected-1", "primary") == "fallback")
    let why = try #require(try json(c, "message/flow-kept-1/why/needs-reply") as? [String: Any])
    #expect(why["ai"] as? String == "yes", "the AI accepted it")
    #expect((why["entry"] as? [String: Any])?["chain_id"] as? Int == 0, "step 1's chain 0 picked it")
    // edit the flow (the served example gives way to the derived figures): the figures are the file's own flow_example rows (Tests/Support/FixtureFileFigures.swift),
    // never typed here: the AI's accepts (kept + rejected), the kept rows, the rejected rows that go to Primary; step 1 picks the 32 rows and every corpus mail that
    // opens "Hi Assem", and the corpus mails wait for the AI
    _ = try c.run(command: .setLabelExitFallback(labelId: "needs-reply", fallback: "primary"))
    let file = try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"))) as? [String: Any])
    let rows = try #require(file["flow_example"] as? [[String: Any]])
    let corpus = try #require(file["messages"] as? [[String: Any]])
    let waiting = corpus.filter { (($0["body"] as? [String]) ?? []).joined(separator: " ").lowercased().contains("hi assem") }.count
    let counts = try #require(try json(c, "label/needs-reply/flow/counts") as? [String: Int])
    #expect(counts["caught_by_entry"] == rows.count + waiting)
    #expect(counts["waiting_for_ai"] == waiting)
    let accepted = try FixtureFileFigures.flowSortedByAi(), kept = try FixtureFileFigures.flowKeptByExit(), sent = try FixtureFileFigures.flowSentToFallback()
    #expect(counts["sorted_by_ai"] == accepted)
    #expect(counts["kept_by_exit"] == kept)
    #expect(counts["sent_to_fallback"] == sent)
}

// A140: a re-run asks step 1, then the AI, then step 3 again; it files nothing by step 1 alone while the AI is on.
@Test func aRerunOfTheNeedsReplyLabelFollowsThePipeline() throws {
    let c = try fixtureCore()
    _ = try c.run(command: .rerunLabel(labelId: "needs-reply"))
    let counts = try #require(try json(c, "label/needs-reply/flow/counts") as? [String: Int])
    #expect(counts["waiting_for_ai"] == 0, "the AI has ruled on every picked mail")
}
