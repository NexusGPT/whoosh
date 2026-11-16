import Foundation
import Testing
@testable import WhooshUI

// gap-decisions: `Command.saveDecisionList` in the test engine (FixtureDecisionList.swift) and the doors of a decision entry (SettingsAIMacFeature.clickTargets).
// Written without a compile; never run by the author. Broken copies (each seen red by the lead's run):
//  - FixtureDecisionList.swift: the chain is never written (`if false && !chains.contains`) -> aRuleOfTheOwnerFilesUnderANestedNodeWithItsAiCardOff
//  - FixtureDecisionList.swift: the held-chain test dropped (`!chains.contains(...)` -> `true`) -> aRuleTheNodeAlreadyHoldsIsNotWrittenTwice
//  - FixtureDecisionList.swift: the label check dropped (`_ = try labelRow(id)` removed) -> aBadDecisionListIsRefusedAndWritesNothing
//  - SettingsAIMacFeature.swift: the `node.<k>` doors not registered -> theEntryDoorsExistForTheSwitchTheQuestionAndEveryTreeNode
//  - AISettingsStore.swift `condition`: "from:" answers nil -> theLabelsRuleReachesTheCoreAsACondition

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func fixtureCore() throws -> FixtureCore {
    try FixtureCore(url: repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}
private func spec(_ name: String, parent: String?) -> LabelSpec {
    LabelSpec(name: name, parentId: parent, colour: nil, description: "", scope: .all, rules: RuleSet(conditions: [], matchMode: .all), pinned: false)
}
/// Work > Clients with the AI card of Clients off: a plain folder of the tree, nested one level down.
private func nestedNode(_ c: FixtureCore) throws -> String {
    let work = try #require(try c.run(command: .createLabelFull(spec: spec("Work-\(UUID().uuidString.prefix(6))", parent: nil))).createdId)
    let clients = try #require(try c.run(command: .createLabelFull(spec: spec("Clients", parent: work))).createdId)
    _ = try c.run(command: .setLabelCard(labelId: clients, card: .ai, enabled: false))
    return clients
}
private func rule(_ words: String) -> RuleSet { RuleSet(conditions: [.subjectContains(words: words)], matchMode: .any) }
private func chainCount(_ c: FixtureCore, _ id: String) -> String? { c.state(path: "label/\(id)/chains/entry/count") }

// Old code: no command wrote a decision list, so the node's filed count never moved after Apply.
@Test func aRuleOfTheOwnerFilesUnderANestedNodeWithItsAiCardOff() throws {
    let c = try fixtureCore()
    let node = try nestedNode(c)
    _ = try c.deliverArrival(id: "dl-a", account: "acc-01", from: "x@dl-example.test", sender: "X", subject: "Quokkas villa", body: "plain")
    _ = try c.deliverArrival(id: "dl-b", account: "acc-01", from: "x@dl-example.test", sender: "X", subject: "Something else", body: "plain")
    #expect(c.labelRerunCount(labelId: node) == 0, "no rule yet: nothing is filed under the nested node")
    _ = try c.run(command: .saveDecisionList(pageId: "labels", nodeIds: [node], rules: [rule("quokkas")]))
    #expect(c.labelRerunCount(labelId: node) == 1, "the mail the rule catches is filed under the node it names")
    #expect(c.state(path: "message/dl-a/label/\(node)/decided_by") != nil, "and the message says it is filed there")
    #expect(chainCount(c, node) == "1")
}

@Test func aRuleTheNodeAlreadyHoldsIsNotWrittenTwice() throws {
    let c = try fixtureCore()
    let node = try nestedNode(c)
    for _ in 0..<2 { _ = try c.run(command: .saveDecisionList(pageId: "labels", nodeIds: [node], rules: [rule("quokkas")])) }
    #expect(chainCount(c, node) == "1")
    _ = try c.run(command: .saveDecisionList(pageId: "labels", nodeIds: [node], rules: [rule("zebrafish")]))
    #expect(chainCount(c, node) == "2", "another rule on the node is a second chain")
}

@Test func aBadDecisionListIsRefusedAndWritesNothing() throws {
    let c = try fixtureCore()
    let node = try nestedNode(c)
    #expect(throws: CoreError.self) { _ = try c.run(command: .saveDecisionList(pageId: "labels", nodeIds: [node, "no-such-label"], rules: [rule("a"), rule("b")])) }
    #expect(chainCount(c, node) == nil || chainCount(c, node) == "0", "an unknown node and the good one is not written either")
    #expect(throws: CoreError.self) { _ = try c.run(command: .saveDecisionList(pageId: "which-tab", nodeIds: [node], rules: [rule("a")])) }
    #expect(throws: CoreError.self) { _ = try c.run(command: .saveDecisionList(pageId: "labels", nodeIds: [node], rules: [])) }
    #expect(throws: CoreError.self) { _ = try c.run(command: .saveDecisionList(pageId: "labels", nodeIds: [node], rules: [RuleSet(conditions: [], matchMode: .any)])) }
}

@MainActor @Test func theEntryDoorsExistForTheSwitchTheQuestionAndEveryTreeNode() {
    let doors = SettingsAIMacFeature.clickTargets()
    for id in ["which-tab", "needs-reply", "labels"] {
        #expect(doors["mac-settings-ai.decisions.page.\(id).entry.1.enabled"] != nil, "the switch of entry 1 on \(id)")
        #expect(doors["mac-settings-ai.decisions.page.\(id).entry.3.question"] != nil, "the question of entry 3 on \(id)")
    }
    #expect(doors["mac-settings-ai.decisions.page.labels.entry.1.node.1"] != nil)
    #expect(doors["mac-settings-ai.decisions.page.which-tab.entry.1.node.1"] == nil, "only the Labels page picks a node")
}

@MainActor @Test func theLabelsRuleReachesTheCoreAsACondition() {
    #expect(AISettingsStore.condition("from:provanta") == .from(words: "provanta"))
    #expect(AISettingsStore.condition("subject:invoice") == .subjectContains(words: "invoice"))
    #expect(AISettingsStore.condition("vip") == .fact(fact: .vipSender, wanted: true))
    #expect(AISettingsStore.condition("me") == nil, "a key the core has no condition for is left out")
    #expect(AISettingsStore.condition(nil) == nil)
}
