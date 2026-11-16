import Foundation
import Testing
@testable import WhooshUI

// DecisionSeamTests: how Whoosh decides, over the core seam, as docs/DECISION-MODEL.md VERSION 2 (signed 2026-11-07 19:16).
// One page per decision; one ordered list per page mixing rule entries and "ask the model" entries; first entry that
// answers wins; every message carries a "why" naming the page and the entry. State paths are the lane's assumed wire
// (decision/list/<page>, decision/why/<message>), documented in docs/lanes/tests-j5.md until the decision crate names its own.
//
// NOT COVERED (hand-back): a test that feeds the core a DecisionPort stub (to prove Off never calls it, trust floors,
// the 2000-character body cut) needs a constructor that takes a DecisionPort. contracts.swift has the protocol and no
// way to inject one. Those rules are held by checks/decision-check.py against a stub engine on 127.0.0.1 until then.

private let pages = ["which-tab", "needs-reply", "sign-in-codes", "vips", "notify-me", "labels"]
private let titles = ["Which tab", "Needs my reply", "Sign-in codes", "VIPs", "Notify me", "Labels"]

private func list(_ core: WhooshCore, _ page: String) throws -> [[String: Any]] {
    let raw = try #require(core.state(path: "decision/list/\(page)"), "no decision list stored for \(page)")
    let obj = try JSONSerialization.jsonObject(with: Data(raw.utf8))
    return try #require(obj as? [[String: Any]])
}

@Test func everyDecisionPageEndsWithADefault() throws {
    let core = CoreHost.shared.core
    for p in pages {
        let kinds = try list(core, p).compactMap { $0["kind"] as? String }
        #expect(kinds.last == "default", "\(p) ends with \(String(describing: kinds.last))")
    }
}

@Test func whichTabShipsRuleRuleModelDefaultWithTheModelTrustedFrom80() throws {
    let core = CoreHost.shared.core
    let entries = try list(core, "which-tab")
    #expect(entries.compactMap { $0["kind"] as? String } == ["rule", "rule", "model", "default"])
    #expect(entries[2]["trust"] as? Int == 80)
    #expect(entries[2]["question"] as? String == "Which tab?")
}

@Test func signInCodesAsksTheModelOnlyWhenNoRuleMatched() throws {
    let core = CoreHost.shared.core
    #expect(try list(core, "sign-in-codes").compactMap { $0["kind"] as? String } == ["rule", "model", "default"])
    #expect(try list(core, "vips").compactMap { $0["kind"] as? String } == ["rule", "default"])  // no model entry shipped on VIPs
}

@Test func everyMessageCarriesAWhyNamingItsPageAndEntry() throws {
    let core = CoreHost.shared.core
    let rows = core.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil))
    #expect(!rows.isEmpty, "the inbox is empty, so no why can be checked")
    for r in rows.prefix(5) {
        let raw = try #require(core.state(path: "decision/why/\(r.messageId)"), "no why for \(r.messageId)")
        let why = raw.trimmingCharacters(in: CharacterSet(charactersIn: "\""))
        let page = titles.first { why.hasPrefix($0 + " · entry ") }
        #expect(page != nil, "why \(why) names no decision page")
        #expect(why.split(separator: "·").count >= 3, "why \(why) names no entry reason")
    }
}

@Test func theCoreReportsWhichEngineDecides() throws {
    // DecisionPort is compiled against here so a contract change breaks this file; the core cannot be handed a port yet (see NOT COVERED above).
    final class Anchor: DecisionPort, @unchecked Sendable {
        func provider() -> DecisionProviderKind { .local }
        func decide(state: DecisionState, questions: [DecisionQuestion]) throws -> DecisionResult { DecisionResult(answers: [], answeredBy: .local, checkpoint: nil) }
    }
    _ = Anchor()
    let core = CoreHost.shared.core
    let raw = try #require(core.state(path: "decision_model/provider"), "the core reports no decision engine")
    #expect(["\"local\"", "\"jev\"", "\"rules\""].contains(raw), "engine \(raw) is none of Local, Jev, Off")
}
