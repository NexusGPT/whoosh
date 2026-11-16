import Foundation

// Delta 17 in the fixture core: a label is a flow of three cards (Entry rules, AI sorting, Exit filter). docs/lanes/contracts.md "delta 17".
// The rules themselves are the contract's pure functions (checkSetCard, checkSetExitFallback, checkSetAiReads, validateChain,
// firstMatchingChain, exitVerdict, nextChainId, countLabelFlow, whyHere): the Rust core calls the same ones. This file keeps the state.
// The entry chains are `labelRuleSets` (delta 9's rule sets, in chain order, their numbers in `entryChainIds`); the exit chains are `exitChains`.

extension FixtureCore {
    // MARK: chains

    /// The entry chains of a label with their numbers. A rule set added by another path (the sheet's first, an always-label rule) takes the next number.
    func entryChainsLocked(_ id: String) -> [LabelChain] {
        let sets = labelRuleSets[id] ?? []
        var ids = Array((entryChainIds[id] ?? []).prefix(sets.count))
        while ids.count < sets.count { ids.append(nextChainId(existing: ids)) }
        entryChainIds[id] = ids
        return zip(ids, sets).map { LabelChain(chainId: $0.0, rules: $0.1) }
    }
    func exitChainsLocked(_ id: String) -> [LabelChain] { exitChains[id] ?? [] }
    func chainsLocked(_ id: String, _ placement: ChainPlacement) -> [LabelChain] { placement == .entry ? entryChainsLocked(id) : exitChainsLocked(id) }

    func writeChainsLocked(_ id: String, _ placement: ChainPlacement, _ chains: [LabelChain]) {
        if placement == .entry {
            labelRuleSets[id] = chains.isEmpty ? nil : chains.map(\.rules)
            entryChainIds[id] = chains.isEmpty ? nil : chains.map(\.chainId)
        } else {
            exitChains[id] = chains.isEmpty ? nil : chains
        }
        flowEdited.insert(id)
    }

    // MARK: the commands

