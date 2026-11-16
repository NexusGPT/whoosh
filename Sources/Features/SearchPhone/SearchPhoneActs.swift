import Foundation

/// What the owner does on the screen: type, filter, open a result, use a recent search. One small act per function, each answering
/// on the frame it is taken; the search itself is `SearchPhoneSearch.swift`.
@MainActor
extension SearchPhoneModel {
    // MARK: typing and filters

    /// A key, or the socket's `type`: the search runs on the next turn (the key's own frame is never held), and a read of the screen
    /// before that turn runs it first (`settle`).
    public func typedChanged() {
        guard active else { return }
        dirty = true
        if naming == false { savedAs = nil }
        guard !scheduled else { return }
        scheduled = true
        Task { @MainActor in
            await Task.yield()
            scheduled = false
            settle()
        }
    }

    /// Runs the search for what the field and the filters hold now, if they changed since the last run.
    public func settle() {
        if app.typed != seenText { dirty = true }
        guard dirty else { return }
        dirty = false
        seenText = app.typed
        refresh()
    }

    /// A filter changed: the rows follow on the same frame, no new Return.
    func filtersChanged() { savedAs = nil; dirty = true; settle() }

    @discardableResult
    func toggleMenu(_ m: SearchPhoneMenu) -> Bool { menu = menu == m ? nil : m; return true }

    @discardableResult
    func closeMenu() -> Bool { menu = nil; return true }

    @discardableResult
    func setAccount(_ id: String?) -> Bool {
        filters.accountId = filters.accountId == id ? nil : id
        menu = nil; filtersChanged(); return true
    }

    @discardableResult
    func setSender(_ index: Int) -> Bool {
        let names = senderNames
        guard names.indices.contains(index) else { return false }
        filters.sender = filters.sender == names[index] ? nil : names[index]
        menu = nil; filtersChanged(); return true
    }

    @discardableResult
    func setMonth(_ month: String) -> Bool {
        guard let m = SearchPhoneFilters.months.first(where: { $0.lowercased() == month.lowercased() }) else { return false }
        filters.beforeMonth = filters.beforeMonth == m ? nil : m
        menu = nil; filtersChanged(); return true
    }

    @discardableResult
    func toggleAttachment() -> Bool { filters.attachment.toggle(); menu = nil; filtersChanged(); return true }

    /// Takes a filter off (the × on its chip, or the picker's own clear).
    @discardableResult
    func clearFilter(_ kind: String) -> Bool {
        switch kind {
        case "account": filters.accountId = nil
        case "sender": filters.sender = nil
        case "date": filters.beforeMonth = nil
        case "attachment": filters.attachment = false
        case "domain": filters.domain = nil
        default: return false
        }
        menu = nil; filtersChanged(); return true
    }

    @discardableResult
    public func clearText() -> Bool { app.typed = ""; typedChanged(); settle(); return true }

    /// Return: the typed words are kept as a recent search; the rows are already there.
    @discardableResult
    public func submit() -> Bool {
        settle()
        guard !isIdleInput else { return false }
        remember(app.typed)
        return true
    }

    @discardableResult
    public func useRecent(_ index: Int) -> Bool {
        guard recents.indices.contains(index) else { return false }
        app.typed = recents[index]
        typedChanged(); settle(); return true
    }

    /// The people the sender picker lists: the senders of the rows that pass every other filter, in the order the rows are in.
    var senderNames: [String] {
        var seen = Set<String>()
        return candidates.map(\.row.senderName).filter { seen.insert($0).inserted }
    }

    // MARK: a result

    /// A result opens its thread over the search, as an inbox row does (focus the message, overlay `iphone-thread`).
    /// The text, the filters and the rows stay: closing the thread lands on them again.
    @discardableResult
    public func openResult(_ index: Int) -> Bool {
        settle()
        guard results.indices.contains(index) else { return false }
        let r = results[index].row
        let hit = Self.holder(of: results[index])
        remember(app.typed)
        if let tab = app.tabIds.first(where: { t in app.core.inbox(query: InboxQuery(tabId: t, accountId: nil, labelId: nil)).contains { $0.messageId == r.messageId } }) {
            app.tab = tab; app.accountFilter = nil
        }
        app.refresh(); app.focus(messageId: r.messageId)
        openedMessageId = r.messageId; openedHitMessageId = hit
        // the thread opens on the result's own mail (a result is often in no loaded row) with the earlier mail that holds the passage unfolded
        if !ThreadPhoneState.shared.open(messageId: r.messageId, unfolding: hit, in: app) { app.overlay = "iphone-thread" }
        return true
    }

    /// The mail that holds the passage of a result: the engine's `hitMessageId`, else the result's own mail (the Mac reader's rule, `ReaderPassage`).
    static func holder(of r: SearchResult) -> String { r.hitMessageId.isEmpty ? r.row.messageId : r.hitMessageId }

    // MARK: recents

    func reloadRecents() {
        guard let json = app.core.state(path: "search/recent"), let d = json.data(using: .utf8),
              let list = try? JSONDecoder().decode([String].self, from: d) else { recents = []; return }
        recents = list
    }

    /// A launched, opened or saved search is a recent one: Command.RecordSearch, then the core's `search/recent` read.
    func remember(_ q: String) {
        let t = q.trimmingCharacters(in: .whitespaces)
        guard !t.isEmpty else { return }
        _ = try? app.core.run(command: .recordSearch(query: t))
        reloadRecents()
    }
}
