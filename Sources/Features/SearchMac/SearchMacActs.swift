import Foundation
import SwiftUI

/// What the search screen does: open, type, move, accept a suggestion, launch, open a result, chips, recents, save.
@MainActor
extension SearchMacModel {
    /// Opens the screen, or (already open) gives the field focus again. `returning` is Escape from a thread this
    /// screen opened: the text, the chips, the results and the selection come back (A148: a result opens with the search
    /// still open, so they are all still there and the field takes the keys again).
    public func open(returning: Bool = false) -> Bool {
        if active && !returning { claim(); caret = true; picked = []; focusTick += 1; return true }   // `/` or ⌘K over listed results: the cursor is back in the words
        if !active {
            corpus = Self.everyRow(app)
            hitCache = [:]; hitRowsCache = [:]; folderWordsPending = []; folderRefusals = [:]           // answers for a mailbox that has since changed are not reused
            loadSuggestSources()
            subscribeSearch()
            if returning {
                held = returnQuery; selected = returnSelected
                launchedText = returnQuery; shownText = returnQuery; launched = returnLaunched; loading = false
                if launched { runSearch() }
            } else {
                edited = [:]
                held = ""; selected = 0; launched = false; launchedText = ""; shownText = ""; loading = false; outcome = SearchEngine.Outcome(); refusal = nil; stage = ""; stageReason = ""
                restOpen = false; onMore = false
            }
            naming = false; fieldSays = ""; notice = ""; dismissed = false; highlight = 0
        }
        reloadFolders()
        returnToSearch = false
        app.overlay = nil
        // no window: the field is the main window's own list-header field; the surface word routes the keys to this screen
        active = true
        claim()                                       // the field takes the keyboard (and the surface word that routes keys to it)
        caret = !returning                            // J5.6: back from an opened result the keys are on the results, not on the field's words
        picked = []
        WindowBridge.bringMainToFront()
        focusTick += 1
        if returning && launched { toResults() }      // back from an opened result: the keys are on the results again, as after Return
        return true
    }

    /// Escape: the name field closes (when naming); otherwise the text clears, the dropdown and the results go, and the keys go back to the list row
    /// that had them (the owner: "esc doesn't work"). One press, never a stack of layers.
    public func close() -> Bool {
        if naming { naming = false; text = savedQuery; fieldSays = ""; return true }
        clearField()
        returnToSearch = false
        leave()
        return true
    }

    /// The field empties and the list pane goes back to the inbox (Esc with words in the field, the field's clear button).
    public func clearField() {
        text = ""; notice = ""; launched = false; launchedText = ""; shownText = ""; loading = false; holdSkeleton = false
        outcome = SearchEngine.Outcome(); refusal = nil; stage = ""; stageReason = ""; selected = 0; highlight = 0; dismissed = false
        restOpen = false; onMore = false; picked = []
    }

    /// Back to the inbox: the field is the list header's own again and the list shows its rows.
    func leave() {
        release("search-closed")
        active = false
        app.surface = "mac-main"
        WindowBridge.bringMainToFront()
    }

    /// The field's text changed (a key, the socket's `type`): the dropdown opens again, its highlight on the first row.
    public func typedChanged() {
        guard holds, !naming else { return }
        held = app.typed
        dismissed = false; highlight = 0
        SearchQueryVector.typed(SearchQuery.typed(held))
        narrowLive()
    }

    /// The list follows the filters as they are typed (JOURNEYS J2.2: "the list narrows on each chip without a new Enter"). When the
    /// chips in the field differ from the ones the list shows and the words are the same (the core's hits are already in hand) or the
    /// field holds filters alone (the local engine reads them), the list is built again from the field now. Changed words wait for Enter.
    /// A key that leaves the chips as they are does no work here: the parse of the shown text is the only cost.
    func narrowLive() {
        guard launched, !loading, !naming else { return }
        let cur = SearchQuery.typed(held), shown = launchedQuery
        let sameWords = cur.words == shown.words && cur.phrase == shown.phrase
        let filtersAlone = cur.words.isEmpty && cur.phrase == nil && !cur.chips.isEmpty
        // the words going away while the same chips stay (the field selected and a filter typed over the words) is a change too
        guard cur.chips != shown.chips || (filtersAlone && !sameWords) else { return }
        guard cur.chips.filter({ !shown.chips.contains($0) }).allSatisfy(Self.settled) else { return }
        guard sameWords || filtersAlone else { return }
        shownText = held
        selected = 0; restOpen = false; onMore = false
        runSearch()
    }

