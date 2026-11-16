import Foundation

/// One message as the fixture's fake provider sees it: what a search tool would hand the model.
struct FixtureMail {
    let id: String, accountId: String, sender: String, subject: String, preview: String, receivedAtMs: Int64
    let text: String          // sender, address, subject, preview, attachment names and body, lower-cased
}

/// The Ask side of the fixture core (delta 7). It mirrors the rules of core/crates/agents (AgentRuntime), which the
/// Swift fixture cannot call: consent once per off-device provider before any mail is read, tools only for the actions
/// an agent is allowed, citations built only from mail the run was shown, a proposal that changes nothing until the
/// owner approves it, a step ceiling of AGENT_MAX_STEPS. The provider is fake and deterministic: a word match over
/// the fixture rows, with a small synonym table so "suppliers" finds Linea Supply and LHG Procurement.
/// Called with FixtureCore's lock held.
final class FixtureAgentBook {
    struct Stored { var proposal: AgentProposal; let ids: [String]; let label: String }
    struct Run { var events: [CoreEvent] = []; var conversationId = "" }

    static let nothingFound = "I found nothing in your mail about that."
    private static let stop: Set<String> = ["what", "this", "that", "did", "with", "from", "have", "your", "mail", "mails", "about", "when", "which", "were", "does", "these", "them", "those"]
    private static let synonyms: [String: [String]] = ["suppliers": ["supplier", "supply", "procurement", "vendor"], "supplier": ["supplier", "supply", "procurement", "vendor"], "quote": ["quote", "quotation"], "quotes": ["quote", "quotation"]]

    var providers: [AiProvider] = [
        AiProvider(id: "anthropic", name: "Anthropic", models: ["claude-sonnet-5-5", "claude-haiku-5-5"], connected: false, keychain: nil),
        AiProvider(id: "openai", name: "OpenAI", models: [], connected: false, keychain: nil),
        AiProvider(id: "gemini", name: "Gemini", models: [], connected: false, keychain: nil),           // hid-ai: the real core's table holds both
        AiProvider(id: "openrouter", name: "OpenRouter", models: [], connected: false, keychain: nil),
        AiProvider(id: "local", name: "On this Mac", models: ["on-device"], connected: true, keychain: nil),
    ]
    var agents: [AgentConfig] = [
        AgentConfig(id: "inbox", name: "Ask my inbox", purpose: "Answers questions about your mail and proposes labels", providerId: "anthropic", model: "claude-sonnet-5-5", instructions: "", allowed: [.readMail, .search, .label], builtIn: true),
        AgentConfig(id: "writer", name: "Writer", purpose: "Drafts replies in your voice", providerId: "anthropic", model: "claude-sonnet-5-5", instructions: "", allowed: [.readMail, .search, .draft], builtIn: true),
    ]
    private(set) var conversations: [AgentConversation] = []
    private(set) var answers: [String: AskAnswer] = [:]            // conversation id -> its latest answer
    private(set) var stored: [Stored] = []
    private(set) var lastConversationId: String?
    private(set) var lastSteps = 0
    private(set) var lastChunks = 0
    var lastQuoted: String?
    var awaitingConsent = false
    var credentialStored: Set<String> = []
    /// A pasted key's two-step answer per provider (FixtureProviderKeys.swift); replaced with the book on `fixture reset`.
    var keyChecks: [String: FixtureKeyCheck] = [:]
    /// `fixture provider-fails <reason>`: every off-device ask fails in the provider's words until cleared.
    var providerFails: String?
    /// `fixture forward-refuse on`: the forward command is refused in these words until `off` (the book is replaced on `fixture reset`, so reset puts it off).
    var forwardRefusal: String?
    static let forwardRefusedWords = "The server refused to forward this message."
    private var seq = 0

    func answerFor(_ conversationId: String) -> AskAnswer? { answers[conversationId] }
    func provider(_ id: String) -> AiProvider? { providers.first { $0.id == id } }

