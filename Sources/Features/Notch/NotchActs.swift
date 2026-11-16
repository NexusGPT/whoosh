import Foundation
#if os(macOS)
import AppKit
#endif

/// The notch's acts on a mail. Each changes the model on the frame it lands; the shape's motion follows.
extension NotchModel {
    /// The pill: the code goes on the clipboard and the mail is read, never archived.
    @discardableResult
    public func copyCode(_ m: AppModel, clipboard: (String) -> Void) -> Bool {
        guard let i = items.firstIndex(where: { $0.code != nil }), let code = items[i].code else { return false }
        clipboard(code)
        items[i].codeCopied = true
        if !items[i].read, m.perform(.markRead(messageIds: [items[i].id], read: true)) { items[i].read = true }
        onChange()
        return true
    }

    @discardableResult
    public func archive(id: String, _ m: AppModel) -> Bool {
        guard let index = items.firstIndex(where: { $0.id == id }) else { return false }
        guard m.perform(.archive(messageIds: [id])) else { return fail(m) }
        items.remove(at: index)
        search.drop(id: id)
        if case .open(let o) = stage, o == id { leaveMail() }
        if arrived.isEmpty, stage == .drop { stage = .rest }
        errorWords = nil
        onChange()
        return true
    }

    /// N12: Delete leaves every list and count and has no undo (contract delta 6). A refusal keeps the mail and says why.
    @discardableResult
    public func delete(id: String, _ m: AppModel) -> Bool {
        guard let index = items.firstIndex(where: { $0.id == id }) else { return false }
        guard m.perform(.deleteMessage(messageId: id)) else { return fail(m) }
        items.remove(at: index)
        search.drop(id: id)
        if case .open(let o) = stage, o == id { leaveMail() }
        if arrived.isEmpty, stage == .drop { stage = .rest }
        errorWords = nil
        onChange()
        return true
    }

    @discardableResult
    public func markRead(id: String, _ m: AppModel) -> Bool {
        guard let index = items.firstIndex(where: { $0.id == id }), !items[index].read else { return false }
        guard m.perform(.markRead(messageIds: [id], read: true)) else { return fail(m) }
        items[index].read = true
        search.setRead(id: id, true)
        errorWords = nil
        onChange()
        return true
    }

    /// Opens one mail in place on its words: the shape grows, Whoosh stays behind the app in front. `reply`: the quick reply starts at once
    /// (a Reply button); else Reply (R) starts it from the open mail (owner's picks of 14 Nov 2026: opening and replying are two states).
    @discardableResult
    public func open(id: String, _ m: AppModel, reply: Bool = false) -> Bool {
        guard let item = items.first(where: { $0.id == id }) else { return false }
        if case .open(let o) = stage {
            if o != id { endOpenReply() }   // another mail was open: its reply and its full-mail view end with it
        } else {
            openedFromList = stage == .list   // Escape goes back to the list this mail was opened from; a mail opened from an alert closes the notch
        }
        sentShown = false
        stage = .open(item.id)
        if reply { openReply(for: item, m) }
        onChange()
        return true
    }

    /// Reply on the open mail: the quick reply starts for THAT mail; the mail folds to one line above it.
    @discardableResult
    public func reply(_ m: AppModel) -> Bool {
        guard let item = openItem else { return false }
        snoozeId = nil
        let opened = openReply(for: item, m)
        onChange()
        return opened
    }

    /// The open mail as the notch shows it by default: its words, its folded parts, whether it has a page of its own. A body the store does not
    /// hold yet reads as the arrival's preview line and is asked for again at the next draw.
    func words(of item: NotchItem, _ m: AppModel) -> NotchWords {
        if let w = wordsOf[item.id] { return w }
        guard let body = m.core.body(messageId: item.id) else {
            return NotchWords(paragraphs: item.preview.isEmpty ? [] : [item.preview], folds: [], hasPage: false, pictures: 0, trackers: 0)
        }
        let w = notchWords(body: body)
        if wordsOf.count >= 16 { wordsOf.removeAll() }
        wordsOf[item.id] = w
        return w
    }

    /// The mail as one line while its reply is written: its first paragraph of four words or more (a greeting alone says nothing), else its
    /// first paragraph, else the arrival's preview line.
    func firstWords(of item: NotchItem, _ m: AppModel) -> String {
        let paragraphs = words(of: item, m).paragraphs
        return paragraphs.first { $0.split(separator: " ").count >= 4 } ?? paragraphs.first ?? item.preview
    }

