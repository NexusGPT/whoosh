import Foundation
#if os(macOS)
import AppKit
#endif

/// The notch by keyboard (owner's picks of 14 Nov 2026: search in the notch's own field, the notch's key from any app). Rules:
///   - the pointer alone never takes the keyboard; the notch's key, "/" and a click in the search field do (`keyboardWanted`);
///   - the cursor is in the search field or on a row, never both: Down leaves the field for the first mail, Up from it goes back;
///   - a letter typed while a row has the keyboard goes to the search field, except the keys that act on a mail (the preset's archive,
///     read, reply and snooze keys), which act on that row;
///   - Escape goes back one step (`back`).
extension NotchModel {
    /// The notch holds the keyboard right now (NotchKeyboard took it and the panel is the key window).
    var keyboardHeld: Bool { NotchRows.keyed }

    /// The notch's key (⌥M to start; Settings > Notifications changes it), from any app. It opens the notch with the keyboard: on a new mail's
    /// card or line the keys act on that mail; else the list opens with the cursor in the search field. Pressed while the notch has the
    /// keyboard, it closes the notch and the keyboard goes back to the app that had it.
    @discardableResult
    public func summon(_ m: AppModel) -> Bool {
        if keyboardWanted || keyboardHeld { return collapse() }
        if focusOn && settings.followFocus { return false }
        keyboardWanted = true
        switch stage {
        case .drop, .line, .open: break                                    // what shows stays: its keys act on its mail, "/" goes to search
        case .rest, .light, .list: stage = .list; searchFocused = true; rowFocusId = nil
        }
        takeKeyboard()
        onChange()
        return true
    }

    /// "/" and a click on the field: the cursor goes to the search field; from a card or an open mail the list comes first.
    @discardableResult
    public func focusSearch() -> Bool {
        if focusOn && settings.followFocus { return false }
        if case .open = stage { endOpenReply() }
        stage = .list
        searchFocused = true
        rowFocusId = nil
        keyboardWanted = true
        takeKeyboard()
        onChange()
        return true
    }

    /// An answer arrived from the engine after the keyboard went to a row (the engine answers the same words twice: the word hits, then the
    /// full ranking): a row the new answer no longer lists gives the keyboard back to the field, so the keyboard is never on nothing.
    func answerArrived() {
        if let id = rowFocusId, !shownRows.contains(where: { $0.id == id }) { rowFocusId = nil; searchFocused = keyboardWanted }
        onChange()
    }

    /// The notch's panel stopped being the key window while the person had taken the notch with the keyboard (a click in another app or
    /// window): they left, so the notch closes, as it does when the pointer leaves a notch opened by the pointer. A reply with words in it,
    /// or a send still in its undo window, keeps its mail open (the rule the pointer already follows). Without this a notch taken with its
    /// key stayed open on the screen with no keyboard in it until the key was pressed again.
    /// `settled` false (the window's own event): a loss inside NOTCH_KEY_SETTLE_MS of the take is the app's activation settling (the main
    /// window can take the key status right after the panel did), not the person leaving: the panel takes the keyboard back, once.
    @discardableResult
    public func keyboardLost(settled: Bool = false) -> Bool {
        guard keyboardWanted else { return false }
        if !settled, let at = keyboardTakenAt, Date().timeIntervalSince(at) * 1000 < Double(WhooshConstants.NOTCH_KEY_SETTLE_MS) {
            guard !keyboardRetaken else { return false }
            keyboardRetaken = true
            #if os(macOS)
            NotchKeyboard.shared.take()
            #endif
            return false
        }
        keyboardWanted = false
        if case .open = stage, !QuickReplyModel.shared.draft.isEmpty || QuickReplyModel.shared.isCounting {
            onChange()
            return true
        }
        if pointerOn, stage == .list {      // the pointer is on the list: it stays, as a list opened by the pointer
            searchFocused = false
            rowFocusId = nil
            onChange()
            return true
        }
        return collapse()
    }

    func takeKeyboard() {
        keyboardTakenAt = Date()
        keyboardRetaken = false
        #if os(macOS)
        NotchPanel.shared.install()
        NotchKeyboard.shared.take()
        #endif
    }

    /// The field's text changed (a typed letter, a paste, the clear mark). A row that left the answer loses the keyboard to the field.
    public func setSearch(_ text: String, _ m: AppModel) {
        search.set(text, m)
        if let id = rowFocusId, !shownRows.contains(where: { $0.id == id }) { rowFocusId = nil; searchFocused = keyboardWanted }
        onChange()
    }

    /// Letters that reach the notch with no text field under them (the socket's `type`; a letter pressed while a row has the keyboard): they
    /// go to the search field, which takes the cursor back.
    @discardableResult
    public func typeSearch(_ text: String, _ m: AppModel) -> Bool {
        guard stage == .list, keyboardWanted, !text.isEmpty else { return false }
        searchFocused = true
        rowFocusId = nil
        setSearch(search.text + text, m)
        return true
    }

