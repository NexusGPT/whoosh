import Foundation
import Testing
@testable import WhooshUI

// gap-decisions-real: the Decisions preview asks the core OFF the main actor (AISettingsStore.runPreview): the page is `running` at once and stays responsive, the answer lands
// on the main actor, a second click while one runs starts no second run, a page closed or a list edited meanwhile drops the result, and Apply after that run marks the page applied. Written without a compile; never run by the author. Broken copies (each to be seen red by the lead's run):
//  - AISettingsStore.swift `runPreview`: `Task.detached(priority: .userInitiated) { [weak self] in` made `Task { @MainActor [weak self] in` (the core is asked on the main actor) -> thePreviewIsRunningAtOnceAndLandsLater
//  - AISettingsStore.swift `runPreview`: the `previewState(id) != .running` half of the guard removed -> aSecondClickWhileOneRunsStartsNoSecondRun
//  - AISettingsStore.swift `landPreview`: the `section == .page(id)` guard removed -> closingThePageDropsTheResult
//  - AISettingsStore.swift `forgetPreview`: the `dropPreview(page: id)` line removed -> editingTheListWhileItRunsDropsTheResult
//  - AISettingsStore.swift `apply`: the `applied.insert(id)` line removed -> applyAfterTheOffMainPreviewMarksThePageApplied (a refusal by the core cannot be forced on the test engine: the real core's refusals are tested in core/crates/engine/tests/engine_decision_apply.rs)

@MainActor private func openStore() -> AISettingsStore {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    setenv("WHOOSH_FIXTURE_PATH", URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent().appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    _ = SharedApp.begin()
    let store = AISettingsStore.shared
    store.dropAllPreviews()
    store.previews = [:]; store.previewNotes = [:]; store.applied = []
    store.section = .page("which-tab")
    return store
}

@MainActor @Test func thePreviewIsRunningAtOnceAndLandsLater() async {
    let store = openStore()
    store.runPreview(page: "which-tab")
    #expect(store.previewState("which-tab") == .running, "the click returns with the page working, not with the answer")
    #expect(store.previews["which-tab"] == nil)
    await store.settlePreview(page: "which-tab")
    #expect(store.previewState("which-tab") == .done)
    #expect(store.previews["which-tab"]?.count == Int(WhooshConstants.DECISION_PREVIEW_N))
    #expect(!AISettingsStore.PreviewAnswer.lastAskRanOnMainThread, "the core was asked off the main thread")
}

@MainActor @Test func aSecondClickWhileOneRunsStartsNoSecondRun() async {
    let store = openStore()
    let before = store.previewStarts
    store.runPreview(page: "which-tab")
    store.runPreview(page: "which-tab")
    #expect(store.previewStarts == before + 1)
    await store.settlePreview(page: "which-tab")
    store.runPreview(page: "which-tab")      // control: once the first landed, a click starts a run
    #expect(store.previewStarts == before + 2)
    await store.settlePreview(page: "which-tab")
}

@MainActor @Test func closingThePageDropsTheResult() async {
    let store = openStore()
    store.runPreview(page: "which-tab")
    store.section = .providers
    await store.settlePreview(page: "which-tab")
    #expect(store.previews["which-tab"] == nil)
    #expect(store.previewState("which-tab") == .idle)
}

@MainActor @Test func editingTheListWhileItRunsDropsTheResult() async throws {
    let store = openStore()
    let p = try #require(store.pageIndex("which-tab"))
    store.runPreview(page: "which-tab")
    store.update(page: "which-tab", entry: store.pages[p].entries[0].id) { $0.enabled.toggle() }
    await store.settlePreview(page: "which-tab")
    #expect(store.previews["which-tab"] == nil)
    #expect(store.previewState("which-tab") == .idle)
}

@MainActor @Test func applyAfterTheOffMainPreviewMarksThePageApplied() async {
    let store = openStore()
    store.runPreview(page: "which-tab")
    await store.settlePreview(page: "which-tab")
    store.apply(page: "which-tab")
    #expect(store.applied.contains("which-tab"))
    store.applied = []
    store.section = .decisions
}