    /// `fixture providers none|default`: stage a core with no provider connected (the on-device "On this Mac" too, with every key and check forgotten),
    /// or put the launch state back (only "On this Mac" connected). `fixture reset` builds a new book, so it restores the default by itself.
    func stageProviders(none: Bool) {
        keyChecks = [:]
        if none {
            credentialStored.subtract(["anthropic", "openai", "gemini", "openrouter"])
            for i in providers.indices { providers[i].connected = false; providers[i].keychain = nil }
        } else if let i = providers.firstIndex(where: { $0.id == "local" }) {
            providers[i].connected = true
        }
    }
    private func onDevice(_ id: String) -> Bool { id == "local" }

    func giveConsent(providerId: String, nowMs: Int64) throws {
        guard let i = providers.firstIndex(where: { $0.id == providerId }) else { throw CoreError.NotFound(kind: "provider", id: providerId) }
        providers[i].mailConsentAtMs = nowMs
    }

    private func failureWords(_ reason: String) -> String {
        reason == "quota" ? "The provider says your quota is used up. Switch agent or add credit." : reason
    }

    /// Terms the fake provider searches for: words of four letters or more, minus filler, plus synonyms.
    static func terms(_ question: String) -> [String] {
        let words = question.lowercased().split(whereSeparator: { !$0.isLetter && !$0.isNumber }).map(String.init).filter { $0.count >= 4 && !stop.contains($0) }
        var out: [String] = []
        for w in words { for t in synonyms[w] ?? [w] where !out.contains(t) { out.append(t) } }
        return out
    }