    /// A chip whose words are complete: the colon form Tab writes (`from:"Linea Supply"`), a date, "has attachment". Typed words never make a
    /// sender chip (SPEC A142), so every chip the field reads is complete.
    static func settled(_ c: SearchChip) -> Bool { c.source.contains(":") || c.kind == .before || c.kind == .attachment }

    /// ↑ ↓: the dropdown's highlight while the field is being edited, else the selected result. The closed line "All other mail" is a stop of its own
    /// between the related rows and the rest (SPEC A150): Down from the last row above it reaches it, Down again goes to the first of the rest while it is open.
    public func move(_ d: Int) -> Bool {
        if dropdownVisible {
            choose(min(max(0, highlight + d), suggestions.count - 1))
            return true
        }
        let n = results.count, k = upperCount
        if showsMore {
            if onMore {
                if d < 0, k > 0 { onMore = false; selected = k - 1; return true }
                if d > 0, restOpen, n > k { onMore = false; selected = k; return true }
                return true
            }
            if d > 0, selected == k - 1 { onMore = true; return true }
            if d < 0, restOpen, selected == k { onMore = true; return true }
        }
        guard n > 0 else { return false }
        selected = min(max(0, selected + d), n - 1)
        return true
    }

    /// Enter. In the field: the typed words launch, unless the owner moved to another row with the arrow keys (or clicked it): that row is
    /// accepted and launched. A pointer resting on a row (`hovered`) never counts. On the results: the selected result opens.
    public func enter() -> Bool {
        if naming { return confirmName() }
        // the label picker the selected results opened (J5.6): Enter labels them all, never opens a mail
        if !picked.isEmpty, app.overlay == Overlay.label, case .label(let ids)? = InboxListState.shared.pending { return finishPickedLabel(ids) }
        if fieldMode {
            let rows = suggestions
            if dropdownVisible, keyed, rows.indices.contains(highlight), rows[highlight].kind != .search {
                accept(rows[highlight])
            }
            return launch()
        }
        if onMore { return toggleRest() }   // the closed line is selected: Enter opens or closes it, it opens no mail
        return openResult()
    }

    /// Tab: the highlighted suggestion becomes a chip (a person, a label, a "contains" word), or fills the field (a recent search, a folder).
    public func tab() -> Bool {
        let rows = suggestions
        guard dropdownVisible, rows.indices.contains(highlight), rows[highlight].kind != .search else { return false }
        accept(rows[highlight])
        return true
    }

    func accept(_ s: SearchSuggestion) {
        if let chip = s.chip {
            if s.kind == .from || s.kind == .to { pillNames[chip.value.lowercased()] = s.title }
            var draft = SearchQuery.draft(of: text)
            if let w = s.replaces, let r = draft.range(of: w, options: [.caseInsensitive, .backwards]) { draft.removeSubrange(r) }
            let chips = SearchQuery.typed(text).chips.filter { $0.kind != chip.kind } + [chip]
            text = SearchQuery.text(chips: chips, draft: draft.split(whereSeparator: \.isWhitespace).joined(separator: " "))
        } else if let words = s.words {
            text = words
        }
        highlight = 0
    }

    /// Launches the field's words: the list pane swaps to skeleton rows on this frame, each row crossfades into its result as the answer lands.
    public func launch() -> Bool {
        guard !naming, !SearchQuery.typed(text).isEmpty else { return false }
        launchedText = text; shownText = text
        remember(launchedText)
        selected = 0; highlight = 0; dismissed = true; notice = ""
        restOpen = false; onMore = false                            // every new search closes "All other mail"
        picked = []
        launched = true; loading = true
        // Return always asks the engine again, also for the same words: the mail or the meaning index may have changed since (J5.4 relaunches the question
        // while the index builds; runSearch alone reuses an answer for unchanged words, which is right for a chip removed, not for Return)
        hitsFor = ""; hitRowsCache = [:]
        runSearch()
        settleSoon()
        toResults()                                                 // the words are launched: the keys are the results' (SearchFocus.toResults)
        return true
    }

    /// The answer is in: skeleton rows give way to the results over SEARCH_SKELETON_FADE_MS.
    func settle() {
        withAnimation(.easeOut(duration: Double(WhooshConstants.SEARCH_SKELETON_FADE_MS) / 1000)) { loading = false }
    }

