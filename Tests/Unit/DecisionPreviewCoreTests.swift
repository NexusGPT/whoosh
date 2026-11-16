import Foundation
import Testing
@testable import WhooshUI

// gap-preview-core: the Decisions preview asks the core with the list the page holds and draws what the core answers (DecisionPreview.swift, AISettingsStore.runPreview,
// FixtureDecisionPreview.swift). Written without a compile; never run by the author. Broken copies (each seen red by the lead's run):
//  - DecisionPreview.swift `entries(of:)`: `enabled: e.enabled` -> `enabled: true` -> thePageSendsItsCurrentListUntickedEntryIncluded, anUntickedEntryDecidesNoRow
//  - DecisionPreview.swift `entries(of:)`: `question: e.kind == .model ? e.text : ""` -> `question: ""` -> anEditedQuestionIsTheOneTheCoreIsSent
//  - DecisionPreview.swift `rows(from:)`: `decidedBy: $0.decidedBy` -> `decidedBy: ""` -> theRowsAreDrawnFromTheCoresAnswer
//  - AISettingsStore.swift `runPreview`: the old line back (`DecisionPreview.run`-style local rows, no `previewDecisionList`) -> theRunSendsTheCommandAndKeepsTheCoresRows
//  - FixtureDecisionPreview.swift `rows`: `where e.enabled` removed -> anUntickedEntryDecidesNoRow

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func fixtureCore() throws -> FixtureCore {
    try FixtureCore(url: repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}
private func page(_ id: String) throws -> DecisionPage { try #require(DecisionCatalog.shipped().first { $0.id == id }) }

@MainActor @Test func thePageSendsItsCurrentListUntickedEntryIncluded() throws {
    var p = try page("which-tab")
    p.entries[0].enabled = false
    let sent = DecisionPreview.entries(of: p)
    #expect(sent.count == p.entries.count)                       // every entry goes, ticked or not
    #expect(sent[0].enabled == false && sent[1].enabled == true)
    #expect(sent[0].kind == .rule && sent[0].matchKey == "vip")
    #expect(sent.last?.kind == .otherwise)
    let model = try #require(p.entries.firstIndex { $0.kind == .model })
    #expect(sent[model].kind == .model && sent[model].trust == 80)
}

@MainActor @Test func anEditedQuestionIsTheOneTheCoreIsSent() throws {
    var p = try page("which-tab")
    let at = try #require(p.entries.firstIndex { $0.kind == .model })
    p.entries[at].text = "Is this a bill?"
    #expect(DecisionPreview.entries(of: p)[at].question == "Is this a bill?")
    #expect(DecisionPreview.entries(of: p).enumerated().allSatisfy { $0.offset == at || $0.element.question.isEmpty })
}

@MainActor @Test func theRowsAreDrawnFromTheCoresAnswer() {
    let verdicts = [
        DecisionVerdict(messageId: "a", entry: 2, outcome: "Updates", differs: true, decidedBy: "entry 2 · the model said Updates (0.91)", sender: "GitHub", subject: "PR 42"),
        DecisionVerdict(messageId: "b", entry: nil, outcome: "—", differs: false, decidedBy: "no entry answered", sender: "Me", subject: "Notes"),
    ]
    let rows = DecisionPreview.rows(from: verdicts)
    #expect(rows == [
        PreviewRow(id: "a", sender: "GitHub", subject: "PR 42", outcome: "Updates", entry: 2, decidedBy: "entry 2 · the model said Updates (0.91)", differs: true),
        PreviewRow(id: "b", sender: "Me", subject: "Notes", outcome: "—", entry: 0, decidedBy: "no entry answered", differs: false),
    ])
}

@MainActor @Test func anUntickedEntryDecidesNoRow() throws {
    let c = try fixtureCore()
    var p = try page("which-tab")
    _ = try c.run(command: .previewDecisionList(pageId: p.id, entries: DecisionPreview.entries(of: p)))
    #expect(c.decisionPreview().contains { $0.entry == 1 }, "ticked: the VIP rule decides some rows")
    p.entries[0].enabled = false
    _ = try c.run(command: .previewDecisionList(pageId: p.id, entries: DecisionPreview.entries(of: p)))
    let rows = c.decisionPreview()
    #expect(rows.count == Int(WhooshConstants.DECISION_PREVIEW_N))
    #expect(!rows.contains { $0.entry == 1 }, "unticked: entry 1 decides nothing")
    #expect(rows.allSatisfy { !$0.decidedBy.isEmpty && !$0.outcome.isEmpty })
}

@MainActor @Test func aBlankQuestionAsksNothingInTheTestEngine() throws {
    let c = try fixtureCore()
    var p = try page("which-tab")
    _ = try c.run(command: .previewDecisionList(pageId: p.id, entries: DecisionPreview.entries(of: p)))
    #expect(c.decisionPreview().contains { $0.decidedBy.contains("the model said") })
    let at = try #require(p.entries.firstIndex { $0.kind == .model })
    p.entries[at].text = ""
    _ = try c.run(command: .previewDecisionList(pageId: p.id, entries: DecisionPreview.entries(of: p)))
    #expect(!c.decisionPreview().contains { $0.decidedBy.contains("the model said") }, "the edit changed the rows")
}

@MainActor @Test func anUnknownPageIsRefusedInWords() throws {
    let c = try fixtureCore()
    #expect(throws: CoreError.self) { _ = try c.run(command: .previewDecisionList(pageId: "nope", entries: [])) }
}

@MainActor @Test func theRunSendsTheCommandAndKeepsTheCoresRows() async throws {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    setenv("WHOOSH_FIXTURE_PATH", repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()
    let store = AISettingsStore.shared
    store.previews = [:]
    store.section = .page("which-tab")      // the preview runs off the main actor and lands only on the open page (gap-decisions-real)
    store.runPreview(page: "which-tab")
    await store.settlePreview(page: "which-tab")
    let rows = try #require(store.previews["which-tab"])
    #expect(rows.count == Int(WhooshConstants.DECISION_PREVIEW_N))
    #expect(rows == DecisionPreview.rows(from: m.core.decisionPreview()), "the page draws exactly what the core kept")
    #expect(store.previewNotes["which-tab"] != nil, "the core's sentence is kept for the page")
}
