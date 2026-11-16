import Foundation

/// Every control's write: one delta 20 command each (docs/lanes/replies-contract.md "Every name, and the pane control it serves").
/// A setter takes the value the control shows next, so a click door and the real control call the same line.
extension RepliesStore {
    // MARK: the two uses (Brouillon automatique, Sur demande)

    func setEnabled(_ kind: ReplyUseKind, _ on: Bool) { run(.setReplyUseEnabled(kind: kind, enabled: on)) }
    func setTone(_ kind: ReplyUseKind, _ id: String) { run(.setReplyUseTone(kind: kind, toneId: id)) }
    func setMatchThread(_ kind: ReplyUseKind, _ on: Bool) { run(.setReplyUseMatchThread(kind: kind, on: on)) }
    func setOption(_ option: OnDemandOption, _ on: Bool) { run(.setOnDemandOption(option: option, enabled: on)) }

    /// « Essayer sur un message »: the trial runs on the message in focus, else the newest one; nothing is stored, nothing is sent.
    func tryOnMessage(_ kind: ReplyUseKind) {
        let app = AppModel.shared
        guard let id = app.focusedMessageId ?? app.rows.first?.messageId else { error = RepliesWords.noMessage; return }
        guard let runId = run(.tryReply(messageId: id, kind: kind)), !runId.isEmpty else { return }
        trial = RepliesRead.trial(RepliesRead.object(core.state(path: "reply/try/\(runId)")))
        popover = .trial
    }

    // MARK: Quels messages

    func setScope(_ scope: ReplyLabelScope) { run(.setReplyFilterScope(scope: scope)) }
    /// A label tick: « Toutes » clears the list; ticking the last label off leaves « no label ticked », which admits nothing (contract trap).
    func toggleLabel(_ id: String) {
        guard let f = filter else { return }
        var ids: [String] = []
        if case .labels(let now) = f.scope { ids = now }
        if let i = ids.firstIndex(of: id) { ids.remove(at: i) } else { ids.append(id) }
        setScope(.labels(labelIds: ids))
    }
    func setCard(_ card: FlowCard, _ on: Bool) { run(.setReplyFilterCard(card: card, enabled: on)) }

    func rule(_ card: FlowCard) -> RuleSet? { card == .exit ? filter?.exitRule : filter?.entryRule }
    func setRule(_ card: FlowCard, _ rule: RuleSet) {
        run(card == .exit ? .setReplyFilterExitRule(rule: rule) : .setReplyFilterEntryRule(rule: rule))
    }
    func setMode(_ card: FlowCard, _ mode: MatchMode) {
        guard var r = rule(card) else { return }
        r.matchMode = mode
        setRule(card, r)
    }
    func setFact(_ card: FlowCard, _ fact: MessageFact, _ state: FactState) {
        guard var r = rule(card) else { return }
        r.conditions = RepliesFacts.set(r.conditions, fact, state)
        setRule(card, r)
    }
    /// The « Expéditeur » field of the entry card: one From row, replaced (blank removes it).
    func setSender(_ words: String) {
        guard var r = filter?.entryRule else { return }
        r.conditions.removeAll { if case .from = $0 { return true } else { return false } }
        let w = words.trimmingCharacters(in: .whitespacesAndNewlines)
        if !w.isEmpty { r.conditions.append(.from(words: w)) }
        setRule(.entry, r)
    }
    func toggleRead(_ input: RepliesReadInput) {
        guard var reads = filter?.aiReads else { return }
        switch input {
        case .subject: reads.subject.toggle()
        case .body: reads.body.toggle()
        case .rest: reads.restOfThread.toggle()
        case .attachments: reads.attachments.toggle()
        }
        run(.setReplyFilterAiReads(reads: reads))
    }

    // MARK: Recherche

    func setRetrieval(_ change: RetrievalChange) { run(.setRetrievalSetting(change: change)) }
    func toggleField(_ field: RepliesField) {
        guard var f = retrieval?.attachedFields else { return }
        switch field { case .date: f.date.toggle(); case .sender: f.sender.toggle(); case .folder: f.folder.toggle(); case .subject: f.subject.toggle() }
        setRetrieval(.attachedFields(fields: f))
    }
}

/// What « L'IA lit » can tick.
enum RepliesReadInput: String, CaseIterable, Sendable { case subject, body, rest, attachments }
/// What « Joint à chaque extrait » can tick.
enum RepliesField: String, CaseIterable, Sendable { case date, sender, folder, subject }