    public func openResult(_ index: Int? = nil) -> Bool {
        let rs = results, i = index ?? selected
        guard rs.indices.contains(i) else { return false }
        onMore = false; picked = []
        let id = rs[i].row.messageId
        if Self.isGone(app.core.message(messageId: id)) { forget(messageId: id, at: i); return true }   // deleted or moved to Trash since the search ran
        // The recent search (a core write and a read of the recents) shows in no part of the reader this Enter opens: it is kept after the
        // frame's commit, before the next input (round 17, J2.4/one-interaction-budget-ms 61.8 ms on main105)
        let words = launchedText
        SearchAfterCommit.run { [self] in remember(words) }
        selected = i                                                // a click on a result opens that one: it is the selection the reader shows
        returnQuery = shownText; returnSelected = i; returnLaunched = true
        openedMessageId = id
        // A quick reply still open on ANOTHER thread is put away first (kept as a draft, QuickReplyModel.close): left open, it holds the reader's keys and
        // takes Esc for itself, so Esc closes that reply instead of bringing the results back (main95 all journeys: J1.6 left a reply open on msg-04,
        // J5.6's Esc from record 1 stayed on the reader with `editing=no`).
        // Round 11: also a reply open on THIS thread (main103 all journeys: J3 works on record 1's thread and leaves its reply open; J5.5 opens record 1
        // from the results and J5.6's Esc closed that reply instead of coming back). A result opens as the reader at rest; the draft is kept and its
        // card shows in the thread.
        let reply = QuickReplyModel.shared
        if reply.isOpen { _ = reply.close(app) }
        _ = app.perform(.openThread(threadId: rs[i].row.threadId))   // as the inbox list's open does (TriageActions.open)
        edited[id] = (unread: false, pinned: edited[id]?.pinned)      // an opened conversation is read: its row loses the unread dot now, as the inbox row does (lead, 13 Nov: the dot stayed on an opened result)
        // A148 (owner 2026-11-11: "when I go enter on this email , it doesn't open it, it just goes straight to no search"): the search stays
        // open and its results stay in the list column; the field lets the keys go to the reader. The reader reads the selected result's own
        // row (AppModel.shownRow), wherever the mail is, so no inbox tab is looked up or switched: memory only, plus the open's one command.
        release("result-opened")
        app.overlay = nil
        ReaderState.shared.deletedNote = nil
        ReaderFocus.shared.settleForOpen(app)                       // an open is not a passing focus: the pane is on this mail in the same turn
        app.surface = "mac-reader"                                  // the picked three-pane design: the thread opens in the reader pane
        MacShellState.shared.focusedPane = .reader                  // READER whole, as the reader's own open door does: its keys (J, K, Esc) come first
        returnToSearch = true
        return true
    }

    /// J / K while the reader shows a result this screen opened: the next or previous listed result opens in its place, so a search is read
    /// through without a trip back to the list for each mail. False at either end of the list (nothing moves).
    public func openNeighbour(_ d: Int) -> Bool {
        let i = selected + d
        guard results.indices.contains(i) else { return false }
        return openResult(i)
    }

    /// A message that no longer exists, or sits in the Trash, is not opened.
    static func isGone(_ message: Message?) -> Bool { message == nil || message?.folder == "Trash" }

    /// Opening a result whose message has gone: the row leaves the results (and the rows later searches are built from), the selection
    /// stays at the same place, and the pane says so in the catalogue's words. Nothing opens.
    func forget(messageId id: String, at index: Int) {
        drop([id], at: index)
        notice = Words.text("no longer in your mail")
    }

    /// The messages leave the results and the rows later searches are built from; the selection stays at `index` (the next result moves up under it).
    private func drop(_ ids: [String], at index: Int) {
        let gone = Set(ids)
        corpus.removeAll { gone.contains($0.messageId) }
        hits.removeAll { gone.contains($0.messageId) }
        hitCache = hitCache.mapValues { $0.filter { !gone.contains($0.messageId) } }
        hitRowsCache = hitRowsCache.mapValues { $0.filter { !gone.contains($0.messageId) } }
        outcome.results.removeAll { gone.contains($0.row.messageId) }
        selected = min(index, max(0, results.count - 1))
    }