    func ask(agentId: String?, question: String, mail: [FixtureMail], nowMs: Int64) throws -> Run {
        let aid = agentId ?? "inbox"
        guard let agent = agents.first(where: { $0.id == aid }) else { throw CoreError.NotFound(kind: "agent", id: aid) }
        guard let prov = provider(agent.providerId) else { throw CoreError.NotFound(kind: "provider", id: agent.providerId) }
        let q = question.trimmingCharacters(in: .whitespacesAndNewlines)
        if q.isEmpty { throw CoreError.Invalid(field: "question", reasonWords: "ask in words") }
        let convId = conversations.first { $0.agentId == aid }?.id ?? { seq += 1; return "conv-\(seq)" }()
        var run = Run(events: [], conversationId: convId)
        lastConversationId = convId
        if !onDevice(prov.id), prov.mailConsentAtMs == nil {
            awaitingConsent = true
            if !conversations.contains(where: { $0.id == convId }) { conversations.append(AgentConversation(id: convId, agentId: aid, turns: [])) }
            run.events.append(.agentConsentNeeded(conversationId: convId, providerId: prov.id, providerName: prov.name))
            return run
        }
        awaitingConsent = false
        if !onDevice(prov.id) {
            if let why = providerFails { throw CoreError.Action(failure: ActionFailed(accountName: prov.name, reasonWords: failureWords(why), retryable: false)) }
            if !credentialStored.contains(prov.id) { throw CoreError.Invalid(field: "provider", reasonWords: "\(prov.name) has no key yet. Add one in Settings: AI providers.") }
            FixtureProviderRequests.note()   // hid-ai F2: the cloud provider would be asked here
        }
        var turns = conversations.first { $0.id == convId }?.turns ?? []
        turns.append(AgentTurn(fromOwner: true, text: q, citations: [], atMs: nowMs))
        var steps = 0
        func step(_ words: String) {
            steps += 1
            run.events.append(.agentStep(conversationId: convId, number: UInt32(steps), words: words))
        }
        let wantsLabel = q.lowercased().contains("label")
        let previous = answers[convId]?.citations.map(\.messageId) ?? []
        var text = ""
        var shown: [FixtureMail] = []
        var proposal: AgentProposal?
        if wantsLabel {
            if !agent.allowed.contains(.label) {
                step("Checked what this agent may do")
                text = "This agent is not allowed to label messages. Allow “label” for it in Settings: AI providers."
            } else if let name = Self.labelName(q) {
                shown = previous.compactMap { id in mail.first { $0.id == id } }
                step("Looked at the \(shown.count) message\(shown.count == 1 ? "" : "s") from the last answer")
                if shown.isEmpty {
                    text = "Ask a question first, then I can label the messages it found."
                } else {
                    seq += 1
                    let words = shown.count == 1 ? "Label this message as “\(name)”" : "Label these \(shown.count) messages as “\(name)”"
                    let p = AgentProposal(id: "\(convId)-\(nowMs)-\(seq)", agentId: aid, actionWords: words, messageIds: shown.map(\.id), label: name, decision: .pending, draftId: nil)
                    stored.append(Stored(proposal: p, ids: shown.map(\.id), label: name))
                    proposal = p
                    run.events.append(.proposalReady(proposalId: p.id))
                    text = "I can \(words.prefix(1).lowercased() + words.dropFirst()). Nothing changes until you approve."
                }
            } else {
                step("Looked for the label name")
                text = "Say which label to use, for example “label these as Supplier quotes”."
            }
        } else if !agent.allowed.contains(.search) || !agent.allowed.contains(.readMail) {
            step("Checked what this agent may do")
            text = Self.nothingFound
        } else {
            let terms = Self.terms(q)
            step("Searched your mail for “\(terms.first ?? q)”")
            let scored = mail.enumerated().compactMap { i, m -> (Int, Int, FixtureMail)? in
                let s = terms.filter { m.text.contains($0) }.count
                return s > 0 ? (s, i, m) : nil
            }.sorted { $0.0 != $1.0 ? $0.0 > $1.0 : $0.1 < $1.1 }
            shown = Array(scored.prefix(WhooshConstants.AGENT_CONTEXT_CHUNKS).map(\.2))
            if !shown.isEmpty && steps < WhooshConstants.AGENT_MAX_STEPS { step("Read \(shown.count) message\(shown.count == 1 ? "" : "s")") }
            text = shown.isEmpty ? Self.nothingFound
                : "I found \(shown.count) message\(shown.count == 1 ? "" : "s") that fit: " + shown.map { "“\($0.subject)” from \($0.sender)" }.joined(separator: "; ") + "."
        }
        for chunk in Self.chunks(text) { run.events.append(.agentDelta(conversationId: convId, text: chunk)) }
        lastChunks = Self.chunks(text).count
        lastSteps = steps
        let cites = wantsLabel && proposal == nil ? [] : shown.map { Citation(messageId: $0.id, quote: String($0.preview.prefix(WhooshConstants.SEARCH_SNIPPET_CHARS)), accountId: $0.accountId, senderName: $0.sender, receivedAtMs: $0.receivedAtMs) }
        let answer = AskAnswer(kind: .answer, text: text, citations: cites)
        answers[convId] = answer
        turns.append(AgentTurn(fromOwner: false, text: text, citations: cites, atMs: nowMs))
        if let i = conversations.firstIndex(where: { $0.id == convId }) { conversations[i] = AgentConversation(id: convId, agentId: aid, turns: turns) }
        else { conversations.append(AgentConversation(id: convId, agentId: aid, turns: turns)) }
        return run
    }

    /// "label these as Supplier quotes" -> "Supplier quotes"; the name may be quoted.
    static func labelName(_ q: String) -> String? {
        guard let r = q.range(of: " as ", options: .caseInsensitive) else { return nil }
        let n = q[r.upperBound...].trimmingCharacters(in: CharacterSet.whitespacesAndNewlines.union(CharacterSet(charactersIn: "\"“”'.")))
        return n.isEmpty ? nil : n
    }

    /// The answer's text in order, split at the end of each clause: the chunks joined are the text.
    static func chunks(_ text: String) -> [String] {
        var out: [String] = [], cur = ""
        for ch in text { cur.append(ch); if ch == ";" || ch == "." { out.append(cur); cur = "" } }
        if !cur.isEmpty { out.append(cur) }
        return out
    }