    /// Full mail (O) on the open mail: the sender's page takes the place of the words, and back. Only a mail with a page of its own has the button.
    @discardableResult
    public func toggleFullMail(_ m: AppModel) -> Bool {
        guard let item = openItem, words(of: item, m).hasPage else { return false }
        fullMailId = fullMailId == item.id ? nil : item.id
        onChange()
        return true
    }

    /// Snooze (H) on the open mail: the three choices come out under the buttons, and go back.
    @discardableResult
    public func toggleSnooze() -> Bool {
        guard let item = openItem else { return false }
        snoozeId = snoozeId == item.id ? nil : item.id
        onChange()
        return true
    }

    /// One snooze choice: the mail leaves the inbox until then (the engine's Snooze), and the notch goes back to its list.
    @discardableResult
    public func snooze(_ when: SnoozeWhen, _ m: AppModel) -> Bool {
        guard let item = openItem, let index = items.firstIndex(where: { $0.id == item.id }) else { return false }
        guard m.perform(.snooze(messageIds: [item.id], when: when)) else { return fail(m) }
        items.remove(at: index)
        search.drop(id: item.id)
        leaveMail()
        errorWords = nil
        onChange()
        return true
    }

    @discardableResult
    public func markUnread(id: String, _ m: AppModel) -> Bool {
        guard let index = items.firstIndex(where: { $0.id == id }), items[index].read else { return false }
        guard m.perform(.markRead(messageIds: [id], read: false)) else { return fail(m) }
        items[index].read = false
        search.setRead(id: id, false)
        errorWords = nil
        onChange()
        return true
    }

    /// Open in Whoosh (⌘O): the main window comes up on the mail the notch is on (the open mail, the row under the keyboard, the alert's mail);
    /// the notch shrinks back. A reply started in the notch goes with it: the main window opens on that mail with the same reply, its words
    /// kept (the reply is one, the notch only stops drawing it). The one act that brings Whoosh to the front, by the person's own act.
    @discardableResult
    public func openInWhoosh(_ m: AppModel) -> Bool {
        let replying = replyTargetsOpen
        if let item = keyTarget ?? (stage == .list && search.active ? shownRows.first : nil) ?? openItem ?? dropItem {
            let thread = item.threadId.isEmpty ? item.id : item.threadId
            _ = try? m.core.run(command: .openThread(threadId: thread))
            m.focus(messageId: item.id)
            handedOver = item.id
        }
        #if os(macOS)
        guard WindowBridge.open(.macMain, in: m) else { return false }
        NSApp.activate(ignoringOtherApps: true)   // a no-op under the hidden rig (HiddenRig refuses activation)
        #endif
        if replying {
            replyStartedFor = nil                 // the reply is the main window's now: closing the notch does not end it
            m.surface = QuickReplyIds.surface      // the reader, where the reply is drawn under its mail
        }
        stage = .rest
        notice = nil
        sentShown = false
        leaveSearch()
        onChange()
        return true
    }

    @discardableResult
    public func close() -> Bool {
        guard case .open = stage else { return false }
        leaveMail(listHas: !listed.isEmpty)
        onChange()
        return true
    }

    /// The open mail leaves the open state (the back chevron, an act that removed it): its reply ends with it, and the notch goes back to the
    /// list (the search's answer it was opened from, or a list that still has mail), else it closes and gives the keyboard back.
    func leaveMail(listHas: Bool? = nil) {
        endOpenReply()
        sentShown = false
        if (search.active && openedFromList) || (listHas ?? !arrived.isEmpty) {
            stage = .list
            if let id = rowFocusId, !shownRows.contains(where: { $0.id == id }) { rowFocusId = nil }
            searchFocused = keyboardWanted && rowFocusId == nil
        } else {
            stage = .rest
            leaveSearch()
        }
    }

    /// A file dropped on the notch starts a new message with it attached. The drop is the person's own act, so the
    /// composer comes to the front (an arrival never does: its ask is only ever made here and by Open in Whoosh).
    @discardableResult
    public func dropFile(_ url: URL) -> Bool {
        droppedFile = url.lastPathComponent
        onChange()
        let taken = fileSink?(url) ?? false
        #if os(macOS)
        if taken { NSApp.activate(ignoringOtherApps: true) }   // a no-op under the hidden rig (HiddenRig refuses and logs the ask)
        #endif
        return taken
    }

    /// The quick send's window ran out with the message still going: the bar says "Sent".
    func sendExpired(_ m: AppModel) {
        QuickReplyModel.shared.expire(m)
        sentShown = true
        onChange()
    }

    func fail(_ m: AppModel) -> Bool {
        errorWords = m.lastFailure.map { NotchFailure.words(account: $0.accountName, reason: $0.reasonWords) }
        onChange()
        return false
    }
}
