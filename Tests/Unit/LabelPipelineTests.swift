import Foundation
import Testing
@testable import WhooshUI

// A label is a pipeline of three steps (SPEC A140, lane flow3-swift): step 1 (Entry rules) PICKS mails, step 2 (AI sorting) is asked only about the picked mails
// and rules on each, step 3 (Exit filter) applies to the mails the AI accepted. The test engine (FixtureCore) and the words of the three cards.
// Each test names what it fails on in the old code (step 1 filed the mail directly and the AI never read it). Written without a compile; never run by the author.

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func fixtureCore() throws -> FixtureCore {
    try FixtureCore(url: repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}
private func json(_ core: FixtureCore, _ path: String) throws -> Any {
    let text = try #require(core.state(path: path), "no state at \(path)")
    return try JSONSerialization.jsonObject(with: Data(text.utf8), options: [.fragmentsAllowed])
}
private func counts(_ core: FixtureCore, _ id: String) throws -> [String: Int] {
    try #require(try json(core, "label/\(id)/flow/counts") as? [String: Int])
}
private func spec(_ name: String, meaning: String, rules: [RuleCondition]) -> LabelSpec {
    LabelSpec(name: name, parentId: nil, colour: nil, description: meaning, scope: .all, rules: RuleSet(conditions: rules, matchMode: .all), pinned: false)
}
private func makeLabel(_ core: FixtureCore, _ name: String, meaning: String, rules: [RuleCondition]) throws -> String {
    try #require(try core.run(command: .createLabelFull(spec: spec(name, meaning: meaning, rules: rules))).createdId)
}
private let domain = "billing-pipe.example"

// Old code: the mail matches the entry rule, so the rule files it at once and "Quarterly figures" is in the label. Now step 1 only picks it and the AI,
// which finds none of the description's words in it, does not accept it.
@Test func aMailStepOnePicksIsNotInTheLabelUntilTheAiAccepts() throws {
    let c = try fixtureCore()
    let id = try makeLabel(c, "Pipe invoices", meaning: "quokkas zebrafish marmots", rules: [.domain(domain: domain)])
    let notAccepted = try c.deliverArrival(id: "pipe-a", account: "acc-01", from: "x@\(domain)", sender: "Billing", subject: "Quarterly figures", body: "see the attachment")
    #expect(notAccepted == [], "picked by step 1, not accepted by the AI: not in the label")
    let accepted = try c.deliverArrival(id: "pipe-b", account: "acc-01", from: "x@\(domain)", sender: "Billing", subject: "Quokkas 77", body: "a zebrafish marmots")
    #expect(accepted == [id], "picked by step 1 and accepted by the AI: in the label")
    #expect(c.state(path: "message/pipe-b/label/\(id)/decided_by") == "\"laya\"", "the AI decided it, not the rule")
    let n = try counts(c, id)
    #expect(n["caught_by_entry"] == 2, "step 1 picked both")
    #expect(n["sorted_by_ai"] == 1, "the AI accepted one")
    #expect(n["waiting_for_ai"] == 0, "the AI has ruled on both: it said no to one")
}

// Old code: a mail no entry rule catches is offered to every described label, so the label is chosen for a mail its entry rules did not pick.
@Test func theAiIsAskedOnlyAboutPickedMails() throws {
    let c = try fixtureCore()
    let gated = try makeLabel(c, "Pipe gated", meaning: "quokkas zebrafish marmots", rules: [.domain(domain: domain)])
    let open = try makeLabel(c, "Pipe open", meaning: "quokkas", rules: [])
    // from another domain: step 1 of the gated label does not pick it, so only the open label (no entry chain: every mail passes) is offered to the AI
    let labels = try c.deliverArrival(id: "pipe-c", account: "acc-01", from: "x@elsewhere.example", sender: "Other", subject: "Quokkas zebrafish", body: "quokkas")
    #expect(labels == [open])
    #expect(!labels.contains(gated))
}

// Old code: nothing records that step 1 picked mails the AI has not ruled on: caught_by_entry is 0 and the key waiting_for_ai does not exist.
@Test func picksArePickedAndWaitingBeforeTheAiHasRuled() throws {
    let c = try fixtureCore()
    let id = try makeLabel(c, "Pipe waiting", meaning: "quokkas zebrafish marmots", rules: [.fact(fact: .unread, wanted: true)])
    let n = try counts(c, id)
    let picked = try #require(n["caught_by_entry"])
    #expect(picked > 0, "the fixture inbox holds unread mail: step 1 picks it")
    #expect(n["waiting_for_ai"] == picked, "no mail is ruled yet")
    #expect(n["sorted_by_ai"] == 0 && n["kept_by_exit"] == 0)
    let held = try c.deliverArrival(id: "pipe-d", account: "acc-01", from: "z@example.com", sender: "Z", subject: "Marmots", body: "quokkas zebrafish")
    #expect(held == [id], "unread, picked, and the description fits: the AI accepts")
}

// With the AI off, step 1 alone files the mail (this held before too: the control of the three tests above).
@Test func withTheAiOffStepOneAloneFilesTheMail() throws {
    let c = try fixtureCore()
    let id = try makeLabel(c, "Pipe rule only", meaning: "quokkas zebrafish marmots", rules: [.domain(domain: domain)])
    _ = try c.run(command: .setLabelCard(labelId: id, card: .ai, enabled: false))
    let labels = try c.deliverArrival(id: "pipe-e", account: "acc-01", from: "x@\(domain)", sender: "Billing", subject: "Quarterly figures", body: "nothing about the description")
    #expect(labels == [id])
    #expect(c.state(path: "message/pipe-e/label/\(id)/decided_by") == "\"rule:rule-\(id)\"")
    let n = try counts(c, id)
    #expect(n["caught_by_entry"] == 1 && n["sorted_by_ai"] == 0 && n["waiting_for_ai"] == 0)
}

// A label with no entry chain and the AI off files nothing, even for a mail its description fits.
@Test func aLabelWithNoEntryChainAndTheAiOffFilesNothing() throws {
    let c = try fixtureCore()
    let id = try makeLabel(c, "Pipe nothing", meaning: "quokkas zebrafish marmots", rules: [])
    _ = try c.run(command: .setLabelCard(labelId: id, card: .ai, enabled: false))
    let labels = try c.deliverArrival(id: "pipe-f", account: "acc-01", from: "x@example.com", sender: "X", subject: "Quokkas", body: "zebrafish marmots")
    #expect(labels == [])
}

// Old code: the exit filter ran only on a mail the AI picked with no entry rule in front; step 3 must reject what the AI accepted and send it to the fallback.
@Test func aMailTheExitFilterRejectsGoesToTheFallback() throws {
    let c = try fixtureCore()
    let id = try makeLabel(c, "Pipe exit", meaning: "quokkas zebrafish marmots", rules: [.domain(domain: domain)])
    _ = try c.run(command: .setLabelExitFallback(labelId: id, fallback: "primary"))
    _ = try c.run(command: .addLabelChain(labelId: id, placement: .exit, chain: RuleSet(conditions: [.fact(fact: .starred, wanted: true)], matchMode: .all)))
    _ = try c.run(command: .setLabelCard(labelId: id, card: .exit, enabled: true))
    let labels = try c.deliverArrival(id: "pipe-g", account: "acc-01", from: "x@\(domain)", sender: "Billing", subject: "Quokkas 78", body: "zebrafish marmots")
    #expect(labels == [], "the AI accepted it, the exit filter (starred: yes) rejects it: no label holds it")
    let rejection = try #require(try json(c, "message/pipe-g/flow_rejection") as? [String: Any])
    #expect(rejection["sent_to"] as? String == "primary" && rejection["ai_label_id"] as? String == id)
    let n = try counts(c, id)
    #expect(n["caught_by_entry"] == 1 && n["sorted_by_ai"] == 1 && n["kept_by_exit"] == 0 && n["sent_to_fallback"] == 1)
}

// The why tag names step 1's condition and the AI's answer: "Is unread: yes · AI: yes" (a fact is written "Name: yes", as everywhere in the why tag).
@Test func theWhyTagNamesStepOneAndTheAiAnswer() throws {
    let c = try fixtureCore()
    let id = try makeLabel(c, "Pipe why", meaning: "quokkas zebrafish marmots", rules: [.fact(fact: .unread, wanted: true)])
    _ = try c.deliverArrival(id: "pipe-h", account: "acc-01", from: "q@example.com", sender: "Q", subject: "Quokkas", body: "zebrafish marmots")
    let why = try #require(try json(c, "message/pipe-h/why/\(id)") as? [String: Any])
    #expect(why["card"] as? String == "ai" && why["ai"] as? String == "yes")
    let entry = try #require(why["entry"] as? [String: Any], "step 1's own reading is there")
    #expect((entry["matched"] as? [[String: Any]])?.count == 1)
    LanguagePin.with("en") {
        let lines = LabelWhy.lines(core: c, label: id, message: "pipe-h")
        #expect(lines.count == 2 && lines[1] == "Is unread: yes · AI: yes")
    }
}

// The three cards' words (the screen and the control socket read these strings once, from the model).
@MainActor private func model(counts c: FlowCounts, proof p: LabelProof, aiOn: Bool) -> LabelInspectorModel {
    let m = LabelInspectorModel()
    m.mode = .modify("pipe-card")
    var saved = LabelDraft(); saved.flow.aiOn = aiOn
    m.saved = saved
    var d = saved; d.meaning = "changed"
    m.draft = d; m.counts = c; m.proof = p
    return m
}
private func flowCounts(entry: Int = 0, ai: Int = 0, kept: Int = 0, sent: Int = 0) -> FlowCounts {
    var c = FlowCounts(); c.entry = entry; c.ai = ai; c.kept = kept; c.sent = sent; return c
}
private func joinProof(_ n: Int) -> LabelProof {
    var p = LabelProof(); p.hasPreview = true
    p.join = (0..<n).map { ProofRow(id: "m\($0)", senderName: "S", subject: "T", accountId: "a", receivedAtMs: Int64($0), unread: false, threadCount: 1, tag: "Is unread", edge: .join) }
    return p
}

// Old code: the entry card said "catches 5", the AI card "31 messages", the foot "Now 0 → 48 by rule".
@MainActor @Test func theCardsSayPicksAcceptedAndTheAiRulesOn() {
    let m = model(counts: flowCounts(entry: 5, ai: 5), proof: joinProof(48), aiOn: true)
    LanguagePin.with("en") {
        #expect(m.entryCountText == "picks 5")
        #expect(m.aiCountText == "5 accepted")
        #expect(m.nowAfterLine() == "Now 5 → the AI rules on 53")
        #expect(LabelsWords.accepted(31, waiting: 17) == "31 accepted · 17 waiting for the AI")
        #expect(ProofRowView.word(.join, aiOn: true) == "picked" && ProofRowView.word(.join, aiOn: false) == "joins")
        #expect(Words.text("%lld picked · the AI rules on each after Save", 48) == "48 picked · the AI rules on each after Save")
    }
    let off = model(counts: flowCounts(entry: 5, ai: 0), proof: joinProof(48), aiOn: false)
    LanguagePin.with("en") { #expect(off.nowAfterLine() == "Now 5 → 53 by rule") }   // AI off: unchanged
}

// The exit card's sentence: M is what the AI accepted when the AI card is on, what step 1 picked when it is off (SPEC A140).
@MainActor @Test func theExitCardSaysKeepsNOfMWithMTheAcceptedOrThePicked() {
    let on = model(counts: flowCounts(entry: 7, ai: 5, kept: 3, sent: 2), proof: joinProof(0), aiOn: true)
    let off = model(counts: flowCounts(entry: 7, ai: 0, kept: 3, sent: 0), proof: joinProof(0), aiOn: false)
    LanguagePin.with("en") {
        #expect(on.exitCountText == "keeps 3 of 5", "AI on: of the mails the AI accepted")
        #expect(off.exitCountText == "keeps 3 of 7", "AI off: of the mails step 1 picked")
    }
}

// The words the three steps added and the exit card's own sentence, one by one: each is in the 14 languages, filled, and keeps its placeholders.
@Test func everyThreeStepSentenceIsInAll14Languages() throws {
    let keys = ["picks %lld", "picks the mails the AI reads", "these mails go straight in",
                "These rules run first. The AI reads only the mails they pick.", "These rules file the mail. AI sorting is off for this label.",
                "%lld accepted", "%lld accepted · %lld waiting for the AI", "picked", "%lld picked · the AI rules on each after Save",
                "Now %lld → the AI rules on %lld", "AI: yes", "%lld picked", "entry %lld · rule · %@", "keeps %lld of %lld"]
    let languages = ["da", "de", "en", "es", "fi", "fr", "it", "ja", "nb", "nl", "pl", "pt", "sv", "zh-Hans"]
    let url = repoRoot.appendingPathComponent("Sources/Resources/Localizable.xcstrings")
    let doc = try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
    let strings = try #require(doc["strings"] as? [String: Any])
    func value(_ key: String, _ code: String) -> String? {
        let entry = strings[key] as? [String: Any]
        let loc = (entry?["localizations"] as? [String: Any])?[code] as? [String: Any]
        return (loc?["stringUnit"] as? [String: Any])?["value"] as? String
    }
    for key in keys {
        let english = try #require(value(key, "en"), "\(key) has an English value")
        let marks = english.filter { $0 == "%" }.count
        for code in languages {
            let v = try #require(value(key, code), "\(key) has no \(code)")
            #expect(!v.isEmpty, "\(key) is empty in \(code)")
            #expect(v.filter { $0 == "%" }.count == marks, "\(key) in \(code) lost a placeholder")
        }
    }
}

// Old code: the amber sentence "This rule files every unread mail. The AI will see none of them." existed, in the catalogue and as `entryWarning`.
@MainActor @Test func noWarningRowExistsAnyMore() throws {
    let m = model(counts: flowCounts(), proof: joinProof(48), aiOn: true)
    LanguagePin.with("en") { #expect(m.nowAfterLine() == "Now 0 → the AI rules on 48", "the foot says the AI rules on the picked mails, no warning") }
    let url = repoRoot.appendingPathComponent("Sources/Resources/Localizable.xcstrings")
    let doc = try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
    let keys = try #require(doc["strings"] as? [String: Any]).keys
    #expect(!keys.contains { $0.contains("The AI will see none of them") }, "the amber sentences are gone from the catalogue")
}

// The language path: the same new words in French come from the catalogue (an English fallback would equal the key).
@Test func theNewWordsAreInTheCatalogueInFrench() {
    LanguagePin.with("fr") {
        #expect(Words.text("picks %lld", 5) == "retient 5")
        #expect(Words.text("picks the mails the AI reads") == "retient les messages que l'IA lit")
        #expect(Words.text("these mails go straight in") == "ces messages entrent directement")
        #expect(Words.text("These rules run first. The AI reads only the mails they pick.") == "Ces règles passent d'abord. L'IA ne lit que les messages qu'elles retiennent.")
        #expect(Words.text("%lld accepted", 3) == "3 acceptés")
        #expect(Words.text("Now %lld → the AI rules on %lld", 0, 48) == "Maintenant 0 → l'IA tranche sur 48")
        #expect(Words.text("AI: yes") == "IA : oui")
    }
}
