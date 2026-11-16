import Foundation
import Testing
@testable import WhooshUI

// The reader opens at the mail that holds the selected search result's passage and marks it (docs/SPEC.md A152, owner 2026-11-11: "it's subject+body+attachment
// and they should be all related"; "+thread too btw"; card answer "P1 The passage on the row", whose drawing shows the reader opened at the passage, marked).
// The mark is the drawing's `.pm` (docs/design/boards/search-r3-parts/designs.js). Run with WHOOSH_FIXTURE_CORE=1 like the neighbours.
// Each test names, in its comment, the one-line broken copy that must turn it red. Nothing here was compiled or run by its author (the brief: the lead runs them).

@MainActor private func app() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}

@MainActor private func reset(_ s: SearchMacModel, _ m: AppModel) {
    s.release()
    s.active = false; s.launched = false; s.loading = false; s.selected = 0; s.outcome = SearchEngine.Outcome(); s.edited = [:]
    s.engineAsk = nil; s.hitRowsRead = nil; s.stageRead = nil; s.stage = ""; s.stageReason = ""; s.refusal = nil
    s.hits = []; s.hitsFor = ""; s.awaiting = ""; s.hitCache = [:]; s.hitRowsCache = [:]
    s.launchedText = ""; s.shownText = ""; s.held = ""; s.folderRefusals = [:]; s.folderWordsPending = []
    s.returnToSearch = false; s.openedMessageId = nil; s.owns = false; s.restOpen = false; s.onMore = false
    m.overlay = nil; m.surface = SurfaceSlot.macMain.id; m.typed = ""
    MacShellState.shared.focusedPane = .list
}

private func row(_ id: String, thread: String? = nil, sender: String = "Claire Martin", subject: String = "Planning for the opening", at: Int64 = 0) -> InboxRow {
    InboxRow(messageId: id, threadId: thread ?? "t-\(id)", accountId: "acc-01", senderName: sender, subject: subject, preview: "Preview that holds nothing typed", receivedAtMs: at,
             unread: false, pinned: false, labelNames: [], attachmentCount: 0, threadCount: 1, senderAddress: "claire.martin@lorvan-hotels.example")
}

private func hit(_ r: InboxRow, _ m: MatchedIn, snippet: String = "", file: String? = nil) -> SearchHit {
    SearchHit(messageId: r.messageId, score: 1, snippet: snippet, matchedIn: m, attachmentName: file, attachmentNote: nil)
}

private func answer(_ typed: String, _ rows: [InboxRow], _ hits: [SearchHit], heads: [String: InboxRow] = [:]) -> [SearchResult] {
    SearchEngine.fromHits(SearchQuery.typed(typed), hits: hits, rows: rows, beforeMs: { _ in 0 }, names: { _ in [] }, engineFiltered: true, heads: heads).results
}

/// The search screen holding one answer, as the owner has it on screen before or after Enter: results showing, none loading, the first selected.
@MainActor private func showing(_ m: AppModel, _ results: [SearchResult]) -> SearchMacModel {
    let s = SearchMacModel.shared
    reset(s, m)
    s.active = true; s.launched = true; s.loading = false; s.stage = "ranked"
    s.outcome.results = results
    s.selected = 0
    return s
}

/// A conversation as the reader builds it: the newest mail open, earlier mails by their row facts, no body stored.
private func thread(open: InboxRow, earlier: [InboxRow] = []) -> ReaderThread {
    ReaderThread(threadId: open.threadId, messageId: open.messageId, accountId: "acc-01", subject: open.subject, participants: [open.senderName],
                 earlier: earlier.map { ReaderFact(row: $0) }, open: ReaderFact(row: open), attachmentCount: 0, messageCount: earlier.count + 1, body: nil)
}

private let textSnippet = "the revised price per room is in the last column"

// MARK: the hand-over

// broken copy: ReaderPassage.of builds `letters: []` (the typed letters are dropped: the whole passage would be marked, not the letters)
@MainActor @Test func theSelectedTextResultHandsTheReaderItsMailItsPassageAndItsLetters() throws {
    let m = try app(); let r = row("m1")
    let s = showing(m, answer("revised price", [r], [hit(r, .textWords(ranges: []), snippet: textSnippet)])); defer { reset(s, m) }
    let p = try #require(ReaderPassage.current(m), "the reader gets nothing for the selected text result")
    #expect(p.messageId == "m1" && p.threadId == "t-m1" && p.source == .text && p.fileName == nil)
    #expect(p.text == textSnippet)
    #expect(p.letters == ["revised", "price"])
    #expect(p.marksText)
}