    func proposals(conversationId: String) -> [AgentProposal] {
        guard let agent = conversations.first(where: { $0.id == conversationId })?.agentId else { return [] }
        return stored.filter { $0.proposal.agentId == agent }.map(\.proposal)
    }

    /// Approve persists Approved BEFORE the command runs; a second approve is refused; a refused command puts the
    /// proposal back to pending (agents crate, approve_proposal). `apply` runs the label; it may throw.
    func decide(_ id: String, approve: Bool, nowMs: Int64, apply: (_ ids: [String], _ label: String) throws -> CommandOutcome) throws -> CommandOutcome {
        guard let i = stored.firstIndex(where: { $0.proposal.id == id }) else { throw CoreError.NotFound(kind: "proposal", id: id) }
        guard case .pending = stored[i].proposal.decision else { throw CoreError.Invalid(field: "proposal", reasonWords: "this proposal was decided already") }
        if !approve {
            stored[i].proposal.decision = .rejected(atMs: nowMs)
            return CommandOutcome(undoEntryId: nil, createdId: nil)
        }
        stored[i].proposal.decision = .approved(atMs: nowMs)
        do { return try apply(stored[i].ids, stored[i].label) }
        catch { stored[i].proposal.decision = .pending; throw error }
    }

    func stateWord(_ i: Int) -> String {
        switch stored[i].proposal.decision { case .pending: return "pending"; case .approved: return "approved"; case .rejected: return "rejected" }
    }

    func reset() {
        conversations = []; answers = [:]; stored = []; lastConversationId = nil; lastSteps = 0; lastChunks = 0; lastQuoted = nil; awaitingConsent = false
        for i in providers.indices { providers[i].mailConsentAtMs = nil }
    }
}

extension FixtureCore {
    /// `fixture providers none|default` (FixtureVerbs.swift): see `FixtureAgentBook.stageProviders`. Fixture core only: the verb is reached through `host.fixture`, which the real core never sets.
    public func fixtureForwardRefuse(_ on: Bool) { lock.withLock { agents.forwardRefusal = on ? FixtureAgentBook.forwardRefusedWords : nil } }

    /// `fixture star <message id> on|off`: the same set the Star command writes (`starred`), so `message/<id>/starred`, the Starred view, a rule's Favori fact and the counts all read it.
    /// `fixture reset` clears the set (`resetMessages`, FixtureCore.swift: `starred = []`). An id the core does not hold is refused.
    public func fixtureStar(_ id: String, on: Bool) throws {
        try lock.withLock {
            guard fxMessage(id) != nil, !deleted.contains(id) else { throw CoreError.NotFound(kind: "message", id: id) }
            if on { starred.insert(id) } else { starred.remove(id) }
        }
        emit()
    }

    public func fixtureProviders(none: Bool) {
        lock.withLock { agents.stageProviders(none: none) }
        emit()
    }
}