    /// A command the core accepted while results show (AppModel.perform). Mark done, delete, junk, snooze and move take the mail out of the
    /// mailbox: it leaves the results as a gone result does, with no notice (the person did it), and the selection stays at the same place so
    /// the reader follows the next result. U and pin change the result's row on screen for the toggles. Z puts the mail back in the core
    /// but not in this list: the next search (the screen opened again) finds it. Reads memory only; one pass over the result rows.
    func acted(_ command: Command) {
        guard showsResults else { return }
        switch command {
        case .archive(let ids), .markSpam(let ids), .deleteMessages(let ids), .snooze(let ids, _), .moveToLabel(let ids, _):
            leaveResults(ids)
        case .deleteMessage(let id):
            leaveResults([id])
        case .markRead(let ids, let read):
            for id in ids { edited[id] = (unread: !read, pinned: edited[id]?.pinned) }
        case .pin(let id, let pinned):
            edited[id] = (unread: edited[id]?.unread, pinned: pinned)
        case .undo:
            // Z brought a mail back (a Mark done, a Snooze, a Junk or a delete made on these results): the same words are asked again, so the
            // mail is back in the list the owner is looking at, where it was. Before 13 Nov it came back in the mailbox only and the results kept
            // the hole until the next search (lead, owner A162: with the keys on the results, E then Z must read as "nothing happened").
            guard launched, !loading else { break }
            let keep = selected
            hitsFor = ""; hitRowsCache = [:]; corpus = Self.everyRow(app)
            runSearch()
            selected = min(keep, max(0, results.count - 1))
        default:
            break
        }
    }

    private func leaveResults(_ ids: [String]) {
        let gone = Set(ids), rs = results
        guard let first = rs.firstIndex(where: { gone.contains($0.row.messageId) }) else { return }
        drop(ids, at: first)
    }

    /// A chip's remove button: its words leave the field, and the list widens at once with no new Enter.
    public func removeChip(_ index: Int) -> Bool {
        let chips = query.chips
        guard !naming, chips.indices.contains(index) else { return false }
        text = SearchQuery.removing(chips[index], from: text)
        selected = 0; restOpen = false; onMore = false
        if launched { launchedText = text; shownText = text; runSearch() }
        return true
    }

    public func useRecent(_ index: Int) -> Bool {
        guard recents.indices.contains(index) else { return false }
        naming = false; text = recents[index]; selected = 0
        return launch()
    }
}

/// J5.6 (docs/JOURNEYS.md): ⌘A selects every result, L labels them all through the list's own picker, Z undoes the batch as one entry.
@MainActor
extension SearchMacModel {
    /// ⌘A. While the caret is in the field (⌘K, `/`, a click on it), or the field holds edited words, it selects the field's words as it always did
    /// (the words are replaced by what is typed next). Back from an opened result (Esc), with the launched words untouched and results listed, it
    /// selects every listed result and the keys go to the results (the field keeps its words; L, Enter, Z act on the selection).
    func selectAllKey() -> Bool {
        guard !(holds && caret), launched, text == launchedText, !results.isEmpty else {
            text = ""; selected = 0; highlight = 0; picked = []
            return true
        }
        picked = Set(results.map(\.row.threadId))
        if holds { owns = false; dismissed = true }      // the field lets the keys go but the screen stays the search (no `release`: it would leave for the inbox)
        return true
    }

    /// Every message of the selected conversations (the core's thread read; a thread it cannot read gives its listed message).
    var pickedMessageIds: [String] {
        let rows = results.filter { picked.contains($0.row.threadId) }.map(\.row)
        var seen = Set<String>()
        return rows.flatMap { r in app.core.thread(threadId: r.threadId)?.messageIds ?? [r.messageId] }.filter { seen.insert($0).inserted }
    }

    /// L with results selected: the list's own label picker (InboxList TriageActions.openPicker, overlay `mac-main.label`) on every selected message.
    func labelPickedKey() -> Bool {
        let ids = pickedMessageIds
        guard !picked.isEmpty, !ids.isEmpty else { return false }
        return TriageActions.openPicker(app, { _ in .label(messageIds: ids) }, overlay: Overlay.label)
    }

    /// The typed name is a label that does not exist yet: the picker offers to create it.
    func offersCreate(_ name: String) -> Bool {
        let n = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !n.isEmpty else { return false }
        return !app.core.labels().contains { $0.name.caseInsensitiveCompare(n) == .orderedSame }
    }

    /// Enter in the picker the search opened: ONE `label` command for every selected message (one undo entry), the label made when it is new
    /// (the picker offered Create); the list's undo capsule says it; the results read their rows again so the chips show.
    func finishPickedLabel(_ ids: [String]) -> Bool {
        let words = app.typed.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !words.isEmpty else { return false }
        let create = offersCreate(words)
        let ok = app.perform(.label(messageIds: ids, labelName: words, createIfMissing: create))
        TriageActions.close(app)
        if ok {
            InboxListState.shared.say(InboxListState.Pending.label(messageIds: ids).doneWords(words))
            hitRowsCache = [:]; corpus = Self.everyRow(app)
            runSearch()
        }
        return ok
    }
}
