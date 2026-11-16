import Foundation

/// Save as smart folder, the Edit control on a folder, the recent searches kept, and the rows the screen searches.
@MainActor
extension SearchMacModel {
    public func startSaving() -> Bool {
        guard active else { return false }
        claim()                                       // a click on Save lands after the click-away release: the name field takes the keys
        if naming { app.typed = ""; fieldSays = ""; return true }   // Save again after a refusal: the name field starts clean
        guard launched, !SearchQuery.typed(shownText).isEmpty else { return false }
        savedQuery = shownText; naming = true; fieldSays = ""; app.typed = ""
        return true
    }

    /// Enter in the name field. The core saves the folder; its refusal ("the name exists already") is said in the field
    /// in the core's own words. A name that is a mailbox's is refused here in the same words.
    public func confirmName() -> Bool {
        let name = app.typed.trimmingCharacters(in: .whitespacesAndNewlines)
        if name.isEmpty { fieldSays = Words.text("Give the folder a name"); return true }
        if app.counts.mailboxes.keys.contains(where: { $0.caseInsensitiveCompare(name) == .orderedSame }) { fieldSays = Words.text("the name exists already"); return true }
        do {
            _ = try app.core.run(command: .saveSmartFolder(name: name, ruleWords: savedQuery))
        } catch CoreError.Invalid(_, let words) {
            fieldSays = words; return true
        } catch CoreError.Action(let failure) {
            fieldSays = failure.reasonWords; return true
        } catch {
            fieldSays = "\(error)"; return true
        }
        app.lastFailure = nil
        app.refresh()
        remember(savedQuery)
        naming = false; fieldSays = ""; text = savedQuery
        reloadFolders()
        return true
    }

    /// The core's smart folders, each with the threads its rule finds: the same reading as the field's (the core's hits for plain
    /// words, the local engine for filters alone or a quoted figure), so a folder lists exactly the threads its query lists.
    func reloadFolders() {
        folders = app.core.smartFolders().map { f in
            let found = threads(rule: f.ruleWords)
            return Folder(id: f.id, name: f.name, rule: f.ruleWords, threads: found, count: listedCount(f.id), says: refusedWords(rule: f.ruleWords) == nil ? "" : Self.refusedSentence)
        }
    }

    /// The rows the main list shows when the sidebar opens the folder: the core's own count for that query
    /// (`smart_folder/<id>/count`, the figure the sidebar row shows), else the rows of the query itself.
    func listedCount(_ id: String) -> Int {
        if let n = app.core.state(path: "smart_folder/\(id)/count").flatMap({ Int($0) }) { return n }
        return app.core.inbox(query: InboxQuery(tabId: app.tab, accountId: nil, labelId: nil, mailbox: nil, accountIds: [], filters: [], smartFolderId: id)).count
    }

    /// A folder row is read now, never from the save: when the core's count moved (an archive from the folder's own list or
    /// anywhere else) the mail the screen searches is read again and the folders are rebuilt, so count and threads follow the list.
    func syncFolders() {
        guard !folders.isEmpty, folders.map(\.count) != folders.map({ listedCount($0.id) }) else { return }
        corpus = Self.everyRow(app)
        reloadFolders()
    }

    /// The threads a rule finds, best first, one per thread. A rule is read exactly like typed text. Plain words are asked of the ENGINE like any
    /// search and the matches and the related mail count (SPEC A150; the rest behind the closed line is not the folder's); the folder fills in when the answer lands.
    /// An engine that refuses lists nothing (`refusedWords`), never another engine's answer. A quoted figure or filters alone are not an engine
    /// search by design and run on the local engine.
    func threads(rule: String) -> [String] {
        let q = SearchQuery.typed(rule)
        guard let words = SearchEngine.engineQuery(q) else {
            return ordered(SearchEngine.run(q, over: corpus, beforeMs: Self.monthStartMs, names: attachmentNames))
        }
        subscribeSearch()                             // the answer comes on the Search stream: listen before asking
        if hitCache[words] == nil, !folderWordsPending.contains(words), folderRefusals[words] == nil {
            folderWordsPending.insert(words)
            do {
                if let ask = engineAsk { try ask(words) } else { _ = try app.core.run(command: .search(query: words)) }
            } catch {
                folderWordsPending.remove(words)
                folderRefusals[words] = Self.engineWords(error)
            }
        }
        guard let hits = hitCache[words] else { return [] }
        var outcome = SearchEngine.fromHits(q, hits: hits, rows: hitRows(words, hits), beforeMs: Self.monthStartMs, names: attachmentNames, engineFiltered: true)
        // SPEC A150: the query lists its matches, then its related mail; "All other mail" stays behind the closed line. The folder lists what the query
        // lists, so it keeps the related mail and drops only the rest (main93 par-folder-same-threads: thr-s06, related, was missing from the folder)
        outcome.results.removeAll { $0.part == .rest }
        return ordered(outcome)
    }

    /// The engine's words when it refused this rule's plain words; nil when it did not.
    func refusedWords(rule: String) -> String? {
        SearchEngine.engineQuery(SearchQuery.typed(rule)).flatMap { folderRefusals[$0] }
    }

    private func ordered(_ o: SearchEngine.Outcome) -> [String] {
        var seen = Set<String>()
        return o.results.map(\.row.threadId).filter { seen.insert($0).inserted }
    }

    /// The Edit control on a smart folder: the labels screen's rule editor opens on that folder.
    public func editFolder(_ i: Int) -> Bool {
        guard folders.indices.contains(i) else { return false }
        let targets = LabelsContactsMacFeature.clickTargets()
        // the editor keys a folder by the core's id (sf-1) or, on older builds, by the slug of its name
        guard let edit = targets["mac-labels-contacts.edit.\(folders[i].id)"] ?? targets["mac-labels-contacts.edit.\(LabelsContactsSource.slug(folders[i].name))"]
        else { return false }
        app.typed = ""                                 // the search words are not the rule editor's keystrokes: `takeTyped` would append them to the folder's rule
        guard edit(app) else { return false }
        return WindowBridge.open(LabelsContactsMacFeature.slot, in: app)
    }

    /// A launched or saved search is a recent one: Command.RecordSearch, then the core's `search/recent` read.
    func remember(_ q: String) {
        let t = q.trimmingCharacters(in: .whitespaces)
        guard !t.isEmpty else { return }
        _ = try? app.core.run(command: .recordSearch(query: t))
        reloadRecents()
    }

    static func everyRow(_ app: AppModel) -> [InboxRow] {
        var seen = Set<String>(), out: [InboxRow] = []
        for t in app.tabIds {
            for r in app.core.inbox(query: InboxQuery(tabId: t, accountId: nil, labelId: nil)) where seen.insert(r.messageId).inserted { out.append(r) }
        }
        return out
    }
}
