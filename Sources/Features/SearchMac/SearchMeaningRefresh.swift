import Foundation

/// While the meaning index builds in the background, `MeaningSearch.rank` answers with the hits found so far and fires
/// `MeaningSearch.onUpdate` as vectors arrive (embedding round 8). The search screen then asks its launched words again,
/// so meaning hits join the word hits on screen instead of the first answer waiting for the whole index. No skeleton on a
/// re-ask: the rows on screen stay until the new answer replaces them. The saved folders re-read their rules the same way.
@MainActor
extension SearchMacModel {
    /// Installed once, when the screen first listens to the Search stream.
    func listenForMeaning() {
        guard !meaningListening else { return }
        meaningListening = true
        MeaningSearch.onUpdate = { Task { @MainActor in SearchMacModel.shared.meaningGrew() } }
        // `fixture reset` (test copy only: the verb exists only on the control socket) puts the shell on the inbox; the search closes with it, so the
        // next ⌘K opens a fresh search instead of the last journey's launched words and opened result (main88 again-046: J5.4 after J2 landed on the reader)
        NotificationCenter.default.addObserver(forName: Notification.Name("whoosh.fixture.reset"), object: nil, queue: .main) { _ in
            MainActor.assumeIsolated { SearchMacModel.shared.closeForReset() }
        }
    }

    /// The search as at launch: closed, nothing launched, no result to come back to.
    func closeForReset() {
        naming = false
        clearField()
        returnToSearch = false
        openedMessageId = nil
        leave()
    }

    /// One re-ask per burst of updates.
    /// SEAM (marked, round 9): debounce by WhooshConstants.EMBED_NOTIFY_MS once contracts lands that row. Until then the
    /// builder's own throttle (it fires at most every EMBED_NOTIFY_MS) and this one-pending flag are the only debounce;
    /// this file holds no number of its own.
    func meaningGrew() {
        guard !reaskPending else { return }
        reaskPending = true
        Task { @MainActor in
            await Task.yield()
            reaskPending = false
            reask()
        }
    }

    /// Asks the core the launched query again (the pills and the words, `SearchEngine.engineQuery`; a refusal is said as a first search's is) and drops the cached answers, so folders re-read too. `reaskCount` is what
    /// par-searchmac reads to tell a re-ask from a first answer.
    func reask() {
        hitCache = [:]; hitRowsCache = [:]
        if !folders.isEmpty { reloadFolders() }
        guard active, launched, let words = SearchEngine.engineQuery(launchedQuery) else { return }
        reaskCount += 1
        awaiting = words
        do { _ = try app.core.run(command: .search(query: words)) } catch { awaiting = ""; hitsFor = ""; hits = []; refusal = Self.engineWords(error) }
    }
}