extension FixtureCore {
    /// State `label/<id>/rerun/preview` on the fixture core: what a re-run of the label would do, in the foot's terms. A mirror of the engine's
    /// `rerun_preview` (core/crates/engine/src/flow_ctx.rs:259-277) over the fixture's own labels, entry rules and messages. Called with the lock held.
    ///   engine `candidate_parts` (flow_ctx.rs:181-211): filed = INBOX messages under the label; rule = messages the entry chains match (only when the entry card is on);
    ///     ai = the newest 500 inbound INBOX messages when the AI card is on and the label has a description (`ai_wanted`); beyond = inbound count - 500.
    ///   engine `rerun_preview`: by_rule = rule minus filed; by_ai = ai minus filed minus rule when a model can answer, else 0; leave (flow_ctx.rs:266) = 0 when
    ///     (no rule match and no entry chain) or ai_wanted, else filed minus rule; stay = filed - leave; ai_words (flow_ctx.rs:267) only when ai_wanted and no model can answer.
    ///   "a model can answer" (decision_model.rs `refusal`, :231-242): the saved decision model is Laya, or Jev with its key stored and the owner's consent given; else the refusal
    ///     words of `ai_cannot_words` (flow_ctx.rs:251-257): none chosen -> "no model on this Mac", Jev without key / without consent -> the two lines.
    /// Not mirrored: `continuation` (flow_ctx.rs:236-239): a re-run here is never partial, so nothing is judged yet and skip is 0. Left over: the fixture re-run (rerunLabelLocked)
    /// still files by the fixture model's meaning whatever the saved decision model is, so with no model the preview says 0 by the AI while a re-run of a described label can still file.
    func rerunPreviewLocked(_ id: String) -> String? {
        guard let label = labelRows.first(where: { $0.id == id }), label.kind != .builtIn else { return nil }
        let own = Set(ownAddressesLocked().map { $0.lowercased() })
        let inbox = messages.filter { !deleted.contains($0.id) && !archived.contains($0.id) && !spam.contains($0.id) && !muted.contains($0.id) }
        let inboxIds = Set(inbox.map(\.id))
        let filed = Set(filedUnderLocked(id).filter { inboxIds.contains($0) })
        let chains = label.entryEnabled ? entryChainsLocked(id).filter { !$0.rules.conditions.isEmpty } : []
        var rule = Set<String>()
        if label.entryEnabled { for f in corpusFactsLocked() where firstMatchingChain(chains: chains, facts: f) != nil { rule.insert(f.messageId) } }
        let aiWanted = label.aiEnabled && !label.description.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        var refusal: String?
        switch settings.decision?.provider {
        case .some(.local): refusal = nil
        case .some(.jev): refusal = !agents.credentialStored.contains("jev") ? "the Jev key is missing." : (!decisionConsentGiven.contains("jev") ? "you have not allowed Jev to read your mail." : nil)
        default: refusal = "no model on this Mac."
        }
        let aiOk = refusal == nil
        let inbound = inbox.filter { !own.contains($0.from.lowercased()) }.sorted { (receivedAt[$0.id] ?? 0, $0.id) > (receivedAt[$1.id] ?? 0, $1.id) }
        let window = aiWanted ? inbound.prefix(500).map(\.id) : []
        // SPEC A140: with the AI on, "by_rule" is the mails step 1 picks that the AI has not ruled on (not filed here, no "ruled, not accepted" mark, no exit rejection),
        // and the AI's own part is counted there, never a second time: with an entry chain "by_ai" is 0 (the AI reads only what step 1 picks); with none the AI reads the window.
        // A filed mail step 1 no longer picks leaves on a re-run; with the AI off the rule files, as before.
        let ruled: (String) -> Bool = { filed.contains($0) || self.flowCard[$0]?[id] != nil || self.flowRejected[$0]?.ai == id }
        let byRule = aiWanted ? rule.filter { !ruled($0) }.count : rule.filter { !filed.contains($0) }.count
        let byAi = (aiOk && aiWanted && chains.isEmpty) ? window.filter { !filed.contains($0) }.count : 0
        let leave = aiWanted ? (chains.isEmpty ? 0 : filed.filter { !rule.contains($0) }.count)
                             : ((rule.isEmpty && chains.isEmpty) ? 0 : filed.filter { !rule.contains($0) }.count)
        let beyond = (aiOk && aiWanted) ? max(0, inbound.count - 500) : 0
        var words = "null"
        if aiWanted, let r = refusal, let d = try? JSONSerialization.data(withJSONObject: "The AI cannot sort yet: " + r, options: [.fragmentsAllowed]), let s = String(data: d, encoding: .utf8) { words = s }
        return "{\"by_rule\":\(byRule),\"by_ai\":\(byAi),\"leave\":\(leave),\"stay\":\(max(0, filed.count - leave)),\"beyond\":\(beyond),\"ai_words\":\(words)}"
    }
}
