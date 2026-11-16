import Foundation

/// The acts of the AI reply (QuickReplyAIState): ⌘J, the instruction's Return, the change's Return, Escape or the
/// discard control, the owner's first edit, and the events that bring the draft. Each flips state on the key frame and
/// sends ONE command; the draft's text only ever arrives by event, so a key never waits for the model. Nothing sends mail.
extension QuickReplyModel {
    /// ⌘J (and the AI pill): open the reply under the open thread if needed, then either show the instruction field
    /// (Sur demande > "Donner une consigne avant" on) or write at once. With the field up, ⌘J writes, as Return does.
    /// The owner's own words in the field become the instruction (or wait in `held`): Escape brings them back.
    public func aiKey(_ m: AppModel) -> Bool {
        settle(m)
        if !isOpen { guard ShortcutEngine.threadIsOpen(m), open(m) else { return false } }
        guard outboxId == nil, phase == .editing || isFailed else { return false }
        ai.start(self, core: m.core)
        switch ai.phase {
        case .writing: return true                                     // already on its way: the key is taken, nothing doubles
        case .asking: return aiWrite(m, instruction: ai.instruction)
        default: break
        }
        ai.readOptions(m.core)
        let own = ai.draft == nil ? draft.trimmingCharacters(in: .whitespacesAndNewlines) : ""
        if !own.isEmpty { ai.held = draft; draft = "" }
        emojiOpen = false; fromOpen = false
        if ai.options.instructionBefore {
            ai.instruction = own.isEmpty ? (ai.draft?.instruction ?? "") : own
            ai.phase = .asking
            ai.focus = .instruction
            return true
        }
        return aiWrite(m, instruction: nil)
    }

    /// The command goes out with the field's state already "writing"; the fixture core answers inside the same turn,
    /// a real core later, by event. A refusal shows in words under the field, and ⌘J tries again.
    func aiWrite(_ m: AppModel, instruction: String?) -> Bool {
        guard let tid = threadId else { return false }
        let words = instruction?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        ai.replaceable = draft
        ai.phase = .writing(rewrite: false)
        ai.focus = .reply
        ai.writes += 1
        do {
            _ = try aiRun(.draftReplyOnDemand(threadId: tid, instruction: words.isEmpty ? nil : words), m)
            ai.instruction = ""
        } catch {
            ai.phase = .failed(words: Self.aiWords(error))
            if draft.isEmpty { draft = ai.held; ai.held = "" }      // the refusal gives the owner's words back, nothing is lost
        }
        return true
    }

