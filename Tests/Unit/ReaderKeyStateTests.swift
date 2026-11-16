import Foundation
import Testing
@testable import WhooshUI

// The state table of the mail view's keys (the owner: "shortcuts are contextual"). ONE answer, `MessageBodyFeature.readerHoldsKeys`
// (a mail is open: surface mac-reader), is read by the bottom key bar, shift I, O and the colours hint; the pane focus is never asked.
//   LIST   no mail open                       bar = list set     shift I = filter-important   Escape: the list's own
//   READER a mail is open, no reply field     bar = reader set   shift I = toggle-colours     Escape: the pane goes to the list, the list's Escape closes the mail
//   REPLY  the reply field holds the keys     bar = composer set letters type                 Escape: the field closes, the pane is the reader's
// Run with WHOOSH_FIXTURE_CORE=1 so the shared AppModel serves the design fixture.
// MUTANTS: each test names, in its trailing comment, the mutation of the target that must turn it red.

@MainActor private func model() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()   // launch state once per test (Tests/Support/SharedAppFresh.swift)
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    MacShellState.shared.listCollapsed = false
    return m
}

@MainActor private func enter(_ surface: String, pane: MacShellState.Pane, _ m: AppModel) {
    m.overlay = nil
    m.surface = surface
    MacShellState.shared.focusedPane = pane
}

@MainActor private func leave(_ m: AppModel) {
    m.surface = "mac-main"
    MacShellState.shared.focusedPane = .list
    MacShellState.shared.listCollapsed = false
}

@MainActor @Test func aMailOpenIsTheReaderStateWhateverPaneHasTheFocus() throws {
    let m = try model()
    defer { leave(m) }
    for pane in MacShellState.Pane.allCases {
        enter(ReaderState.surface, pane: pane, m)
        #expect(MessageBodyFeature.readerHoldsKeys(m), "pane \(pane) with a mail open is the reader state")   // MUTANT: the state reads focusedPane == .reader
        #expect(ReaderKeys.isOpen(m) == !QuickReplyModel.shared.isOpen, "O reads the same state as the bar")
    }
}

@MainActor @Test func noMailOpenIsTheListStateWhateverPaneHasTheFocus() throws {
    let m = try model()
    defer { leave(m) }
    for pane in MacShellState.Pane.allCases {
        enter("mac-main", pane: pane, m)
        #expect(!MessageBodyFeature.readerHoldsKeys(m), "pane \(pane) with no mail open is the list state")   // MUTANT: the state reads focusedPane == .reader
        #expect(!ReaderKeys.isOpen(m))
        #expect(MessageBodyFeature.coloursHint() == nil, "no colours hint in the list state")
    }
}

@MainActor @Test func theBottomBarReadsTheReaderSetWhenAMailIsOpenWhileThePaneFocusIsTheList() throws {
    let m = try model()
    defer { leave(m) }
    enter(ReaderState.surface, pane: .list, m)
    #expect(MacFootHints.surface(m) == .reader)   // MUTANT: the bar asks focusedPane == .reader
}

@MainActor @Test func theBottomBarReadsTheListSetWhenNoMailIsOpenWhileThePaneFocusIsTheReader() throws {
    let m = try model()
    defer { leave(m) }
    enter("mac-main", pane: .reader, m)
    #expect(MacFootHints.surface(m) == .list)   // MUTANT: the bar asks focusedPane == .reader (the main79 stale pane)
}

@MainActor @Test func theBottomBarReadsTheComposerSetWhenTheComposerHoldsTheKeyboard() throws {
    let m = try model()
    defer { leave(m) }
    enter(ComposerIds.surface, pane: .reader, m)
    #expect(MacFootHints.surface(m) == .composer)
}

@MainActor @Test func escapeInTheReaderStateMovesThePaneToTheListAndPassesTheKeyOn() throws {
    let m = try model()
    defer { leave(m) }
    enter(ReaderState.surface, pane: .reader, m)
    #expect(!ReaderKeys.back(m), "the list's Escape closes the mail after this, so the command passes")
    #expect(MacShellState.shared.focusedPane == .list)   // MUTANT: back leaves the pane where the click put it
    #expect(m.surface == ReaderState.surface, "closing the mail is the list's Escape, not this command's")
}

@MainActor @Test func escapeInTheListStateWithAStaleReaderPaneMovesThePaneToTheList() throws {
    let m = try model()
    defer { leave(m) }
    enter("mac-main", pane: .reader, m)
    #expect(!ReaderKeys.back(m))
    #expect(MacShellState.shared.focusedPane == .list)   // MUTANT: back leaves the pane where the click put it
}

@MainActor @Test func escapeKeepsAnotherPaneAndAnOverlayAndACollapsedListAsTheyAre() throws {
    let m = try model()
    defer { leave(m) }
    enter(ReaderState.surface, pane: .sidebar, m)
    _ = ReaderKeys.back(m)
    #expect(MacShellState.shared.focusedPane == .sidebar, "only a reader pane focus is moved")
    enter(ReaderState.surface, pane: .reader, m)
    m.overlay = "mac-palette"
    _ = ReaderKeys.back(m)
    #expect(MacShellState.shared.focusedPane == .reader, "an overlay takes the Escape: nothing moves")
    enter(ReaderState.surface, pane: .reader, m)
    MacShellState.shared.listCollapsed = true
    _ = ReaderKeys.back(m)
    #expect(MacShellState.shared.focusedPane == .reader, "a collapsed list cannot take the focus")
}

@MainActor @Test func theComposerTakesEnterAndEscapeBeforeTheReaderWhateverThePaneFocusIs() throws {
    let m = try model()
    defer { _ = ComposerModel.shared.close(m); leave(m) }
    enter(ReaderState.surface, pane: .reader, m)
    #expect(ReaderFeature.hasFocus(m), "no composer open: the reader pane holds the keys")
    #expect(ComposerModel.shared.openNew(m), "a composer opens on the fixture")
    enter(ComposerIds.surface, pane: .reader, m)   // a click in the mail left the pane focus on the reader
    #expect(ComposerMacFeature.hasFocus(m))
    #expect(!ReaderFeature.hasFocus(m), "the open composer outranks the reader pane in the key chain (byFocus)")   // MUTANT: hasFocus is only focusedPane == .reader
    #expect(!ReaderFeature.openAndReply(m), "Enter is the composer's (a pending discard confirmation), never a reply")   // MUTANT: openAndReply has no composer guard
}
