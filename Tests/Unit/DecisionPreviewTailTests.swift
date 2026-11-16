import Foundation
import Testing
@testable import WhooshUI

// fix-decisions-tail: (F1) the page draws the pane's own "checking" word only while the preview runs; (F3) the test engine serves `decision_preview/placed` as the real one does.
// Written without a compile; never run by the author. Broken copies (each to be seen red by the lead's run):
//  - DecisionLabelMenu.swift `PreviewLine.shows`: `state == .running` made `state != .idle` -> theWorkingWordShowsOnlyWhileThePreviewRuns (done reads as working)
//  - DecisionLabelMenu.swift `PreviewLine.shows`: `state == .running` made `false` -> theWorkingWordShowsOnlyWhileThePreviewRuns (running reads as idle)
//  - FixtureCore.swift the `"placed"` arm: `settings.placedBy.count` made `0` -> theTestEngineServesThePlacedCount

@MainActor private func openTailStore() -> (AppModel, AISettingsStore) {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    setenv("WHOOSH_FIXTURE_PATH", URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()
    let store = AISettingsStore.shared
    store.dropAllPreviews()
    store.previews = [:]; store.previewNotes = [:]; store.applied = []
    store.section = .page("which-tab")
    return (m, store)
}

@MainActor @Test func theWorkingWordShowsOnlyWhileThePreviewRuns() async {
    let (_, store) = openTailStore()
    #expect(!PreviewLine.shows(store.previewState("which-tab")), "idle before the click: no working word")
    store.runPreview(page: "which-tab")
    #expect(PreviewLine.shows(store.previewState("which-tab")), "the click returns with the page running: the word shows")
    await store.settlePreview(page: "which-tab")
    #expect(store.previewState("which-tab") == .done)
    #expect(!PreviewLine.shows(store.previewState("which-tab")), "the word leaves when the result lands")
    for s in [AISettingsStore.PreviewState.idle, .done, .refused] { #expect(!PreviewLine.shows(s)) }
    #expect(PreviewLine.shows(.running))
    store.section = .decisions
}

@MainActor @Test func theTestEngineServesThePlacedCount() async throws {
    let (m, store) = openTailStore()
    store.runPreview(page: "which-tab")
    await store.settlePreview(page: "which-tab")
    #expect(m.core.state(path: "decision_preview/placed") == "0", "nothing placed before Apply")
    let differing = try #require(store.previews["which-tab"]).filter { $0.differs }.count
    store.apply(page: "which-tab")
    #expect(m.core.state(path: "decision_preview/applied") == "true")
    let placed = Int(m.core.state(path: "decision_preview/placed") ?? "") ?? -1
    #expect(placed >= differing, "the count Apply wrote covers the rows that differ (the fixture keeps earlier placements, so it may be higher)")
    #expect(placed > 0 || differing == 0)
    store.applied = []
    store.section = .decisions
}
