import Foundation
import Testing
@testable import WhooshUI

// Two saves that were missing: the full composer's draft (written to the core after the typing rests, on close, on a lost keyboard; one save at a
// time) and a smart folder's rule and parent (Save runs the core's `updateSmartFolder` with the folder's id; a refusal shows in the core's words).
// Run with WHOOSH_FIXTURE_CORE=1 so the shared AppModel serves the design fixture.

@MainActor private func model() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()   // launch state once per test (Tests/Support/SharedAppFresh.swift)
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}

/// A new message open on the shared composer with a recipient, and nothing saved yet.
@MainActor private func openMessage(_ m: AppModel) throws -> ComposerModel {
    let c = ComposerModel.shared
    c.discardDraft()
    try #require(c.openNew(m), "a composer opens on the fixture")
    c.to = ["someone@example.com"]
    c.autosaveWrites = 0; c.autosaveSaved = ""; c.autosaveGate = nil
    return c
}

@MainActor private func closeMessage(_ c: ComposerModel, _ m: AppModel) {
    c.autosaveGate = nil
    c.dropCoreDraft(); c.discardDraft()
    _ = c.close(m)
}

@MainActor private func rest() async throws { try await Task.sleep(for: .milliseconds(ComposerModel.draftPauseMs + 600)) }

@MainActor @Test func typingThenThePauseSavesTheDraftOnceWithTheLatestText() async throws {
    let m = try model()
    let c = try openMessage(m)
    defer { closeMessage(c, m) }
    c.session.setPlain("he"); c.session.setPlain("hello"); c.session.setPlain("hello there")
    #expect(c.autosaveWrites == 0, "a key press writes nothing: the save waits for the pause")   // MUTANT: the change saves at once
    try await rest()
    #expect(c.autosaveWrites == 1, "one save for a burst of keys")
    let id = try #require(c.coreDraftId, "the save made the core's draft")
    #expect(m.core.state(path: "draft/\(id)/body")?.contains("hello there") == true, "the core holds the latest text")
}

@MainActor @Test func aChangeDuringASaveIsSavedAfterIt() async throws {
    let m = try model()
    let c = try openMessage(m)
    defer { closeMessage(c, m) }
    c.session.setPlain("first")
    var rounds = 0
    c.autosaveGate = {
        rounds += 1
        if rounds == 1 { c.session.setPlain("second"); await c.autosave(refresh: false) }   // asked while the first save runs: it must wait, not overlap
    }
    await c.autosave(refresh: false)
    #expect(rounds == 2, "the running save went round again for the request it was handed")   // MUTANT: a request during a save is dropped
    // The gate runs at the START of a round (ComposerModel.swift:128), before the round reads the text: the change made inside it is in that round's one write,
    // and the round the request asked for finds nothing new to write. So the first text is never written alone: one write, holding the latest text.
    #expect(c.autosaveWrites == 1, "the change made while the save was starting is in its write; the second round finds nothing new")   // MUTANT: the second round writes the same text again
    let id = try #require(c.coreDraftId)
    let body = try #require(m.core.state(path: "draft/\(id)/body"))
    #expect(body.contains("second"), "the core holds the latest text")   // MUTANT: the save writes the text read before the change
    #expect(!body.contains("first"), "the first text was replaced, not kept beside it")
    #expect(!c.autosaveRunning)
}

@MainActor @Test func closingTheComposerSavesTheDraft() async throws {
    let m = try model()
    let c = try openMessage(m)
    defer { closeMessage(c, m) }
    c.session.setPlain("typed and closed at once")
    #expect(c.coreDraftId == nil)
    #expect(c.close(m))
    try await Task.sleep(for: .milliseconds(300))   // the save runs after the key's turn, without waiting for the pause
    let id = try #require(c.coreDraftId, "closing saved the draft")
    #expect(m.core.state(path: "draft/\(id)/body")?.contains("typed and closed at once") == true)
}

@MainActor @Test func aNewMessagesFirstSaveGivesItACoreDraftId() async throws {
    let m = try model()
    let c = try openMessage(m)
    defer { closeMessage(c, m) }
    c.session.setPlain("first words")
    #expect(c.coreDraftId == nil, "nothing is saved before the first save")
    await c.saveNow(refresh: false).value
    let id = try #require(c.coreDraftId, "the first save gave the message its draft id")
    c.session.setPlain("more words")
    await c.saveNow(refresh: false).value
    #expect(c.coreDraftId == id, "later saves update the same draft")
    #expect(m.core.state(path: "draft/\(id)/body")?.contains("more words") == true)
}

@MainActor @Test func aSmartFoldersSaveRunsTheCommandWithTheRuleAndTheParentId() throws {
    let m = try model()
    let folders = m.core.smartFolders()
    let f = try #require(folders.first), g = try #require(folders.dropFirst().first)
    let lc = LabelsContactsModel.shared
    defer { lc.pane = .labels }
    #expect(lc.edit(f.id))
    lc.draftRule = "has attachment from suppliers"
    lc.draftParent = g.name    // the chip holds the name it shows
    #expect(lc.save(), "the core accepted the update")
    #expect(lc.folderRefusal == nil)
    let after = try #require(m.core.smartFolders().first { $0.id == f.id })
    #expect(after.ruleWords == "has attachment from suppliers", "the core holds the new rule")   // MUTANT: save keeps the rule on screen only
    #expect(after.parentId == g.id, "the core holds the parent's smart folder id, not a name")
}

@MainActor @Test func aSmartFolderRefusedByTheCoreShowsItsWordsOnTheScreensLine() throws {
    let m = try model()
    let f = try #require(m.core.smartFolders().first)
    let lc = LabelsContactsModel.shared
    defer { lc.pane = .labels }
    #expect(lc.edit(f.id))
    lc.draftParent = f.id    // inside itself: the core refuses
    #expect(!lc.save())
    #expect(lc.folderRefusal?.isEmpty == false, "the core's words are on the screen's own line")
    #expect(m.core.smartFolders().first { $0.id == f.id }?.parentId == nil, "a refused save changes nothing")
}