    func setLabelCardLocked(_ id: String, _ card: FlowCard, _ enabled: Bool) throws -> CommandOutcome {
        let l = try labelRow(id)
        try checkSetCard(label: l, card: card, enabled: enabled)
        setLabel(id) { switch card { case .entry: $0.entryEnabled = enabled; case .ai: $0.aiEnabled = enabled; case .exit: $0.exitEnabled = enabled } }
        flowEdited.insert(id)
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    func setLabelAiReadsLocked(_ id: String, _ reads: AiReads) throws -> CommandOutcome {
        _ = try labelRow(id)
        try checkSetAiReads(reads: reads)
        setLabel(id) { $0.aiReads = reads }
        flowEdited.insert(id)
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    func setLabelExitFallbackLocked(_ id: String, _ fallback: String?) throws -> CommandOutcome {
        let l = try labelRow(id)
        try checkSetExitFallback(label: l, fallback: fallback, labels: labelRows)
        setLabel(id) { $0.exitFallback = fallback?.trimmingCharacters(in: .whitespaces) }
        flowEdited.insert(id)
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    func addLabelChainLocked(_ id: String, _ placement: ChainPlacement, _ chain: RuleSet) throws -> CommandOutcome {
        _ = try labelRow(id)
        try validateChain(rules: chain)
        var chains = chainsLocked(id, placement)
        let n = nextChainId(existing: chains.map(\.chainId))
        chains.append(LabelChain(chainId: n, rules: chain))
        writeChainsLocked(id, placement, chains)
        return CommandOutcome(undoEntryId: nil, createdId: String(n))
    }

    func updateLabelChainLocked(_ id: String, _ placement: ChainPlacement, _ chainId: UInt32, _ chain: RuleSet) throws -> CommandOutcome {
        _ = try labelRow(id)
        var chains = chainsLocked(id, placement)
        guard let i = chains.firstIndex(where: { $0.chainId == chainId }) else { throw CoreError.NotFound(kind: "chain", id: "\(id)/\(chainId)") }
        try validateChain(rules: chain)
        chains[i] = LabelChain(chainId: chainId, rules: chain)
        writeChainsLocked(id, placement, chains)
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    func removeLabelChainLocked(_ id: String, _ placement: ChainPlacement, _ chainId: UInt32) throws -> CommandOutcome {
        _ = try labelRow(id)
        var chains = chainsLocked(id, placement)
        guard chains.contains(where: { $0.chainId == chainId }) else { throw CoreError.NotFound(kind: "chain", id: "\(id)/\(chainId)") }
        chains.removeAll { $0.chainId == chainId }
        writeChainsLocked(id, placement, chains)
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    // MARK: the example of the board (mac-main: À répondre)

    /// The label the board draws: the file's `flow_example` rows (1 caught by the entry rule, 31 picked by the AI, 23 kept by the exit filter, 8 sent
    /// to Primary) are filed as real messages, so every figure of its flow is derived from the filings, before and after an edit or a `fixture reply`.
    /// docs/design/boards/labels-flow-r1/v4.html is the source of every value below.
    static let exampleLabelId = "needs-reply"

    /// The example label's configuration: entry chain (the body says "Hi Assem"), the AI reads subject and body, exit chain (unanswered: yes), the others go to Primary. Called after the built-in labels are installed.
    func seedFlowExampleLocked() {
        let id = Self.exampleLabelId
        guard labelRows.contains(where: { $0.id == id }) else { return }
        setLabel(id) {
            $0.description = "Emails waiting for my reply: a question, a request, a decision someone is waiting on."
            $0.entryEnabled = true; $0.aiEnabled = true; $0.exitEnabled = true
            $0.aiReads = AiReads(subject: true, body: true, attachments: false)
            $0.exitFallback = "primary"
        }
        // A140: step 1 picks the mails the AI reads: every mail whose body opens "Hi Assem" (the 32 rows below, and the corpus mails that open the same way).
        labelRuleSets[id] = [RuleSet(conditions: [.bodyContains(words: "Hi Assem")], matchMode: .all)]
        entryChainIds[id] = [0]
        exitChains[id] = [LabelChain(chainId: 0, rules: RuleSet(conditions: [.fact(fact: .unanswered, wanted: true)], matchMode: .all))]
        flowEdited.remove(id)
    }

    // MARK: the rows that exercise every card, and `fixture reply`

    /// Files the file's `flow_example` rows as inserted rows (outside the tab figures) and runs each through the three steps of A140. Every row passes step 1
    /// (its body opens "Hi Assem"). "entry": step 1 picked it and the AI ruled it out (nothing in it fits the description: it is not in the label, only counted as
    /// picked). "kept" and "rejected": step 1 picked them and the AI accepted them (the fixture model offers one pick per row, so the ruling is stated by the row); step 3
    /// keeps an unanswered one and sends an answered one to the fallback. The file holds 1 entry row, 23 kept and 8 rejected: 32 rows picked by step 1 (plus the corpus
    /// mails that open "Hi Assem"), 31 accepted by the AI, 23 kept by the exit filter, 8 sent to the fallback; after a reply on one kept message 22 kept and 9 sent.
    func seedFlowMessagesLocked() {
        let lid = Self.exampleLabelId
        guard labelRows.contains(where: { $0.id == lid }) else { return }
        for r in fx.flow_example ?? [] {
            guard !messages.contains(where: { $0.id == r.id }), accountIdsLocked().contains(r.account) else { continue }
            let m = FixtureFile.Message(id: r.id, sender: r.sender, from: r.from, account: r.account, subject: r.subject, preview: String(r.body.prefix(160)), time: "",
                                        unread: true, pinned: false, attachments: r.attachments, thread_count: 1, trackers_blocked: 0, tab: "primary", body: [r.body],
                                        quoted: nil, date: nil, thread: nil)
            messages.insert(m, at: 0); order.insert(r.id, at: 0)
            account[r.id] = r.account; receivedAt[r.id] = clock.nowMs(); pinned[r.id] = false; inserted.insert(r.id)
            switch r.step {
            case "entry":
                if case .picked(let c) = entryReadLocked(labelRows.first { $0.id == lid }!, facts(m)) { flowCard[r.id, default: [:]][lid] = (.entry, c) }   // picked, the AI said no
            case "kept": unansweredState[r.id] = true; fileByModelLocked(r.id, lid, facts(m))
            case "rejected": unansweredState[r.id] = false; repliedByMe.insert(r.id); fileByModelLocked(r.id, lid, facts(m))
            default: break
            }
        }
    }

    /// `fixture reply <messageId>`: flips RepliedByMe and Unanswered for one message and asks that message's exit filter again at once. A message
    /// the filter now rejects moves to the label's fallback; one it now keeps comes back from the fallback. A message with no AI pick only flips.
    /// Returns the facts after the flip and the label the message ends in (nil = no label). Throws NotFound for an unknown message.
    @discardableResult
    public func fixtureReply(messageId id: String) throws -> (repliedByMe: Bool, unanswered: Bool, endedIn: String?) {
        let out: (Bool, Bool, String?) = try lock.withLock {
            guard let m = fxMessage(id), !deleted.contains(id) else { throw CoreError.NotFound(kind: "message", id: id) }
            let wasUnanswered = facts(m).unanswered
            let factsBefore = facts(m)
            if repliedByMe.contains(id) { repliedByMe.remove(id) } else { repliedByMe.insert(id) }
            unansweredState[id] = !wasUnanswered
            // the AI's pick: the label it was rejected from, else the label it sits in by the AI card or the exit card
            let pick: String? = flowRejected[id]?.ai ?? (filedBy[id] ?? [:]).keys.sorted().first { lid in
                let card = flowCard[id]?[lid]?.card
                return card == .ai || card == .exit
            }
            var left = false
            if let lid = pick, let pl = labelRows.first(where: { $0.id == lid }) {
                var wasPicked = false
                if case .picked = entryReadLocked(pl, factsBefore) { wasPicked = true }
                if wasPicked, case .notPicked = entryReadLocked(pl, facts(m)) {   // step 1 reads the flipped fact at once: a mail it picked and no longer picks leaves
                    if let r = flowRejected[id], labelRows.contains(where: { $0.id == r.to }) { unfileMessage(id, from: r.to) }
                    unfileMessage(id, from: lid); flowCard[id]?[lid] = nil; flowRejected[id] = nil; left = true
                    flowEdited.insert(lid)
                } else if pl.exitEnabled {
                    if let r = flowRejected[id], labelRows.contains(where: { $0.id == r.to }) { unfileMessage(id, from: r.to) }   // leave the fallback label before the verdict
                    fileByModelLocked(id, lid, facts(m), byEntry: !aiRulesIn(pl))   // step 3 reads the new facts at once; the AI is not asked again
                    flowEdited.insert(lid)
                }
            }
            let ended: String?
            if left { ended = (assigned[id] ?? []).first }
            else if let r = flowRejected[id] { ended = labelRows.contains(where: { $0.id == r.to }) ? r.to : nil }
            else { ended = pick ?? (assigned[id] ?? []).first }
            return (repliedByMe.contains(id), unansweredState[id] ?? false, ended)
        }
        emit()
        return out
    }

    /// `fixture ai-set-aside <label> <n>`: the count the card reads at `label/<id>/flow/ai_set_aside` (the real core's number, fix-aisort round 11); `fixture reset` clears it.
    public func armSetAside(label: String, count: Int) {
        lock.withLock { aiSetAside[label] = count }
        emit()
    }

    // MARK: filings, counts and "why here"

    /// How the fixture read a filing before a card was recorded for it: a rule = the entry card, a model = the AI card, the owner or the default = by hand.
    private func cardOf(_ d: DecidedBy, exitOn: Bool) -> WhyCard {
        switch d { case .rule: return .entry; case .laya, .jev, .apple, .model: return exitOn ? .exit : .ai; case .owner, .default: return .hand }
    }

    func flowFilingsLocked() -> [FlowFiling] {
        var out: [FlowFiling] = []
        for m in messages where !deleted.contains(m.id) {
            if let r = flowRejected[m.id] {
                let landed = labelRows.contains { $0.id == r.to } && (filedBy[m.id]?[r.to] != nil)
                out.append(FlowFiling(messageId: m.id, endedIn: landed ? r.to : nil, card: .fallback, aiPick: r.ai))
                continue
            }
            for (labelId, d) in filedBy[m.id] ?? [:] {
                let exitOn = labelRows.first { $0.id == labelId }?.exitEnabled ?? false
                let card = flowCard[m.id]?[labelId]?.card ?? cardOf(d, exitOn: exitOn)
                out.append(FlowFiling(messageId: m.id, endedIn: labelId, card: card, aiPick: (card == .ai || card == .exit) ? labelId : nil))
            }
        }
        return out
    }

    /// The messages step 1 of the label picks: the step is on, it has a chain, and a chain matches (SPEC A140). A label with no chain picks nothing here
    /// (every mail passes, nothing is "picked"). Derived from the mail as it is now, so a fact that flips is re-read at once.
    func pickedByEntryLocked(_ l: MailLabel) -> [String] {
        messages.filter { m in
            guard !deleted.contains(m.id) else { return false }
            if case .picked = entryReadLocked(l, facts(m)) { return true }
            return false
        }.map(\.id)
    }

    /// The mails step 1 picked that the AI has not ruled on yet for this label: no filing, no "ruled, not accepted" mark, no exit rejection. Always 0 when the AI is off.
    func waitingForAiLocked(_ l: MailLabel) -> Int {
        guard aiRulesIn(l) else { return 0 }
        return pickedByEntryLocked(l).filter { filedBy[$0]?[l.id] == nil && flowCard[$0]?[l.id] == nil && flowRejected[$0]?.ai != l.id }.count
    }

    /// The four figures with the pipeline's meaning (SPEC A140): `caught_by_entry` = the mails step 1 picked; `sorted_by_ai` = the mails the AI accepted
    /// (kept by the exit filter, sent to the fallback, or filed with no exit filter), 0 when the AI is off; `kept_by_exit` and `sent_to_fallback` from the filings.
    func flowCountsLocked(_ id: String) -> LabelFlowCounts {
        let base = countLabelFlow(labelId: id, filings: flowFilingsLocked())
        guard let l = labelRows.first(where: { $0.id == id }) else { return base }
        return LabelFlowCounts(labelId: id, caughtByEntry: UInt32(pickedByEntryLocked(l).count), sortedByAi: aiRulesIn(l) ? base.sortedByAi : 0,
                               keptByExit: base.keptByExit, sentToFallback: base.sentToFallback)
    }

    /// Step 1's own reading of a message the AI accepted into the label: its chain and the conditions that held. Nil when the label has no entry chain
    /// that picks the message (the AI saw every mail), or the AI is off (the entry card is then the whole answer).
    func entryWhyLocked(message mid: String, label lid: String) -> WhyHere? {
        guard let m = fxMessage(mid), let l = labelRows.first(where: { $0.id == lid }), aiRulesIn(l) else { return nil }
        let f = facts(m)
        guard case .picked(let c) = entryReadLocked(l, f), let chain = entryChainsLocked(lid).first(where: { $0.chainId == c }) else { return nil }
        return whyHere(card: .entry, chain: chain, facts: f, decidedBy: .laya)
    }

    func whyHereLocked(message mid: String, label lid: String) -> WhyHere? {
        guard let m = fxMessage(mid) else { return nil }
        let f = facts(m)
        if let r = flowRejected[mid], r.to == lid, filedBy[mid]?[lid] == nil {   // went to Primary or Junk: no label holds it
            return whyHere(card: .fallback, chain: nil, facts: f, decidedBy: .laya)
        }
        guard let d = filedBy[mid]?[lid] else { return nil }
        let recorded = flowCard[mid]?[lid]
        let exitOn = labelRows.first { $0.id == lid }?.exitEnabled ?? false
        let card = recorded?.card ?? cardOf(d, exitOn: exitOn)
        var chain: LabelChain?
        if card == .entry || card == .exit {
            let chains = chainsLocked(lid, card == .entry ? .entry : .exit)
            let wanted = recorded?.chain ?? firstMatchingChain(chains: chains, facts: f)
            chain = chains.first { $0.chainId == wanted }
        }
        return whyHere(card: card, chain: chain, facts: f, decidedBy: d)
    }

    // MARK: state paths (the socket's `core state`)

    private func flowJSON(_ v: Any) -> String? {
        guard JSONSerialization.isValidJSONObject([v]) || v is String || v is Bool || v is Int || v is UInt32 else { return nil }
        guard let d = try? JSONSerialization.data(withJSONObject: v, options: [.fragmentsAllowed, .sortedKeys]) else { return nil }
        return String(data: d, encoding: .utf8)
    }
    private func whyCardWords(_ c: WhyCard) -> String {
        switch c { case .entry: return "entry"; case .ai: return "ai"; case .exit: return "exit"; case .fallback: return "fallback"; case .hand: return "hand" }
    }
    private func decidedByWords(_ d: DecidedBy) -> String {
        switch d { case .rule(let id): return "rule:\(id)"; case .laya: return "laya"; case .jev: return "jev"; case .apple: return "apple"; case .model(let id): return "model:\(id)"; case .owner: return "owner"; case .default: return "default" }
    }
    private func stateWords(_ s: FactState) -> String { switch s { case .any: return "any"; case .yes: return "yes"; case .no: return "no" } }
    private func conditionsJSON(_ cs: [RuleCondition], _ mode: MatchMode) -> [[String: Any]] {
        (FixtureRuleJSON.encode(RuleSet(conditions: cs, matchMode: mode))["conditions"] as? [[String: Any]]) ?? []
    }
    private func factsJSON(_ f: MessageFacts) -> [String: Bool] {
        [messageFactKey(fact: .unread): f.unread, messageFactKey(fact: .unanswered): f.unanswered, messageFactKey(fact: .hasAttachment): f.hasAttachment,
         messageFactKey(fact: .starred): f.starred, messageFactKey(fact: .vipSender): f.vipSender, messageFactKey(fact: .ccMe): f.ccMe,
         messageFactKey(fact: .mailingList): !f.listHeader.isEmpty, messageFactKey(fact: .repliedByMe): f.repliedByMe]
    }
    private func chainJSON(_ c: LabelChain) -> [String: Any] {
        let facts = Dictionary(uniqueKeysWithValues: factStates(conditions: c.rules.conditions).map { (messageFactKey(fact: $0.fact), stateWords($0.state)) })
        return ["chain_id": Int(c.chainId), "match": c.rules.matchMode == .all ? "all" : "any", "conditions": conditionsJSON(c.rules.conditions, c.rules.matchMode),
                "facts": facts, "rows": conditionsJSON(rowConditions(conditions: c.rules.conditions), c.rules.matchMode)]
    }

    /// nil = not a delta 17 path. A path that is one, with nothing there, answers `D9Hit(value: nil)`. Called with the lock held.
    func flowStateLocked(_ p: [String]) -> D9Hit? {
        switch (p.first, p.count) {
        case ("label", 4) where p[2] == "flow":      // label/<id>/flow/<field>
            guard let l = labelRows.first(where: { $0.id == p[1] }) else { return D9Hit(value: nil) }
            switch p[3] {
            case "entry_enabled": return D9Hit(value: flowJSON(l.entryEnabled))
            case "ai_enabled": return D9Hit(value: flowJSON(l.aiEnabled))
            case "exit_enabled": return D9Hit(value: flowJSON(l.exitEnabled))
            case "ai_reads":
                // the real engine's path holds three keys (core/crates/engine/src/label_flow.rs:90, docs/FIRST-RUN.json): a new installation must equal it, so the fourth is
                // served only while it is on; a reader takes a missing `rest_of_thread` as false
                var reads: [String: Any] = ["subject": l.aiReads.subject, "body": l.aiReads.body, "attachments": l.aiReads.attachments]
                if l.aiReads.restOfThread { reads["rest_of_thread"] = true }
                return D9Hit(value: flowJSON(reads))
            case "exit_fallback": return D9Hit(value: l.exitFallback.flatMap { flowJSON($0) })
            case "ai_set_aside": return D9Hit(value: flowJSON(aiSetAside[l.id] ?? 0))
            case "ai_refusal":
                if let first = firstRunRefusalLocked() { return D9Hit(value: first) }   // a new installation with no decision model (FixtureFirstRun.swift)
                // the engine's `ai_under_trust` (the AI judged mails and was sure of none), served for ONE sample label so the card's sentence can be judged: the built-in Social,
                // 42 judged, best 58 %, files from 80 %
                if l.id == "social", l.aiEnabled {
                    return D9Hit(value: "{\"code\":\"ai_under_trust\",\"highest_percent\":58,\"judged\":42,\"trust_percent\":80,\"words\":\"The AI judged %d mails and was sure of none: its best was %d %%, and it files from %d %%.\"}")
                }
                // the engine's `ai_needs_apple` (Apple's on-device model judges the label AI step and Apple Intelligence is off), served for the built-in Updates
                if l.id == "updates", l.aiEnabled {
                    return D9Hit(value: "{\"code\":\"ai_needs_apple\",\"words\":\"Sorting mail into labels needs Apple Intelligence: turn it on in System Settings, then Apple Intelligence and Siri.\"}")
                }
                return D9Hit(value: nil)
            case "counts":
                // a new installation has synced nothing yet: the golden (docs/FIRST-RUN.json, read from the real core on a fresh store) says all zeros, whatever mail the fixture holds
                if newInstall { return D9Hit(value: flowJSON(["caught_by_entry": 0, "sorted_by_ai": 0, "kept_by_exit": 0, "sent_to_fallback": 0, "waiting_for_ai": 0])) }
                let c = flowCountsLocked(l.id)
                return D9Hit(value: flowJSON(["caught_by_entry": Int(c.caughtByEntry), "sorted_by_ai": Int(c.sortedByAi), "kept_by_exit": Int(c.keptByExit), "sent_to_fallback": Int(c.sentToFallback),
                                              "waiting_for_ai": waitingForAiLocked(l)]))
            default: return D9Hit(value: nil)
            }
        case ("label", 4) where p[2] == "chains":    // label/<id>/chains/<entry|exit>
            guard labelRows.contains(where: { $0.id == p[1] }), let placement = ["entry": ChainPlacement.entry, "exit": .exit][p[3]] else { return D9Hit(value: nil) }
            return D9Hit(value: flowJSON(chainsLocked(p[1], placement).map(chainJSON)))
        case ("label", 5) where p[2] == "chains" && p[4] == "count":
            guard labelRows.contains(where: { $0.id == p[1] }), let placement = ["entry": ChainPlacement.entry, "exit": .exit][p[3]] else { return D9Hit(value: nil) }
            return D9Hit(value: flowJSON(chainsLocked(p[1], placement).count))
        case ("message", 4) where p[2] == "why":     // message/<id>/why/<label id | primary | junk>
            guard let w = whyHereLocked(message: p[1], label: p[3]) else { return D9Hit(value: nil) }
            var out: [String: Any] = ["card": whyCardWords(w.card), "chain_id": w.chainId.map { Int($0) as Any } ?? NSNull(), "decided_by": decidedByWords(w.decidedBy),
                                      "matched": conditionsJSON(w.matched, .all)]
            if w.card == .ai || w.card == .exit, let l = labelRows.first(where: { $0.id == p[3] }), aiRulesIn(l) {   // A140: the AI accepted this mail
                out["ai"] = "yes"
                if let e = entryWhyLocked(message: p[1], label: p[3]) {   // ... and step 1 had picked it: its chain and the conditions that held
                    out["entry"] = ["chain_id": e.chainId.map { Int($0) as Any } ?? NSNull(), "matched": conditionsJSON(e.matched, .all)] as [String: Any]
                }
            }
            return D9Hit(value: flowJSON(out))
        case ("message", 3) where p[2] == "facts":   // message/<id>/facts: the eight facts, each true or false, keyed as `messageFactKey`
            guard let m = fxMessage(p[1]) else { return D9Hit(value: nil) }
            return D9Hit(value: flowJSON(factsJSON(facts(m))))
        case ("message", 4) where p[2] == "facts":   // message/<id>/facts/<unread|unanswered|has_attachment|starred|vip_sender|cc_me|mailing_list|replied_by_me>
            guard let m = fxMessage(p[1]), let v = factsJSON(facts(m))[p[3]] else { return D9Hit(value: nil) }
            return D9Hit(value: flowJSON(v))
        case ("message", 3) where p[2] == "flow_rejection":
            guard let r = flowRejected[p[1]] else { return D9Hit(value: nil) }
            return D9Hit(value: flowJSON(["ai_label_id": r.ai, "sent_to": r.to, "chain_id": r.chain.map { Int($0) as Any } ?? NSNull()]))
        default: return nil
        }
    }
}
