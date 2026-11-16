import Foundation
import Testing
@testable import WhooshUI

// Seven faults of build main81 (lane app-faults-4, docs/lanes/reds81-labels.md). Run with WHOOSH_FIXTURE_CORE=1 like the neighbours.
// Each test says in a comment what it reads on the old code. No window opens: the unit process has no scene, so `WindowBridge.open` answers false.

@MainActor private func app() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    try #require(m.rows.count >= 4, "the fixture inbox needs four rows")
    return m
}

private func result(_ row: InboxRow) -> SearchResult {
    SearchResult(row: row, score: 1, passage: "", highlights: [], marks: [], subjectMarks: [], attachmentName: nil, attachmentMarks: [], matchedIn: "subject")
}

// MARK: 1. flow-ai-thread-saved

@MainActor @Test func theCoreServesTheAiReadsWithTheRestOfTheThread() throws {
    let m = try app()
    let label = try #require(m.core.labels().first)
    let text = try #require(m.core.state(path: "label/\(label.id)/flow/ai_reads"))
    let object = try #require(JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any])
    #expect(object["rest_of_thread"] is Bool)                 // fails on the old code: the dictionary held subject, body and attachments only
    #expect(object["subject"] is Bool && object["body"] is Bool && object["attachments"] is Bool)
}

// MARK: 2. d-plus-gives-the-value-field-back

@MainActor @Test func thePlusDoorGivesTheValueFieldBackWithNoSwiftUIFocus() throws {
    let m = try app()
    let inspector = LabelInspectorModel.shared
    defer { inspector.close(); inspector.focusedRule = nil; inspector.focusRequest = nil }
    inspector.focusedRule = nil
    #expect(inspector.openNew(parent: nil))
    inspector.editChain(.entry) { $0.rows = [RuleDraft(kind: .from, values: ["a@example.be"])] }
    let row = try #require(inspector.draft.entry.rows.first)
    #expect(!LabelRuleDoors.fieldOpen(row))                   // a row with a value and no focus shows the quiet "+"
    let door = try #require(LabelRuleDoors.clicks()["\(LabelInspectorSocket.ax).rule.0.addvalue"])
    #expect(door(m))
    let after = try #require(inspector.draft.entry.rows.first)
    #expect(LabelRuleDoors.fieldOpen(after))                  // fails on the old code: only `focusRequest` was set, and `focusedRule` waits for the view
    #expect(inspector.focusedRule == after.id)
}

// MARK: 3. a-tree-node-decided-by-names-the-node

@MainActor @Test func aRuleRowOfTheLabelsPageNamesTheNodeItFilesUnder() throws {
    let page = try #require(DecisionCatalog.shipped().first { $0.id == "labels" })
    let rule = try #require(page.entries.first { $0.kind == .rule })
    let rows = FixtureDecisionPreview.rows(page: page.id, entries: DecisionPreview.entries(of: page), engineOn: false)   // the sample moved into the test engine (gap-preview-core)
    let hits = rows.filter { $0.decidedBy.contains(" · rule") }
    #expect(!hits.isEmpty)
    #expect(hits.allSatisfy { $0.decidedBy.contains(rule.outcome) })   // fails on the old code: "entry 1 · rule" with no node
    let other = try #require(DecisionCatalog.shipped().first { $0.id == "which-tab" })
    #expect(FixtureDecisionPreview.rows(page: other.id, entries: DecisionPreview.entries(of: other), engineOn: false).contains { $0.decidedBy == "entry 1 · rule" })   // the other pages keep their words
}

// MARK: 4. a-tree-node-accepting-the-proposal-creates-the-label

@MainActor @Test func acceptingTheProposalCreatesTheChildInTheCoresTree() throws {
    let m = try app()
    let store = AISettingsStore.shared
    let parent = try #require(m.core.labels().first { $0.parentId == nil })
    let path = AILabelTree.display(parent) + AILabelTree.separator + "Proposed-\(UUID().uuidString.prefix(6))"
    let before = m.core.labels().map(AILabelTree.display)
    #expect(!before.contains(path))
    store.proposals["labels"] = path
    let at = try #require(store.pageIndex("labels"))
    let rules = store.pages[at].entries.count
    store.acceptProposal(page: "labels")
    #expect(m.core.labels().map(AILabelTree.display).contains(path))   // fails on the old code: a rule was inserted, the tree stayed as it was
    #expect(store.pages[at].entries.count == rules + 1)
    #expect(store.proposals["labels"] == nil)
    let node = try #require(m.core.labels().first { AILabelTree.display($0) == path })
    #expect(node.parentId == parent.id)
}

// MARK: 5. preview-runs-on-20-and-writes-nothing

@MainActor @Test func applyAsksTheCoreToWriteThePreviewAndThePreviewAloneWritesNothing() async throws {
    let m = try app()
    let store = AISettingsStore.shared
    store.applied = []
    store.section = .page("which-tab")      // the preview runs off the main actor and lands only on the open page (gap-decisions-real)
    store.runPreview(page: "which-tab")
    await store.settlePreview(page: "which-tab")
    #expect(m.core.state(path: "decision_preview/applied") == "false")
    #expect(!m.core.decisionPreview().isEmpty)                // the core ran its own preview on the recent mail
    store.apply(page: "which-tab")
    #expect(m.core.state(path: "decision_preview/applied") == "true")   // fails on the old code: `.applyDecisionPreview` was never sent
    #expect(store.applied.contains("which-tab"))
    store.applied = []
}

// MARK: 6. J2.6/rule-in-words

@MainActor @Test func theEditButtonOfASavedFolderDoesNotCarryTheSearchWordsIntoItsRule() throws {
    let m = try app()
    let s = SearchMacModel.shared, labels = LabelsContactsModel.shared
    defer { labels.pane = .labels; labels.draftRule = ""; m.typed = ""; m.surface = "mac-main"; s.folders = [] }
    _ = try m.core.run(command: .saveSmartFolder(name: "Quotes-\(UUID().uuidString.prefix(6))", ruleWords: "quote"))
    labels.reloadFolders()
    s.reloadFolders()
    let i = try #require(s.folders.firstIndex { $0.rule == "quote" })
    m.typed = "quote"                                         // the search field's words, mirrored in the shared buffer
    _ = s.editFolder(i)
    #expect(m.typed.isEmpty)                                  // fails on the old code: the buffer kept the words
    labels.takeTyped(m)                                       // what the editor does on its next read
    #expect(labels.draftRule == "quote")                      // fails on the old code: "quotequote"
}

// MARK: 7. the unread dot of a search result row follows U

@MainActor @Test func theResultRowDrawsTheUnreadStateTheUKeyLeftInTheModel() throws {
    let m = try app()
    let s = SearchMacModel.shared
    defer { s.active = false; s.launched = false; s.loading = false; s.selected = 0; s.outcome = SearchEngine.Outcome(); s.edited = [:] }
    m.focusIndex = 0
    s.active = true; s.launched = true; s.loading = false; s.selected = 0
    s.outcome.results = Array(m.rows[1...3]).map(result)
    let first = try #require(s.results.first).row
    #expect(TriageActions.toggleRead(m))                      // the U key acts on the selected result and records it in `edited`
    let drawn = ResultRow.shown(first, edited: s.edited[first.messageId])
    #expect(drawn.unread == !first.unread)                    // the old row drew `result.row`, the snapshot, and kept the old dot
    #expect(ResultRow.shown(first, edited: nil).unread == first.unread)
    #expect(TriageActions.toggleRead(m))
    #expect(ResultRow.shown(first, edited: s.edited[first.messageId]).unread == first.unread)
}