// broken copy: ReaderPassage.of leaves `fileName` nil for an attachment, or `marksText` is true for source .attachment (a text mark for a file)
@MainActor @Test func anAttachmentResultHandsTheFileNameAndMarksNoText() throws {
    let m = try app(); let r = row("m1")
    let s = showing(m, answer("revised price", [r], [hit(r, .attachmentContent, snippet: "… the revised price per room is in the last column …", file: "rooms-2026.xlsx")])); defer { reset(s, m) }
    let p = try #require(ReaderPassage.current(m))
    #expect(p.source == .attachment && p.fileName == "rooms-2026.xlsx" && p.messageId == "m1")
    #expect(!p.marksText)
    let t = thread(open: r)
    let body = MessageBodyContent(id: "m1", safeHtml: nil, plainText: "the revised price per room is in the last column", trackersBlocked: 0, quoteOffset: nil)
    #expect(p.page(in: t, content: body) == nil)                      // no text mark for a file's passage
    #expect(p.file(in: ["notes.txt", "rooms-2026.xlsx (210 KB)"]) == "rooms-2026.xlsx (210 KB)")
    #expect(p.file(in: ["other.xlsx"]) == nil)
}

// broken copy: ReaderPassage.of takes `r.row.messageId` and not `r.hitMessageId` (the conversation's newest mail stands for the earlier mail that holds the passage)
@MainActor @Test func anEarlierMailResultHandsTheEarlierMailsIdAndTheReaderFindsItInTheThread() throws {
    let m = try app()
    let head = row("m9", thread: "t1", sender: "Claire Martin", subject: "Call next week?", at: 1_700_000_000_000)
    let early = row("m1", thread: "t1", sender: "Marc Petit", subject: "Call next week?", at: 1_699_000_000_000)
    let s = showing(m, answer("revised price", [early], [hit(early, .textWords(ranges: []), snippet: textSnippet)], heads: ["t1": head])); defer { reset(s, m) }
    #expect(s.selectedResult?.row.messageId == "m9")                  // the list row is the conversation's newest mail
    let p = try #require(ReaderPassage.current(m))
    #expect(p.messageId == "m1" && p.threadId == "t1" && p.source == .thread)
    let t = thread(open: head, earlier: [early])
    #expect(p.earlierIndex(in: t) == 0 && !p.holdsOpenMail(t))
    #expect(p.isFor(t) && !p.isFor(thread(open: row("other"))))
}

// MARK: the ranges

// broken copy: PassageMark.core does not take the "…" off the ends (the passage with its dots is looked for and is not in the mail)
@Test func aPassageCutWithDotsMarksThePartBetweenTheDots() {
    let text = "Attached: offer-v3.pdf, page 2\n\nas agreed, we lowered the amount for the second phase and kept the first phase unchanged"
    let both = PassageMark.pieces(passage: "… as agreed, we lowered the amount for the second phase …", letters: [], in: text)
    #expect(both == ["as agreed, we lowered the amount for the second phase"])
    #expect(PassageMark.pieces(passage: "...we lowered the amount...", letters: [], in: text) == ["we lowered the amount"])
    #expect(PassageMark.core("… as agreed, we lowered …") == "as agreed, we lowered")
}

// broken copy: PassageMark.fold keeps white space runs as they are (no fold) or `find` drops .diacriticInsensitive
@Test func whiteSpaceThatDiffersAndAccentsStillFindThePassage() {
    let text = "Hello Assem,\n\nCan you confirm the\n  revised   price of the quote?"
    let piece = PassageMark.pieces(passage: "Can you confirm the revised price", letters: [], in: text)
    #expect(piece == ["Can you confirm the\n  revised   price"])      // the mail's own characters, with its own white space, are the marked part
    let accents = "Le café est prêt pour la réunion"
    #expect(PassageMark.pieces(passage: "le cafe est pret", letters: ["CAFE"], in: accents) == ["café"])
}

// broken copy: PassageMark.find searches the typed letters in the whole text and not inside the passage (the two earlier "price" are marked too)
@Test func eachOccurrenceOfTheLettersInsideThePassageIsMarkedAndOnlyThere() {
    let text = "The price list. A price is a price. Then the revised price per room stays."
    let marked = PassageMark.pieces(passage: "the revised price per room", letters: ["price", "revised"], in: text)
    #expect(marked == ["revised", "price"])
    let twice = PassageMark.pieces(passage: "price for price", letters: ["price"], in: "Our price for price lists")
    #expect(twice == ["price", "price"])
}

// broken copy: PassageMark.find searches with `.backwards` (the last occurrence is marked, not the first)
@Test func theFirstOccurrenceOfThePassageIsTheOneMarked() {
    let text = "ok. total stays. then total stays."
    let r = PassageMark.ranges(passage: "total stays", letters: [], in: text)
    #expect(r.count == 1 && text.distance(from: text.startIndex, to: r[0].lowerBound) == 4)
}

