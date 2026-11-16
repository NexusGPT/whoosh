import Foundation

/// What each long text is in the core and how it saves: the prompt (each save a version), the retrieval prompt, my voice, the AI card's
/// description, a fixed note, a new note. One door for every save: a refusal leaves the draft as it is and says why.
extension RepliesStore {
    /// The editor of a text, made once and kept (its draft survives leaving the page).
    func editor(_ k: RepliesEditorKey) -> RepliesEditorState {
        if let e = editors[k] { return e }
        let e = RepliesEditorState(key: k)
        e.saved = savedText(k)
        editors[k] = e
        return e
    }

    /// The stored form the core holds for a text.
    func savedText(_ k: RepliesEditorKey) -> String {
        switch k {
        case .prompt: return prompt?.text ?? ""
        case .retrievalPrompt: return retrieval?.retrievalPrompt ?? ""
        case .voiceText: return voice?.toneText ?? ""
        case .aiDescription: return RepliesWords.aiDescriptionShown(filter?.aiDescription ?? "")   // a default in the app's language; a save writes what is shown
        case .newNote: return ""
        default: return notes.first { k.noteId == $0.id }?.text ?? ""
        }
    }

    /// ⌘S, ⌘↩ or « Enregistrer »: one command. The prompt's save is a new version (the Versions row gains it).
    @discardableResult
    func save(_ k: RepliesEditorKey) -> Bool {
        let e = editor(k)
        guard e.dirty || k == .newNote, let text = e.draft else { return false }
        let landed: String?
        switch k {
        case .prompt: landed = run(.saveReplyPromptVersion(promptId: prompt?.promptId ?? "replies", text: text))
        case .retrievalPrompt: landed = run(.setRetrievalSetting(change: .prompt(text: text)))
        case .voiceText: landed = run(.setVoiceText(text: text))
        case .aiDescription: landed = run(.setReplyFilterAiDescription(description: text))
        case .newNote:
            // no title typed: the note's first words name it
            let title = newNoteTitle.trimmingCharacters(in: .whitespaces).isEmpty ? String(text.prefix(40)) : newNoteTitle
            landed = run(.saveFixedNote(note: FixedNote(id: "", title: title, text: text)))
            if landed != nil { newNoteTitle = "" }
        default:
            guard let id = k.noteId, let n = notes.first(where: { $0.id == id }) else { return false }
            landed = run(.saveFixedNote(note: FixedNote(id: id, title: n.title, text: text)))
        }
        guard landed != nil else { return false }
        e.saved = k == .newNote ? "" : savedText(k)
        e.draft = nil
        e.confirmRevert = false
        if k == .newNote { e.revision += 1 }   // the written note left for the list: the field empties; any other text stays as typed
        return true
    }

    /// A variable from the Variables list: its pill at the prompt editor's caret when the editor is on screen, else at the end of the draft.
    func placeVariable(_ v: ReplyPromptVariable) {
        popover = nil
        #if os(macOS)
        if let c = RepliesEditorViews.live[.prompt]?.owner { c.insert(v); return }
        #endif
        let e = editor(.prompt)
        e.draft = e.text + (e.text.hasSuffix(" ") || e.text.isEmpty ? "" : " ") + promptPlaceholder(id: v.id) + " "
        e.revision += 1
    }

    /// « Voir ce que le modèle reçoit »: the text being edited when it differs from the saved one (rendered here with the contract's own
    /// renderer and this Mac's values), else the saved version as the core renders it. The popover says which.
    func openPreview() {
        let e = editor(.prompt)
        if e.dirty, let draft = e.draft {
            preview = renderReplyPrompt(text: draft, values: previewValues())
            previewOfDraft = true
        } else {
            preview = RepliesRead.string(core.state(path: "reply/prompt/preview"))
            previewOfDraft = false
        }
        popover = .preview
    }

    /// The values a draft is shown with: the newest message of the list, my voice, the fixed notes (the core's preview path takes no text).
    private func previewValues() -> [PromptValue] {
        let m = AppModel.shared.rows.first
        let message = m.map { "\($0.senderName) : \($0.subject)\n\($0.preview)" } ?? ""
        return [PromptValue(id: "message", value: message), PromptValue(id: "thread", value: m?.preview ?? ""), PromptValue(id: "instruction", value: ""),
                PromptValue(id: "excerpts", value: ""), PromptValue(id: "calendar", value: ""), PromptValue(id: "tone", value: voice?.toneText ?? ""),
                PromptValue(id: "notes", value: notes.map { "\($0.title) : \($0.text)" }.joined(separator: "\n"))]
    }
}