    /// Return in the change field: the draft is rewritten in place (same draft, the next revision), by event.
    public func aiRequestChange(_ m: AppModel) -> Bool {
        guard let d = ai.draft, ai.options.changeAfter else { return false }
        let text = ai.change.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return true }                       // an empty change asks nothing; the key stays here
        ai.replaceable = draft
        ai.phase = .writing(rewrite: true)
        ai.writes += 1
        do {
            _ = try aiRun(.requestReplyChange(draftId: d.id, text: text), m)
        } catch {
            ai.phase = .failed(words: Self.aiWords(error))
        }
        return true
    }

    /// Escape or the × beside the mark: the AI's draft goes (`DiscardReplyDraft`) and the owner's words before ⌘J come back.
    public func aiDiscard(_ m: AppModel) -> Bool {
        guard let d = ai.draft else { return false }
        let held = ai.held
        ai.clear()
        draft = held
        do { _ = try m.core.run(command: .discardReplyDraft(draftId: d.id)) } catch { ai.phase = .failed(words: Self.aiWords(error)) }
        return true
    }

    /// Escape, in order: the instruction field closes (the words come back), else the AI draft is discarded, else a
    /// write in flight stops being waited for (its draft stays a proposal in the core). False: the slab's own Escape.
    func aiEscape(_ m: AppModel) -> Bool {
        if case .asking = ai.phase {
            if ai.draft != nil { ai.phase = .ready; ai.instruction = ""; ai.focus = .reply; return true }   // ⌘J again over a draft: it stays
            let held = ai.held
            ai.clear()
            if draft.isEmpty { draft = held }
            return true
        }
        if ai.draft != nil { return aiDiscard(m) }
        if ai.isWriting { let held = ai.held; ai.clear(); if draft.isEmpty { draft = held }; return true }
        return false
    }

    /// The owner changed the AI's words (typing, a paste, an emoji): from now on it is their draft (`AcceptReplyDraft`
    /// makes the ordinary composer draft the field then keeps). The mark and the change field go on the same frame.
    func aiTextChanged(_ m: AppModel) {
        guard let d = ai.draft, draft != d.text else { return }
        if case .asking = ai.phase { return }
        if case .writing(rewrite: false) = ai.phase { return }        // a new draft replaces this one: nothing to take over
        aiAccept(m)
    }

    /// The AI draft becomes an ordinary draft: by an edit, by Send, by the pop-out.
    func aiAccept(_ m: AppModel) {
        guard let d = ai.draft else { return }
        ai.clear()
        do {
            if let created = try m.core.run(command: .acceptReplyDraft(draftId: d.id)).createdId {
                openDraftId = created
                draftsRevision += 1
            }
        } catch {
            ai.phase = .failed(words: Self.aiWords(error))
        }
    }

    /// Before Send: a shown AI draft is accepted first (the send then deletes that ordinary draft as for any reply);
    /// the instruction field closes. Nothing is ever sent by the AI path itself.
    func aiBeforeSend(_ m: AppModel) {
        if ai.draft != nil { aiAccept(m) } else if case .asking = ai.phase { ai.clear() }
    }

    /// Before the field's words are kept as the thread's draft (Esc, a move to another thread): an untouched AI draft is
    /// not copied into an ordinary draft (it stays the core's proposal and shows again on open); the owner's held words are.
    func aiReleaseForKeep() {
        let untouched = ai.draft.map { $0.text == draft } ?? false
        let pending = (ai.phase == .asking || ai.isWriting) && (draft.isEmpty || draft == ai.replaceable)
        if untouched || pending { draft = ai.held }
        ai.clear()
    }

    /// The thread opens: its live AUTO or on-demand draft shows the same way, when the field has nothing else.
    func aiOpened(_ m: AppModel) {
        ai.start(self, core: m.core)
        ai.clear()
        guard let tid = threadId, draft.isEmpty, openDraftId == nil, phase == .editing,
              let d = QuickReplyAIState.liveDraft(threadId: tid, core: m.core), QuickReplyAIState.isProposed(d) else { return }
        ai.readOptions(m.core)
        draft = d.text
        ai.show(d)
    }

    /// One reply event of the thread stream. A draft lands when this field waits for it, when it replaces the shown one
    /// (a change), or when an auto draft arrives while the field is empty; the owner's words typed since always win.
    func aiReceive(_ e: QuickReplyAIEvent) {
        let core = AppModel.shared.core
        switch e {
        case .ready(let t, let id):
            guard t == threadId, ai.draft?.id != id, let d = QuickReplyAIState.liveDraft(threadId: t, core: core), d.id == id else { return }
            aiLand(d, core)
        case .changed(let t, let d):
            guard t == threadId else { return }
            if QuickReplyAIState.isProposed(d) {
                let auto = d.madeBy == .auto && ai.phase == .idle && draft.isEmpty && openDraftId == nil && isOpen
                if ai.isWriting || ai.draft?.id == d.id || auto { aiLand(d, core) }
            } else if ai.draft?.id == d.id {
                ai.draft = nil                                        // sealed elsewhere: the mark goes, the words stay
                if !ai.isWriting { ai.phase = .idle }
            }
        }
    }

    private func aiLand(_ d: ReplyDraft, _ core: WhooshCore) {
        guard ai.draft != d else { return }
        guard draft.isEmpty || draft == ai.replaceable || draft == ai.draft?.text else {
            ai.phase = .idle; ai.replaceable = nil                   // typed while it was writing: their words win
            return
        }
        draft = d.text
        ai.replaceable = nil
        ai.readOptions(core)
        ai.show(d)
    }

    /// One AI command. Under the fixture core an armed `fixture fail-next <op> <words…>` refuses it once in those words
    /// (op `draft-reply-on-demand`, `request-reply-change`), as the slab's own send does: the failed state is drivable.
    private func aiRun(_ command: Command, _ m: AppModel) throws -> CommandOutcome {
        if m.host.kind == .fixture, let armed = FixtureFaults.take(FixtureFaults.op(of: command)) {
            throw CoreError.Invalid(field: "reply", reasonWords: armed)
        }
        return try m.core.run(command: command)
    }

    static func aiWords(_ error: Error) -> String {
        switch error {
        case CoreError.Action(let failure): return failure.reasonWords
        case CoreError.Invalid(_, let words): return words
        default: return QuickReplyCopy.aiFailed
        }
    }
}