// broken copy: PassageMark.find falls back to the first line when the passage is not found (a mark where the mail does not hold the passage)
@Test func aMailThatDoesNotHoldThePassageGetsNoMark() {
    #expect(PassageMark.ranges(passage: "the second phase", letters: [], in: "Nothing of that kind here").isEmpty)
    #expect(PassageMark.page(passage: "the second phase", letters: [], in: "Nothing of that kind here") == nil)
    #expect(PassageMark.ranges(passage: "…", letters: [], in: "text").isEmpty)
}

// broken copy: PassageMark.page counts spans in Characters and not UTF-16 units (an emoji or an accent before the mark shifts the mark in the page)
@Test func thePageGetsTheMarkedSpansInItsOwnUnitsAndTheScriptLoadsNothing() throws {
    let page = try #require(PassageMark.page(passage: "😀 revised price", letters: ["revised"], in: "Café 😀 revised price today"))
    #expect(page.text == "😀 revised price" && page.spans == [[3, 10]])
    let js = PassageMark.script(page)
    #expect(js.contains("mark.pm") || js.contains("'mark'"))
    for remote in ["http", "fetch(", "XMLHttpRequest", "src=", "import("] { #expect(!js.contains(remote), "the script mentions \(remote)") }
    #expect(PassageMark.script(nil).contains("return [0,0]"))          // no passage: the earlier marks go, nothing else
    #expect(PassageMark.css(accent: RGBA(10, 132, 255, 1)).contains("0.26"))
}

// MARK: the reader

// broken copy: ReaderFocus.settle and the early-return branch of `changed` do not call `syncPassage(m)` (the old passage stays when the search closes)
@MainActor @Test func nothingIsMarkedAfterTheSearchClosesOrWhenAnInboxMailIsOpen() throws {
    let m = try app(); let r = row("m1")
    let s = showing(m, answer("revised price", [r], [hit(r, .textWords(ranges: []), snippet: textSnippet)])); defer { reset(s, m) }
    let focus = ReaderFocus(); focus.ensure(m)
    #expect(focus.passage?.messageId == "m1")
    #expect(s.close())
    focus.catchUp(m)
    #expect(ReaderPassage.current(m) == nil)
    #expect(focus.passage == nil, "the reader still holds the passage of the closed search")
    let inbox = try #require(m.rows.first)
    let t = try #require(ReaderThread.make(m, messageId: inbox.messageId, resting: true))
    ReaderFocus.shared.catchUp(m)
    let reading = ReaderPassage.reading(m, t)
    #expect(reading["marked"] as? Bool == false && reading["text"] as? String == "" && reading["source"] as? String == "")
}

// broken copy: ReaderState.earlierShown / expandedShown ignore the passage (the earlier mail that holds it stays folded)
@MainActor @Test func theEarlierMailThatHoldsThePassageOpensAndTheOthersKeepTheirState() throws {
    let m = try app()
    let head = row("m9", thread: "t1", sender: "Claire Martin", subject: "Call next week?", at: 3)
    let early = row("m1", thread: "t1", sender: "Marc Petit", subject: "Call next week?", at: 1)
    let mid = row("m5", thread: "t1", sender: "Claire Martin", subject: "Call next week?", at: 2)
    let s = showing(m, answer("revised price", [early], [hit(early, .textWords(ranges: []), snippet: textSnippet)], heads: ["t1": head])); defer { reset(s, m); ReaderFocus.shared.catchUp(m) }
    ReaderFocus.shared.ensure(m); ReaderFocus.shared.catchUp(m)
    let t = thread(open: head, earlier: [early, mid])
    let state = ReaderState.shared
    state.earlierOpenFor = nil; state.expanded = []
    #expect(!state.earlierOpen(t))                                    // the person did not unfold anything
    #expect(state.earlierShown(t))                                    // the passage's mail is shown anyway
    #expect(state.expandedShown(t) == [0])                            // whole for the mail that holds it, the other earlier mail keeps its first words
    let other = thread(open: row("zz"))
    #expect(!state.earlierShown(other) && state.expandedShown(other).isEmpty)
}

// broken copy: ReaderPassage.rings ignores `messageId` (every mail's file of that name rings) or compares the name with the passage text
@MainActor @Test func theTileOfTheResultsFileRingsAndNoOtherFileDoes() throws {
    let m = try app(); let r = row("m1")
    let s = showing(m, answer("revised price", [r], [hit(r, .attachmentContent, snippet: "… the revised price …", file: "rooms-2026.xlsx")])); defer { reset(s, m); ReaderFocus.shared.catchUp(m) }
    ReaderFocus.shared.ensure(m); ReaderFocus.shared.catchUp(m)
    #expect(ReaderPassage.rings(messageId: "m1", name: "rooms-2026.xlsx"))
    #expect(!ReaderPassage.rings(messageId: "m1", name: "other.xlsx"))
    #expect(!ReaderPassage.rings(messageId: "m2", name: "rooms-2026.xlsx"))
}

// broken copy: ReaderPassage.reading answers `marked: true` from the hand-over alone (without finding the passage in the stored body), or reads the body from the network
@MainActor @Test func aBodyThatIsNotStoredGivesNoMarkAndNoWait() throws {
    let m = try app(); let r = row("no-body-stored-1")
    let s = showing(m, answer("revised price", [r], [hit(r, .textWords(ranges: []), snippet: textSnippet)])); defer { reset(s, m); ReaderFocus.shared.catchUp(m) }
    ReaderFocus.shared.ensure(m); ReaderFocus.shared.catchUp(m)
    let t = thread(open: r)
    let began = ContinuousClock.now
    let reading = ReaderPassage.reading(m, t)
    let took = began.duration(to: .now)
    #expect(reading["marked"] as? Bool == false)
    #expect((reading["pieces"] as? [String])?.isEmpty == true)
    #expect(reading["text"] as? String == textSnippet && reading["source"] as? String == "text")   // the hand-over is there, the mark is not
    #expect(took < .milliseconds(50), "the row took \(took)")
    let empty = MessageBodyContent(id: "no-body-stored-1", safeHtml: nil, plainText: "", trackersBlocked: 0, quoteOffset: nil)
    #expect(try #require(ReaderPassage.current(m)).page(in: t, content: empty) == nil)
}

// broken copy: PassageMemo.page always scans (the key check removed): a redraw of the pane scans the text again
@MainActor @Test func theTextIsScannedOncePerAnswerNotPerDraw() {
    let memo = PassageMemo()
    let before = memo.scans
    for _ in 0..<5 { _ = memo.page(id: "m1", passage: "the revised price", letters: ["price"], text: "all of the revised price list") }
    #expect(memo.scans == before + 1)
    _ = memo.page(id: "m1", passage: "the revised price", letters: ["revised"], text: "all of the revised price list")
    #expect(memo.scans == before + 2)                                 // other letters: a new answer
}

// broken copy: ReaderPassage.reading serves "marked" true for an empty `pieces`, omits "source" or "text", or the open mail's body is not the one the pane draws
@MainActor @Test func theRowTheChecksReadServesMarkedTextAndSourceForTheOpenMail() throws {
    let m = try app()
    let inbox = try #require(m.rows.first)
    let t = try #require(ReaderThread.make(m, messageId: inbox.messageId, resting: true))
    let plain = try #require(t.shownBody(m)?.plainText, "the fixture's first mail has a stored body")
    let line = SearchLetters.oneLine(plain)
    let word = try #require(line.prefix(80).split(separator: " ").first { w in w.count >= 4 && w.allSatisfy(\.isLetter) && !inbox.subject.localizedCaseInsensitiveContains(w) && !inbox.senderName.localizedCaseInsensitiveContains(w) }.map(String.init), "a word of four letters in the first line")
    let s = showing(m, answer(word, [inbox], [hit(inbox, .textWords(ranges: []), snippet: String(line.prefix(80)))])); defer { reset(s, m); ReaderFocus.shared.catchUp(m) }
    ReaderFocus.shared.ensure(m); ReaderFocus.shared.catchUp(m)
    let reading = ReaderPassage.reading(m, t)
    #expect(reading["marked"] as? Bool == true && reading["source"] as? String == "text" && reading["in_open_mail"] as? Bool == true)
    #expect((reading["text"] as? String)?.isEmpty == false)
    let pieces = try #require(reading["pieces"] as? [String])
    #expect(!pieces.isEmpty && pieces.allSatisfy { $0.lowercased() == word.lowercased() }, "marked: \(pieces)")
}

// broken copy: the ring condition in AttachmentChip / AttachmentGalleryTile writes `AttachmentStore.shared.selected` instead of reading `ReaderPassage.rings`
// (a selected chip holds the keys: hasFocus turns true and Enter/Esc go to the file), or `rings` is true for another file
#if os(macOS)
@MainActor @Test func theResultsFileRingsWithoutTheAttachmentsTakingTheKeys() throws {
    let m = try app(); let r = row("m1")
    let s = showing(m, answer("revised price", [r], [hit(r, .attachmentContent, snippet: "… the revised price …", file: "rooms-2026.xlsx")])); defer { reset(s, m); ReaderFocus.shared.catchUp(m) }
    AttachmentStore.shared.selected = nil
    ReaderFocus.shared.ensure(m); ReaderFocus.shared.catchUp(m)
    #expect(ReaderPassage.rings(messageId: "m1", name: "rooms-2026.xlsx"))
    #expect(AttachmentStore.shared.selected == nil)
    #expect(!AttachmentsMacFeature.hasFocus(m), "the ring must not give the attachments the keys")
}
#endif
