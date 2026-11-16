import Foundation
import Testing
@testable import WhooshUI

// The label inspector's counts while changes are not saved (lane label-counts, words of lane flow3-swift, SPEC A140): each card and the foot read the figures of the
// one proof the preview list reads ("48 join · 0 leaves · 0 stay", with the AI on "48 picked · the AI rules on each after Save"). Pure values only: no core, no window.
// The amber line ("The AI will see none of them") is gone: LabelPipelineTests.noWarningRowExistsAnyMore.

private func row(_ n: Int) -> ProofRow {
    ProofRow(id: "m\(n)", senderName: "Sender", subject: "Subject \(n)", accountId: "a", receivedAtMs: Int64(n), unread: false, threadCount: 1, tag: "Unanswered", edge: .join)
}

private func proof(join: Int, leave: Int = 0, preview: Bool = true, engine: EnginePreview? = nil) -> LabelProof {
    var p = LabelProof()
    p.hasPreview = preview
    p.join = (0..<join).map(row)
    p.leave = (0..<leave).map(row)
    p.engine = engine
    return p
}

private func counts(entry: Int = 0, ai: Int = 0, kept: Int = 0, sent: Int = 0) -> FlowCounts {
    var c = FlowCounts(); c.entry = entry; c.ai = ai; c.kept = kept; c.sent = sent; return c
}

private func draft(fact: MessageFact? = nil, state: FactState = .yes, aiOn: Bool = true, entryOn: Bool = true) -> LabelDraft {
    var d = LabelDraft()
    d.entry.rows = []
    if let fact, let i = ChainDraft.order.firstIndex(of: fact) { d.entry.facts[i] = state }
    d.flow.aiOn = aiOn; d.flow.entryOn = entryOn
    return d
}

private func after(_ c: FlowCounts?, _ p: LabelProof, _ d: LabelDraft, dirty: Bool = true, isNew: Bool = false, aiOn: Bool = false) -> CountsAfter? {
    CountsAfter.of(counts: c, proof: p, draft: d, dirty: dirty, isNew: isNew, aiOn: aiOn, exitOn: false)
}

// Old code: the count text was "catches 0".
@Test func noUnsavedChangeGivesNoAfterFigureAndTheCardsStayAsToday() {
    let p = proof(join: 48)
    #expect(after(counts(), p, draft(fact: .unanswered), dirty: false) == nil)
    #expect(after(counts(), proof(join: 0, preview: false), draft(fact: .unanswered)) == nil)   // name or colour changed only: no preview
    #expect(after(nil, p, draft(fact: .unanswered)) == nil)
    #expect(after(counts(), p, draft(fact: .unanswered), isNew: true) == nil)
    LanguagePin.with("en") { #expect(Words.text("picks %lld", 0) == "picks 0") }   // A140: step 1 picks
}

// Old code: the entry card says "catches 0" and nothing else while the list says 48 join. With the AI on the foot says "the AI rules on 48", with it off "48 by rule".
@Test func anUnsavedEntryRuleThatMatchesNMessagesSaysPicksSavedToNAfterSave() throws {
    let a = try #require(after(counts(entry: 0), proof(join: 48), draft(fact: .unanswered)))
    #expect(a.entry == 48)
    try LanguagePin.with("en") {
        let entry = try #require(a.entry)   // a `try` inside #expect does not compile: the value is taken first
        #expect(Words.text("%lld after Save", entry) == "48 after Save")
        #expect(a.now == 0)
        #expect(Words.text("Now %lld → %lld by rule", a.now, 48) == "Now 0 → 48 by rule")
        #expect(Words.text("Now %lld → the AI rules on %lld", a.now, 48) == "Now 0 → the AI rules on 48")
    }
}

// Old code: the foot says "0 by rule" while the list says 48 join; the AI figure is never guessed.
@Test func theAiFigureIsShownOnlyWhenTheEngineKnowsIt() throws {
    let r1 = try #require(after(counts(), proof(join: 48), draft(fact: .unanswered)))
    #expect(r1.ai == nil)
    var e = try #require(EnginePreview(#"{"by_rule":48,"by_ai":3,"leave":0,"stay":0,"beyond":0}"#))
    e.aiWords = nil
    let a = try #require(after(counts(ai: 5), proof(join: 48, engine: e), draft(fact: .unanswered)))
    #expect(a.ai == 8)
    #expect(a.entry == 48)
}

// Old code: the cards had no second figure to compare with the list.
@Test func theCardFiguresEqualThePreviewListFiguresForThreeDrafts() throws {
    let cases: [(saved: Int, join: Int, leave: Int)] = [(0, 48, 0), (10, 3, 2), (7, 0, 0)]
    for k in cases {
        let p = proof(join: k.join, leave: k.leave)
        let a = try #require(after(counts(entry: k.saved), p, draft(fact: .unanswered)))
        // the list line reads p.joinCount / p.leaveCount; the card is the saved figure plus the same joins minus the same leaves
        #expect(a.entry == k.saved + p.joinCount - p.leaveCount)
        #expect(p.joinCount == k.join)
        LanguagePin.with("en") {
            #expect(Words.text("%lld join · %lld leaves · %lld stay", p.joinCount, p.leaveCount, p.stayCount).hasPrefix("\(k.join) join"))
        }
    }
}

// The language path, not avoided: the same lines in French read the French catalogue and the French number format. Fails if a line ignores the
// chosen language (English text) or if the figure is not in the sentence.
@Test func theSameLinesInFrenchReadTheFrenchCatalogueAndTheFrenchFigures() throws {
    let p = proof(join: 3, leave: 2)
    try LanguagePin.with("fr") {
        let a = try #require(after(counts(entry: 0), proof(join: 48), draft(fact: .unanswered)))
        #expect(Words.text("picks %lld", 0) == "retient 0")
        let entryFr = try #require(a.entry)
        #expect(Words.text("%lld after Save", entryFr) == "48 après Enregistrer")
        #expect(Words.text("Now %lld → %lld by rule", a.now, 48) == "Maintenant 0 → 48 par règle")
        #expect(Words.text("%lld join · %lld leaves · %lld stay", p.joinCount, p.leaveCount, p.stayCount) == "3 rejoignent · 2 quittent · 0 restent")
        #expect(Words.text("%lld picked · the AI rules on each after Save", 48) == "48 retenus · l'IA tranche pour chacun après Enregistrer")
    }
}
