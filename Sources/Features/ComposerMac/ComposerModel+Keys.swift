import Foundation
#if os(macOS)
import AppKit
#endif

/// The field keys of the Superhuman composer: To, Cc, Bcc, From, Subject, Message, Attach, Discard, Move to Bcc. A key
/// puts the focus ring on its field on the key's own frame; the view moves the real cursor after (ComposerView).
extension ComposerModel {
    // MARK: fields

    @discardableResult
    public func focus(_ f: ComposerField) -> Bool {
        guard isOpen, !isCounting else { return false }
        if f == .cc { showCc = true }
        if f == .bcc { showBcc = true }
        closePanels()
        ringSlide = Self.ringSlides(from: ringField, to: f, visible: visibleFields)
        ringField = f
        if f == .from { fromOpen = true; listIndex = max(0, accounts.firstIndex { $0.id == fromId } ?? 0) }
        focusSerial += 1
        keyFocusAt = ProcessInfo.processInfo.systemUptime
        focusRequest = ComposerFocusRequest(field: f, serial: focusSerial)
        return true
    }

    /// The field the cursor is in, in words, for the control socket: "" when the cursor is in none of them.
    var focusedWord: String { ringField?.rawValue ?? "" }

    /// The text cursor leaves the text so the keys of a list (Up, Down, Return) reach the list and not the text.
    func resignText() {
        #if os(macOS)
        WindowBridge.window(for: ComposerContext.slot)?.makeFirstResponder(nil)
        #endif
    }

    @discardableResult
    func closeFromList() -> Bool { fromOpen = false; return true }

    // MARK: move the contacts to Bcc

    @discardableResult
    public func moveContactsToBcc() -> Bool {
        guard isOpen, !isCounting else { return false }
        if !toInput.isEmpty { _ = commitRecipient() }
        let moving = to + cc
        guard !moving.isEmpty else { line = ComposerWords.noContactsToMove; return true }
        bcc += moving.filter { !bcc.contains($0) }
        to = []; cc = []; showBcc = true; line = ""
        return true
    }

    // MARK: discard

    /// The key and the trash button: a draft with something in it asks first; an empty one goes at once.
    @discardableResult
    public func requestDiscard(_ m: AppModel) -> Bool {
        guard isOpen, !isCounting else { return false }
        guard hasDraft || !attachments.isEmpty else { return confirmDiscard(m) }
        closePanels(); discardAsk = true; resignText()
        return true
    }

    @discardableResult
    public func confirmDiscard(_ m: AppModel) -> Bool {
        guard isOpen, !isCounting else { return false }
        discardAsk = false
        #if os(macOS)
        AttachmentPlaces.removePasted(fileTiles.compactMap(\.url) + pendingAdds)   // the draft is thrown away: its pasted pictures (tiles and in the text) with it
        #endif
        dropCoreDraft()
        discardDraft()
        to = []; cc = []; bcc = []; toInput = ""; ccInput = ""; bccInput = ""; subject = ""; aiText = nil; ringField = nil
        return close(m)
    }

    // MARK: attach

    @discardableResult
    public func toggleAttachPanel() -> Bool {
        guard isOpen, !isCounting else { return false }
        let open = !attachOpen; closePanels(); attachOpen = open; listIndex = 0
        if open { resignText() }
        return true
    }

    /// Row 0 is the file chooser; the rows after it are the files attached lately.
    @discardableResult
    public func pickAttachRow(_ i: Int) -> Bool {
        guard attachOpen else { return false }
        if i == 0 { chooseFiles(); return true }
        let recent = library.recentFiles
        guard recent.indices.contains(i - 1) else { return false }
        closePanels()
        return attach(URL(fileURLWithPath: recent[i - 1]))
    }

    /// The system's open panel as a sheet of the composer's window: it never blocks the app (a driven check can go on).
    func chooseFiles() {
        closePanels()
        #if os(macOS)
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = true
        let done: (NSApplication.ModalResponse) -> Void = { [weak panel] r in
            guard r == .OK, let urls = panel?.urls else { return }
            Task { @MainActor in urls.forEach { _ = self.attach($0) } }
        }
        if let w = WindowBridge.window(for: ComposerContext.slot) { panel.beginSheetModal(for: w, completionHandler: done) } else { panel.begin(completionHandler: done) }
        #endif
    }

    // MARK: the arrow keys and Return inside a list

    /// Up and Down (Left and Right in the colour row) move the highlight of the list that is open. False when none is.
    @discardableResult
    func listKey(_ delta: Int) -> Bool {
        let count: Int
        if fromOpen { count = accounts.count }
        else if attachOpen { count = 1 + library.recentFiles.count }
        else if colorOpen { count = ComposerColor.all.count }
        else { return false }
        listIndex = min(max(listIndex + delta, 0), max(count - 1, 0))
        return true
    }

    /// Return picks the highlighted row of the list that is open, or answers the discard question.
    @discardableResult
    func listReturn(_ m: AppModel) -> Bool {
        if discardAsk { return confirmDiscard(m) }
        if fromOpen, accounts.indices.contains(listIndex) { fromId = accounts[listIndex].id; return focus(.body) }
        if attachOpen { return pickAttachRow(listIndex) }
        if colorOpen { return pickColor(ComposerColor.all[min(listIndex, ComposerColor.all.count - 1)].name) }
        return false
    }
}

extension ComposerModel {
    /// The cursor went into a field by a click: the ring follows. A cursor that left every field by a click clears the ring
    /// of a text field; the ring a key put on From or the message stays.
    func focusFollows(_ f: ComposerField?) {
        guard let f else {
            if let r = ringField, r != .from, r != .body { ringField = nil }
            return
        }
        guard f != ringField else { return }
        // The field a key just left reports its focus once more (the window may refuse to let go at once, as in a window that is
        // not key): inside one popover duration after a key's request that report is not the owner's click, so the ring stays.
        let sinceKey = (ProcessInfo.processInfo.systemUptime - keyFocusAt) * 1000
        if sinceKey < Double(WhooshConstants.POPOVER_MS), let asked = focusRequest?.field, asked != f { return }
        ringSlide = Self.ringSlides(from: ringField, to: f, visible: visibleFields)
        ringField = f
    }
}
