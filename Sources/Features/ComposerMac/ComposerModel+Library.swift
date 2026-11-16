import Foundation

/// Templates, snippets, Instant Reply, the Bcc row and the footer switch: the par-composermac items.
extension ComposerModel {
    // MARK: templates (subject, text and files)

    @discardableResult
    public func toggleTemplates() -> Bool {
        guard isOpen, !isCounting else { return false }
        let open = !templatesOpen; closePanels(); templatesOpen = open
        return true
    }

    /// A template fills the subject and the text and attaches its files that still exist.
    @discardableResult
    public func applyTemplate(_ index: Int) -> Bool {
        guard isOpen, library.templates.indices.contains(index) else { return false }
        let t = library.templates[index]
        if !t.subject.isEmpty { subject = t.subject }
        session.setPlain(t.body)
        clearFiles()
        for f in t.files { _ = attach(URL(fileURLWithPath: f)) }
        closePanels(); line = ""
        return true
    }

    /// Keeps the current subject, text and files as a template.
    @discardableResult
    public func saveTemplate() -> Bool {
        guard isOpen, !bodyText.isEmpty else { line = ComposerWords.nothingToSave; return true }
        library.addTemplate(subject: subject, body: bodyText, files: attachments.compactMap { $0.url?.path })
        line = ComposerWords.templateSaved
        return true
    }

    // MARK: snippets (⌘;)

    @discardableResult
    public func toggleSnippets() -> Bool {
        guard isOpen, !isCounting else { return false }
        let open = !snippetsOpen; closePanels(); snippetsOpen = open
        return true
    }

    /// A snippet goes in at the caret.
    @discardableResult
    public func insertSnippet(_ index: Int) -> Bool {
        guard isOpen, library.snippets.indices.contains(index) else { return false }
        insertAtCaret(library.snippets[index].text)
        closePanels()
        return true
    }

    /// Keeps the current text as a snippet.
    @discardableResult
    public func saveSnippet() -> Bool {
        guard isOpen, !bodyText.isEmpty else { line = ComposerWords.nothingToSave; return true }
        library.addSnippet(text: bodyText)
        line = ComposerWords.snippetSaved
        return true
    }

    // MARK: Instant Reply

    /// Asks the writer for whole replies to the message being answered; the panel lists them, a pick fills the text.
    @discardableResult
    public func toggleInstantReply() -> Bool {
        guard isOpen, !isCounting else { return false }
        if instantOpen { instantOpen = false; return true }
        closePanels(); instantOpen = true; instantOptions = []; aiBusy = true; line = ""
        let (w, subj, text) = (writer, subject, quoted.map(\.text).joined(separator: "\n"))
        Task { @MainActor in
            defer { self.aiBusy = false }
            do { self.instantOptions = try await w.instantReplies(to: text, subject: subj) }
            catch { self.line = Self.writerWords(error); self.instantOpen = false }
        }
        return true
    }

    @discardableResult
    public func pickInstantReply(_ index: Int) -> Bool {
        guard instantOptions.indices.contains(index) else { return false }
        session.setPlain(instantOptions[index])
        closePanels(); aiText = nil
        return true
    }

    // MARK: Cc and Bcc keys

    @discardableResult
    public func showRecipientRow(_ field: RecipientField) -> Bool {
        guard isOpen, !isCounting else { return false }
        if field == .bcc { showBcc = true } else { showCc = true }
        return true
    }

    @discardableResult
    public func toggleFooter() -> Bool { library.footerOn.toggle(); return true }
}

extension ComposerModel {
    /// The name the core's contact card gives an address, nil for a stranger.
    func contactName(_ address: String) -> String? { ComposerContext.contactName(address, core: CoreHost.shared.core) }
}

extension ComposerModel {
    /// The chevron beside Send opens its menu and closes every other panel.
    func closePanelsExcept(more: Bool) { let open = !moreSendOpen; closePanels(); moreSendOpen = more && open }
}