    /// Backspace with a row under the keyboard: the search loses its last letter and the cursor goes back to the field.
    @discardableResult
    public func eraseSearch(_ m: AppModel) -> Bool {
        guard stage == .list, keyboardWanted, !search.text.isEmpty else { return false }
        searchFocused = true
        rowFocusId = nil
        setSearch(String(search.text.dropLast()), m)
        return true
    }

    /// Down and Up: from the search field Down goes to the first mail; Up from the first mail goes back to the field.
    @discardableResult
    public func moveRow(_ delta: Int) -> Bool {
        guard stage == .list else { return false }
        let rows = shownRows
        guard let id = rowFocusId, let i = rows.firstIndex(where: { $0.id == id }) else {
            guard delta > 0, let first = rows.first else { return false }
            rowFocusId = first.id
            searchFocused = false
            onChange()
            return true
        }
        let next = i + delta
        if next < 0 {
            rowFocusId = nil
            searchFocused = true
            onChange()
            return true
        }
        guard next < rows.count else { return false }
        rowFocusId = rows[next].id
        onChange()
        return true
    }

    /// The mail a key acts on: the open mail; on the list the row under the keyboard; on a card or a line the alert's mail.
    var keyTarget: NotchItem? {
        switch stage {
        case .open: return openItem
        case .list: return focusedRow
        case .drop, .line: return notice == nil ? dropItem : nil
        case .rest, .light: return nil
        }
    }

    /// Return: opens the mail in the notch. On the list it is the row under the keyboard, or the answer's first mail while the cursor is in the
    /// search field; on a card or a line it is the alert's mail (a sign-in code's card keeps its own Copy and Fill).
    @discardableResult
    public func enter(_ m: AppModel) -> Bool {
        switch stage {
        case .drop, .line:
            guard notice == nil, let it = dropItem, it.code == nil else { return false }
            return open(id: it.id, m)
        case .list:
            // Return acts in the list only for a notch taken with the keyboard: a pointer resting on the notch never takes the main window's Return
            guard keyboardWanted, let it = focusedRow ?? (search.active ? shownRows.first : nil) else { return false }
            return openRow(it, m)
        case .rest, .light, .open: return false
        }
    }

    /// Opens a row of the list or of the search's answer in the notch. A found mail joins the notch's mails first, so every act works on it;
    /// a search that led to a mail is kept as a recent search, as Return does in the main window.
    @discardableResult
    func openRow(_ it: NotchItem, _ m: AppModel, reply: Bool = false) -> Bool {
        adopt(it)
        if search.active { _ = try? m.core.run(command: .recordSearch(query: search.text.trimmingCharacters(in: .whitespacesAndNewlines))) }
        guard open(id: it.id, m, reply: reply) else { return false }
        if keyboardWanted { rowFocusId = it.id; searchFocused = false }   // Escape comes back to this row
        return true
    }

    /// A mail the search found joins `items` (marked `found`), so open, archive, read, snooze and delete act on it like on an arrival.
    func adopt(_ it: NotchItem) {
        guard !items.contains(where: { $0.id == it.id }) else { return }
        var found = it
        found.found = true
        items.append(found)
    }

    /// One of the mail keys on the list's row or on an alert (archive, read, delete): the act runs on that mail, the answer drops it, and the
    /// keyboard moves to the row that took its place. An alert the person took with the keyboard closes once its mail is acted on.
    @discardableResult
    func act(on it: NotchItem, _ m: AppModel, _ run: (String, AppModel) -> Bool) -> Bool {
        let rows = shownRows
        let at = rows.firstIndex(where: { $0.id == it.id })
        adopt(it)
        guard run(it.id, m) else { return false }
        if !items.contains(where: { $0.id == it.id }) {          // the mail left the notch (archived, deleted)
            search.drop(id: it.id)
            switch stage {
            case .list:
                guard rowFocusId == it.id else { break }      // a pointer's click on a row's button leaves the keyboard where it is
                let now = shownRows
                if let at, !now.isEmpty { rowFocusId = now[min(at, now.count - 1)].id } else { rowFocusId = nil; searchFocused = keyboardWanted }
            case .drop, .line:
                if keyboardWanted { return collapse() }
            case .rest, .light, .open: break
            }
        }
        onChange()
        return true
    }

    /// Escape, one step back: the snooze choices close; an open mail goes back to the list it was opened from (a mail opened from an alert
    /// closes the notch); typed search words are emptied; then the notch closes. A reply being written closes first, by the reply's own Escape.
    @discardableResult
    public func back(_ m: AppModel) -> Bool {
        if case .open = stage {
            if snoozeId != nil { snoozeId = nil; onChange(); return true }
            guard openedFromList, search.active || !listed.isEmpty else { return collapse() }
            endOpenReply()
            stage = .list
            sentShown = false
            if let id = rowFocusId, !shownRows.contains(where: { $0.id == id }) { rowFocusId = nil }
            searchFocused = keyboardWanted && rowFocusId == nil
            onChange()
            return true
        }
        if stage == .list, search.active {
            rowFocusId = nil
            searchFocused = keyboardWanted
            setSearch("", m)
            return true
        }
        return collapse()
    }
}
