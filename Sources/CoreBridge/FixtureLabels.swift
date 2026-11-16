import Foundation

// Delta 9 in the fixture core: labels are the tabs, label rules and re-runs, suggestions and blocks, the mailbox selector.
// The rules themselves are the contract's pure functions (reorderLabel, validateLabelSpec, ruleSetMatches, planRerun,
// suggestionEffect, isBlocked): the Rust core calls the same ones, so a rule is written once. This file only keeps the state.

extension FixtureFile.ConditionNode {
    var condition: RuleCondition? {
        switch kind {
        case "from": return .from(words: words ?? "")
        case "to": return .to(words: words ?? "")
        case "cc": return .cc(words: words ?? "")
        case "domain": return .domain(domain: domain ?? "")
        case "subject": return .subjectContains(words: words ?? "")
        case "body": return .bodyContains(words: words ?? "")
        case "list_header": return .listHeader(words: words ?? "")
        case "account": return .account(accountId: account_id ?? "")
        case "has_attachment": return .hasAttachment
        case "not":   // round 18: the negated twin (its field's key and its value)
            let keys: [String: RuleField] = ["from": .from, "to": .to, "cc": .cc, "domain": .domain, "subject": .subjectContains, "body": .bodyContains, "list_header": .listHeader, "account": .account]
            return field.flatMap { keys[$0] }.map { RuleCondition.not(field: $0, value: value ?? "") }
        case "fact": return fact.flatMap { messageFactFromKey(key: $0) }.map { RuleCondition.fact(fact: $0, wanted: wanted ?? true) }   // delta 17
        default: return nil
        }
    }
}
extension FixtureFile.RuleNode {
    var ruleSet: RuleSet { RuleSet(conditions: conditions.compactMap(\.condition), matchMode: match == "all" ? .all : .any) }
}

enum FixtureRuleJSON {
    /// The rule set in the store's JSON shape: {"match":"any|all","conditions":[{"kind":"from","words":"x"}, ...]}.
    static func encode(_ r: RuleSet) -> [String: Any] {
        let cs: [[String: Any]] = r.conditions.map { c in
            switch c {
            case .from(let w): return ["kind": "from", "words": w]
            case .to(let w): return ["kind": "to", "words": w]
            case .cc(let w): return ["kind": "cc", "words": w]
            case .domain(let d): return ["kind": "domain", "domain": d]
            case .subjectContains(let w): return ["kind": "subject", "words": w]
            case .bodyContains(let w): return ["kind": "body", "words": w]
            case .listHeader(let w): return ["kind": "list_header", "words": w]
            case .account(let a): return ["kind": "account", "account_id": a]
            case .hasAttachment: return ["kind": "has_attachment"]
            case .not(let field, let v):
                let keys: [RuleField: String] = [.from: "from", .to: "to", .cc: "cc", .domain: "domain", .subjectContains: "subject", .bodyContains: "body", .listHeader: "list_header", .account: "account"]
                return ["kind": "not", "field": keys[field] ?? "", "value": v]
            case .fact(let f, let w): return ["kind": "fact", "fact": messageFactKey(fact: f), "wanted": w]
            }
        }
        return ["match": r.matchMode == .all ? "all" : "any", "conditions": cs]
    }
    static func decode(_ json: String) -> RuleSet? {
        guard let data = json.data(using: .utf8), let node = try? JSONDecoder().decode(FixtureFile.RuleNode.self, from: data) else { return nil }
        return node.ruleSet
    }
}

struct D9Hit { let value: String? }

extension FixtureCore {
    // MARK: labels as the tabs

    func labelsLocked() -> [MailLabel] {
        normalizeLabelOrders()
        let tabUnread = Dictionary(uniqueKeysWithValues: countsLocked().tabs.map { ($0.tabId, $0.unread) })
        return orderedTree(labels: labelRows.map { l in
            var row = l
            if l.kind == .builtIn { row.unread = tabUnread[l.id] ?? 0; return row }
            let live = messages.filter { $0.unread && !read.contains($0.id) && !gone($0.id) && (assigned[$0.id] ?? []).contains(l.id) }.count
            // delta 13: the live rows only; the fixture file's fixed `unread` was added here and no message backed it, so the
            // sidebar disagreed with the tab badge and the list. A label holds what was filed under it (by the owner, a rule
            // run, or the fixture model); its rules run when the owner re-runs it, not at load.
            row.unread = UInt32(live)
            return row
        })
    }

    public func pinnedLabels() -> [MailLabel] { lock.withLock { pinnedInTreeOrder(labels: labelsLocked()) } }

    /// The tab strip the UI reads (delta 16): Primary and All (views), the pinned labels in tree order (`inboxTabs`, the contract's
    /// one rule), then the tabs the owner added with a sender, domain or account rule (the older split-tab commands).
    func inboxTabsLocked() -> [InboxTab] {
        let labelTabs = inboxTabs(labels: labelRows)
        let known = Set(labelTabs.map(\.id))
        let rule = splitTabRows.filter { s in s.enabled && !known.contains(s.id) && !labelRows.contains { l in l.id == s.id } }
            .map { s in InboxTab(id: s.id, name: tabNames[s.id] ?? s.name, kind: .label) }
        return labelTabs.map { t in InboxTab(id: t.id, name: t.kind == .view ? t.name : (tabNames[t.id] ?? t.name), kind: t.kind) } + rule
    }

    /// The array order of `labelRows` is the sibling order (every structural change inserts in place): `order` is read from it.
    func normalizeLabelOrders() {
        var next: [String: UInt32] = [:]
        for i in labelRows.indices {
            let key = labelRows[i].parentId ?? ""
            labelRows[i].order = next[key, default: 0]
            next[key, default: 0] += 1
        }
    }

    func setLabel(_ id: String, _ change: (inout MailLabel) -> Void) {
        if let i = labelRows.firstIndex(where: { $0.id == id }) { change(&labelRows[i]) }
    }

    /// The five built-in categories come first, then the file's labels; a corpus' own pinned list is authoritative.
    func installBuiltInLabels() {
        let b = builtinLabels()
        labelRows = b + labelRows.filter { row in !b.contains { $0.id == row.id } }
        if let wanted = pinnedFromCorpus {
            let keys = Set(wanted.map { $0.lowercased() })
            for i in labelRows.indices {
                let l = labelRows[i]
                labelRows[i].pinned = keys.contains(l.id.lowercased()) || keys.contains(l.path.lowercased()) || keys.contains(l.name.lowercased())
            }
        }
        for t in fx.tabs { tabNames[t.id] = t.name }
        defaultPins = Set(labelRows.filter(\.pinned).map(\.id))
        splitTabRows = []
        syncTabsFromLabels()
        seedFlowExampleLocked()   // delta 17: the board's À répondre example
    }

    /// The tab strip is the built-in categories and the pinned labels, in tree order; tabs an owner added with a sender, domain or
    /// account rule (the older split-tab commands) stay after them.
    func syncTabsFromLabels() {
        normalizeLabelOrders()
        let others = splitTabRows.filter { t in !labelRows.contains { $0.id == t.id } }
        var rows: [SplitTab] = []
        for l in orderedTree(labels: labelRows) where l.kind == .builtIn || l.pinned {
            let rule: SplitTabRule = l.kind == .builtIn ? .builtin(name: l.id) : .label(labelId: l.id)
            rows.append(SplitTab(id: l.id, name: tabNames[l.id] ?? l.name, order: 0, rule: rule, enabled: l.pinned))
        }
        splitTabRows = (rows + others).enumerated().map { var t = $0.element; t.order = UInt32($0.offset); return t }
    }

    /// The older split-tab commands changed the strip: pins follow its on/off, and the labels it names take its order.
    func syncLabelsFromTabs(restoring: Bool = false) {
        if restoring { for i in labelRows.indices { labelRows[i].pinned = defaultPins.contains(labelRows[i].id) } }
        for t in splitTabRows where !restoring { setLabel(t.id) { $0.pinned = t.enabled } }
        let roots = labelRows.filter { $0.parentId == nil }
        let desired = splitTabRows.map(\.id).filter { id in roots.contains { $0.id == id } }
        let slots = roots.indices.filter { desired.contains(roots[$0].id) }
        var newRoots = roots
        for (slot, id) in zip(slots, desired) { if let r = roots.first(where: { $0.id == id }) { newRoots[slot] = r } }
        let rebuilt = newRoots.flatMap { subtree(of: $0.id) }
        if rebuilt.count == labelRows.count { labelRows = rebuilt }
        syncTabsFromLabels()
    }

    func labelRow(_ id: String) throws -> MailLabel {
        guard let l = labelRows.first(where: { $0.id == id }) else { throw CoreError.NotFound(kind: "label", id: id) }
        return l
    }

    func applySpecFields(_ id: String, _ spec: LabelSpec) {
        setLabel(id) {
            $0.colour = spec.colour
            $0.description = spec.description
            $0.pinned = spec.pinned
            $0.scope = spec.scope
            if case .account(let a) = spec.scope { $0.accountIds = [a] } else { $0.accountIds = [] }
        }
        // delta 17: `LabelSpec.rules` is the first ENTRY chain only. An update replaces that chain's rules in place and keeps every other chain
        // with its number; an empty rule set removes the first chain only. A number freed is never handed out again while a higher one lives.
        var chains = entryChainsLocked(id)
        if spec.rules.conditions.isEmpty {
            if !chains.isEmpty { chains.removeFirst() }
        } else if chains.isEmpty {
            chains = [LabelChain(chainId: nextChainId(existing: []), rules: spec.rules)]
        } else {
            chains[0] = LabelChain(chainId: chains[0].chainId, rules: spec.rules)
        }
        labelRuleSets[id] = chains.isEmpty ? nil : chains.map(\.rules)
        entryChainIds[id] = chains.isEmpty ? nil : chains.map(\.chainId)
    }

    func checkSpec(_ spec: LabelSpec, excluding id: String?) throws {
        let parent = try spec.parentId.map { try labelRow($0) }
        try validateLabelSpec(spec: spec, parent: parent)
        let n = spec.name.trimmingCharacters(in: .whitespacesAndNewlines)
        if labelRows.contains(where: { $0.id != id && $0.parentId == spec.parentId && $0.name.caseInsensitiveCompare(n) == .orderedSame }) {
            throw CoreError.Invalid(field: "name", reasonWords: "there is already a label named \"\(n)\" there")
        }
        if case .account(let a) = spec.scope, !accountIdsLocked().contains(a) { throw CoreError.NotFound(kind: "account", id: a) }
    }

    func createLabelFullLocked(_ spec: LabelSpec) throws -> CommandOutcome {
        try checkSpec(spec, excluding: nil)
        let id = try createLabel(spec.name, parentId: spec.parentId)
        applySpecFields(id, spec)
        syncTabsFromLabels()
        return CommandOutcome(undoEntryId: nil, createdId: id)
    }

    func updateLabelLocked(_ id: String, _ spec: LabelSpec) throws -> CommandOutcome {
        let cur = try labelRow(id)
        try checkSpec(spec, excluding: id)
        let n = spec.name.trimmingCharacters(in: .whitespacesAndNewlines)
        if n != cur.name { try checkLabelRename(label: cur); try renameLabel(id, n) }
        if spec.parentId != cur.parentId { labelRows = try reorderLabel(labels: labelRows, labelId: id, newParent: spec.parentId, newIndex: UInt32.max) }
        applySpecFields(id, spec)
        syncTabsFromLabels()
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    func reorderLabelLocked(_ id: String, _ parent: String?, _ index: UInt32) throws -> CommandOutcome {
        normalizeLabelOrders()
        labelRows = try reorderLabel(labels: labelRows, labelId: id, newParent: parent, newIndex: index)
        syncTabsFromLabels()
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    func pinLabelLocked(_ id: String, _ on: Bool) throws -> CommandOutcome {
        let cur = try labelRow(id)
        if !on, cur.pinned, labelRows.filter(\.pinned).count <= 1 {
            throw CoreError.Invalid(field: "pinned", reasonWords: "at least one label has to stay pinned: the list always has a first tab")
        }
        setLabel(id) { $0.pinned = on }
        syncTabsFromLabels()
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    func deleteLabelLocked(_ id: String) throws -> CommandOutcome {
        let cur = try labelRow(id)
        try checkLabelDelete(label: cur)
        let gone = Set(subtree(of: id).map(\.id))
        labelRows.removeAll { gone.contains($0.id) }
        for m in assigned.keys { assigned[m]?.removeAll { gone.contains($0) } }
        for m in filedBy.keys { for g in gone { filedBy[m]?[g] = nil } }
        for g in gone { labelRuleSets[g] = nil; rerunState[g] = nil }
        suggestionRows.removeAll { $0.labelId.map(gone.contains) ?? false }
        splitTabRows.removeAll { gone.contains($0.id) }
        syncTabsFromLabels()
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    // MARK: rules, counts and the re-run

    func facts(_ m: FixtureFile.Message) -> MessageFacts {
        let own = Set(ownAddressesLocked().map { $0.lowercased() })
        let inTo = (m.to ?? []).contains { own.contains($0.lowercased()) }
        let inCc = (m.cc ?? []).contains { own.contains($0.lowercased()) }
        return MessageFacts(messageId: m.id, accountId: account[m.id] ?? m.account, fromName: m.sender, fromAddress: m.from, to: [], cc: [], subject: m.subject,
                     body: (m.body ?? [m.preview]).joined(separator: " "), hasAttachment: !(m.attachments ?? []).isEmpty,
                     listHeader: (m.list_unsubscribe ?? []).joined(separator: " "),   // delta 17: MailingList is derived from this header (not empty)
                     unread: m.unread && !read.contains(m.id), receivedAtMs: receivedAt[m.id] ?? 0,
                     // delta 17: the facts of the grid. Waiting = the owner's own Unanswered once `fixture reply` set it, else filed in Needs Reply
                     // (the list filter's `awaitingReply`); VIP = filed in VIPs; Cc me = an own address in Cc and not in To; Replied by me = `fixture reply`.
                     unanswered: unansweredState[m.id] ?? ((category[m.id] ?? m.tab) == "needs-reply"), starred: starred.contains(m.id),
                     vipSender: (category[m.id] ?? m.tab) == "vips", ccMe: inCc && !inTo, repliedByMe: repliedByMe.contains(m.id))
    }
    func corpusFactsLocked() -> [MessageFacts] { messages.filter { !deleted.contains($0.id) }.map(facts) }

    /// delta 20: the ids a rule set matches, newest first, at most `limit` (the same rule as `ruleSetMatches`: a one-chain `firstMatchingChain`)
    public func ruleSetMatches(ruleSet: RuleSet, limit: UInt32) -> [String] {
        lock.withLock {
            let chain = [LabelChain(chainId: 0, rules: ruleSet)]
            return corpusFactsLocked().filter { firstMatchingChain(chains: chain, facts: $0) != nil }
                .sorted { $0.receivedAtMs > $1.receivedAtMs }.prefix(Int(limit)).map(\.messageId)
        }
    }

    public func ruleSetCount(ruleSet: RuleSet) -> RuleSetCount { lock.withLock { countRuleMatches(rules: ruleSet, corpus: corpusFactsLocked()) } }

    func filedUnderLocked(_ labelId: String) -> [String] {
        let isBuiltIn = labelRows.first { $0.id == labelId }?.kind == .builtIn
        return messages.filter { m in
            guard !deleted.contains(m.id) else { return false }
            if isBuiltIn { return (category[m.id] ?? m.tab ?? "primary") == labelId }
            return (assigned[m.id] ?? []).contains(labelId)
        }.map(\.id)
    }
    public func labelRerunCount(labelId: String) -> UInt32 { lock.withLock { UInt32(filedUnderLocked(labelId).count) } }

    /// Files a message under a label with who decided it. A built-in category replaces the message's category (one category at a time).
    func fileMessage(_ id: String, under labelId: String, by: DecidedBy) {
        // delta 11: a label the owner removed from this thread is never filed back by a rule or the model
        let thread = threadOf(id)
        guard mayFileLabel(decidedBy: by, removedByUser: removedByUser.contains("\(thread)|\(labelId)")) else { return }
        if case .owner = by { removedByUser.remove("\(thread)|\(labelId)") }
        if labelRows.first(where: { $0.id == labelId })?.kind == .builtIn {
            category[id] = labelId
        } else if !(assigned[id] ?? []).contains(labelId) {
            assigned[id, default: []].append(labelId)
        }
        filedBy[id, default: [:]][labelId] = by
    }
    func unfileMessage(_ id: String, from labelId: String) {
        assigned[id]?.removeAll { $0 == labelId }
        filedBy[id]?[labelId] = nil
        if category[id] == labelId { category[id] = nil }
    }

    /// SPEC A140: a re-run runs the label's steps over the corpus (`rerunMeaningLocked`, FixtureFiling.swift): step 1 picks, the AI rules on each picked mail
    /// (asked again), step 3 filters the accepted ones; with the AI off step 1 alone files. An owner's own filing stays: a re-run never takes a label away
    /// from a message the owner put it on.
    func rerunLabelLocked(_ id: String) throws -> CommandOutcome {
        _ = try labelRow(id)
        let filed = filedUnderLocked(id)
        let (refiled, unfiled) = rerunMeaningLocked(id)
        rerunState[id] = (filed: filed.count, refiled: refiled, unfiled: unfiled, modelPending: false)
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    // MARK: suggestions and blocks

    func offerAfterJunkLocked(_ ids: [String]) -> String? {
        guard let first = ids.first, let m = fxMessage(first) else { return nil }
        let address = m.from.lowercased()
        if isBlocked(blocks: blockRows, address: address) { return nil }
        suggestionRows.removeAll { !$0.accepted && $0.kind == .blockSender && $0.subject == address }
        let s = Suggestion(id: newId("suggestion"), kind: .blockSender, subject: address, labelId: nil, offeredAtMs: clock.nowMs(), accepted: false)
        suggestionRows.append(s)
        return s.id
    }

    // MARK: the three other suggestion triggers (contracts delta 12; the junk one is above)

    /// One offer of `kind` for `subject`; an offer of the same kind and subject still open is replaced, an accepted one is
    /// never offered again. Returns the suggestion id, or nil when nothing is offered.
    func offerLocked(_ kind: SuggestionKind, subject: String, labelId: String? = nil) -> String? {
        if suggestionRows.contains(where: { $0.accepted && $0.kind == kind && $0.subject == subject && $0.labelId == labelId }) { return nil }
        suggestionRows.removeAll { !$0.accepted && $0.kind == kind && $0.subject == subject && $0.labelId == labelId }
        let s = Suggestion(id: newId("suggestion"), kind: kind, subject: subject, labelId: labelId, offeredAtMs: clock.nowMs(), accepted: false)
        suggestionRows.append(s)
        return s.id
    }

    /// After an archive: a list message (it carries List-Unsubscribe) not already left offers Unsubscribe from its sender.
    func offerAfterArchiveLocked(_ ids: [String]) -> String? {
        guard let id = ids.first(where: { unsubscribeMethodLocked($0) != nil && listsLeft[$0] == nil }), let m = fxMessage(id) else { return nil }
        return offerLocked(.unsubscribe, subject: m.from.lowercased())
    }

    /// After a snooze: the thread snoozed for the third time (or more) and not muted offers Mute thread.
    func offerAfterSnoozeLocked(_ ids: [String]) -> String? {
        var offered: String?
        for thread in Set(ids.map(threadOf)) {
            snoozesOfThread[thread, default: 0] += 1
            let isMuted = messages.contains { threadIdOf($0) == thread && muted.contains($0.id) }
            if snoozesOfThread[thread, default: 0] >= WhooshConstants.SUGGEST_SNOOZE_COUNT, !isMuted { offered = offerLocked(.muteThread, subject: thread) ?? offered }
        }
        return offered
    }

    /// After the owner labels: the same label put on the same sender a second time offers Always label.
    func offerAfterLabelLocked(_ ids: [String], labelId: String) {
        for sender in Set(ids.compactMap { fxMessage($0)?.from.lowercased() }) {
            let key = "\(sender)|\(labelId)"
            labelsOfSender[key, default: 0] += 1
            if labelsOfSender[key, default: 0] >= WhooshConstants.SUGGEST_LABEL_REPEAT { _ = offerLocked(.alwaysLabel, subject: sender, labelId: labelId) }
        }
    }

    /// `fixture reset`: every open suggestion and the counts behind them are forgotten (accepted ones keep their effect).
    public func clearSuggestions() {
        lock.withLock { suggestionRows.removeAll { !$0.accepted }; snoozesOfThread = [:]; labelsOfSender = [:] }
        emit()
    }

    func addBlockLocked(_ target: BlockTarget) {
        let exists = blockRows.contains { $0.target == target }
        if !exists { blockRows.append(Block(target: target, sinceMs: clock.nowMs())) }
    }

    func acceptSuggestionLocked(_ id: String) throws -> CommandOutcome {
        guard let i = suggestionRows.firstIndex(where: { $0.id == id }) else { throw CoreError.NotFound(kind: "suggestion", id: id) }
        switch try suggestionEffect(suggestion: suggestionRows[i]) {
        case .block(let target): addBlockLocked(target)
        case .unsubscribe(let subject):
            unsubscribed.insert(subject)
            // delta 12: the subject is the list's sender; every message of it with a way out leaves the list
            for m in messages where m.from.lowercased() == subject.lowercased() { if let how = unsubscribeMethodLocked(m.id) { listsLeft[m.id] = how } }
        case .muteThread(let threadId): for m in messages where threadIdOf(m) == threadId { muted.insert(m.id) }
        case .labelRule(let address, let labelId):
            _ = try labelRow(labelId)
            var sets = labelRuleSets[labelId] ?? []
            if var firstSet = sets.first, firstSet.matchMode == .any { firstSet.conditions.append(.from(words: address)); sets[0] = firstSet }
            else { sets.append(RuleSet(conditions: [.from(words: address)], matchMode: .any)) }
            labelRuleSets[labelId] = sets
        }
        suggestionRows[i].accepted = true
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    func dismissSuggestionLocked(_ id: String) throws -> CommandOutcome {
        guard let i = suggestionRows.firstIndex(where: { $0.id == id && !$0.accepted }) else { throw CoreError.NotFound(kind: "suggestion", id: id) }
        suggestionRows.remove(at: i)
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    func blockSenderLocked(_ address: String) throws -> CommandOutcome {
        let a = address.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let parts = a.split(separator: "@", omittingEmptySubsequences: false)
        guard parts.count == 2, !parts[0].isEmpty, parts[1].contains(".") else {
            throw CoreError.Invalid(field: "address", reasonWords: "a sender is a full address like name@example.com")
        }
        addBlockLocked(.sender(address: a))
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    func blockDomainLocked(_ domain: String) throws -> CommandOutcome {
        let d = domain.trimmingCharacters(in: .whitespacesAndNewlines).lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "@"))
        guard !d.isEmpty, d.contains("."), !d.contains("@"), !d.contains(" ") else {
            throw CoreError.Invalid(field: "domain", reasonWords: "a domain looks like example.com")
        }
        addBlockLocked(.domain(domain: d))
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    public func suggestions() -> [Suggestion] { lock.withLock { suggestionRows.filter { !$0.accepted }.reversed() } }
    public func blocks() -> [Block] { lock.withLock { blockRows.reversed() } }

    // MARK: the mailbox selector

    func waitingEntriesLocked() -> [OutboxEntry] {
        settleOutboxLocked()
        return entries.filter {
            switch $0.state {
            case .undoable(let until): return clock.nowMs() < until
            case .sent, .cancelled: return false
            default: return true
            }
        }
    }

    func liveMailboxCountsLocked() -> [String: UInt32] {
        let sent = entries.filter { if case .sent = $0.state { return true } else { return false } }.count
        return [mailboxKey(mailbox: .outbox): UInt32(waitingEntriesLocked().count), mailboxKey(mailbox: .sent): UInt32(sent),
                mailboxKey(mailbox: .junk): UInt32(spam.subtracting(deleted).filter(listable).count), mailboxKey(mailbox: .trash): UInt32(deleted.filter(listable).count),
                mailboxKey(mailbox: .archive): UInt32(archived.subtracting(deleted).filter(listable).count)].merging(vipsFigureLocked()) { $1 }
    }

    /// vip-mark: the VIPs figure, the design file's figure plus or minus the unread mail `MarkVip` re-filed into or out of "vips" through `category`
    /// (FixtureCore.swift `.markVip`). No figure and nothing moved: no entry, as before.
    func vipsFigureLocked() -> [String: UInt32] {
        let key = mailboxKey(mailbox: .vips)
        func counted(_ m: FixtureFile.Message) -> Bool { m.unread && !read.contains(m.id) && !gone(m.id) }
        let movedIn = category.filter { $0.value == "vips" }.compactMap { fxMessage($0.key) }.filter { ($0.tab ?? "primary") != "vips" && counted($0) }.count
        let movedOut = category.filter { $0.value != "vips" }.compactMap { fxMessage($0.key) }.filter { $0.tab == "vips" && counted($0) }.count
        let file = fx.mailboxes.first { $0.name.lowercased() == key }?.count
        guard file != nil || movedIn + movedOut > 0 else { return [:] }
        return [key: UInt32(max(0, (file ?? 0) + movedIn - movedOut))]
    }

    /// The live counts with the delta 10 rule applied: Junk, Trash and Archive carry no count at zero (`mailboxCountShown`).
    func shownMailboxCountsLocked() -> [String: UInt32] {
        var out: [String: UInt32] = [:]
        let byKey = Dictionary(uniqueKeysWithValues: [Mailbox.outbox, .sent, .junk, .trash, .archive].map { (mailboxKey(mailbox: $0), $0) })
        for (key, n) in liveMailboxCountsLocked() {
            if let mb = byKey[key], let shown = mailboxCountShown(mailbox: mb, count: n) { out[key] = shown } else if byKey[key] == nil { out[key] = n }
        }
        return out
    }

    func draftRow(id: String, account: String, to: [String], subject: String, body: String, thread: String?, atMs: Int64) -> InboxRow {
        InboxRow(messageId: id, threadId: thread ?? id, accountId: account, senderName: to.isEmpty ? "No recipient" : to.joined(separator: ", "),
                 subject: subject.isEmpty ? "(no subject)" : subject, preview: String(body.prefix(160)), receivedAtMs: atMs, unread: false, pinned: false,
                 labelNames: [], attachmentCount: 0, threadCount: 1)
    }

    func mailboxRowsLocked(_ mailbox: Mailbox, accountIds: [String]?) -> [InboxRow] {
        func inAccount(_ a: String) -> Bool { accountIds.map { $0.contains(a) } ?? true }
        func entryRows(_ list: [OutboxEntry]) -> [InboxRow] {
            list.filter { inAccount($0.draft.fromAccountId) }.sorted { $0.createdAtMs > $1.createdAtMs }.map {
                draftRow(id: $0.draft.id, account: $0.draft.fromAccountId, to: $0.draft.to, subject: $0.draft.subject, body: $0.draft.body, thread: $0.draft.replyToThreadId, atMs: $0.createdAtMs)
            }
        }
        // d15b: a sent copy is listed by Sent (its outbox entry) only, never as a message row of another mailbox
        func messageRows(_ ids: [String]) -> [InboxRow] { ids.filter { id in listable(id) && (fxMessage(id).map { inAccount(account[id] ?? $0.account) } ?? false) }.compactMap(inboxRow) }
        switch mailbox {
        case .outbox: return entryRows(waitingEntriesLocked())
        case .sent: return entryRows(entries.filter { if case .sent = $0.state { return true } else { return false } })
        case .drafts:
            return draftRows.keys.sorted().compactMap { id in draftRows[id].map { (id, $0) } }.filter { inAccount($0.1.account) }.enumerated().map { e in
                draftRow(id: e.element.0, account: e.element.1.account, to: e.element.1.to, subject: e.element.1.subject, body: e.element.1.body, thread: e.element.1.thread, atMs: bornMs + Int64(e.offset))
            }
        case .snoozed: return messageRows(order.filter { snoozed[$0] != nil && !deleted.contains($0) && !archived.contains($0) }.sorted { (snoozed[$0] ?? 0) < (snoozed[$1] ?? 0) })
        case .pinned: return messageRows(order.filter { (pinned[$0] ?? false) && !gone($0) })
        case .vips: return messageRows(order.filter { (category[$0] ?? fxMessage($0)?.tab) == "vips" && !gone($0) })   // vip-mark: a MarkVip re-filing counts
        case .junk: return messageRows(order.filter { spam.contains($0) && !deleted.contains($0) })
        case .trash: return messageRows(order.filter { deleted.contains($0) })
        case .archive: return messageRows(order.filter { archived.contains($0) && !deleted.contains($0) && !spam.contains($0) })
        // the built-in views (delta 10)
        case .starred: return messageRows(order.filter { starred.contains($0) && !deleted.contains($0) && !spam.contains($0) })
        case .muted: return messageRows(order.filter { muted.contains($0) && !deleted.contains($0) && !spam.contains($0) })
        case .allMail: return messageRows(order.filter { !deleted.contains($0) && !spam.contains($0) })
        // delta 18: the threads whose reminder fired, one row each, until the owner clears the reminder
        case .reminders: return messageRows(firedReminderIdsLocked().filter { !deleted.contains($0) && !spam.contains($0) })
        }
    }

    // MARK: state paths (the socket's `core state`)

    private func json(_ v: Any) -> String? {
        guard JSONSerialization.isValidJSONObject([v]) || v is String || v is Bool || v is Int || v is Int64 || v is UInt32 else { return nil }
        guard let d = try? JSONSerialization.data(withJSONObject: v, options: [.fragmentsAllowed, .sortedKeys]) else { return nil }
        return String(data: d, encoding: .utf8)
    }
    private func scopeWords(_ s: LabelScope) -> String { if case .account(let a) = s { return "account:\(a)" } else { return "all" } }
    private func decidedWords(_ d: DecidedBy) -> String {
        switch d { case .rule(let id): return "rule:\(id)"; case .laya: return "laya"; case .jev: return "jev"; case .apple: return "apple"; case .model(let id): return "model:\(id)"; case .owner: return "owner"; case .default: return "default" }
    }
    private func treeJSON(_ rows: [MailLabel], parent: String?) -> [[String: Any]] {
        rows.filter { $0.parentId == parent }.map { l in
            ["id": l.id, "name": l.name, "path": l.path, "depth": Int(l.depth), "order": Int(l.order), "pinned": l.pinned, "kind": l.kind == .builtIn ? "built_in" : "owner",
             "scope": scopeWords(l.scope), "unread": Int(l.unread), "children": treeJSON(rows, parent: l.id)]
        }
    }

    /// nil = not a delta 9 path. A path that is one, with nothing there, answers `D9Hit(value: nil)`.
    func stateDelta9(_ path: String) -> D9Hit? {
        let p = path.components(separatedBy: "/")
        // rules/<ruleSetJson>/count|unread|newest: the JSON may hold slashes, so read the middle whole
        if path.hasPrefix("rules/"), let slash = path.lastIndex(of: "/"), slash > path.index(path.startIndex, offsetBy: "rules/".count - 1) {
            let field = String(path[path.index(after: slash)...])
            guard ["count", "unread", "newest"].contains(field) else { return nil }
            let mid = String(path[path.index(path.startIndex, offsetBy: "rules/".count)..<slash])
            guard let rs = FixtureRuleJSON.decode(mid) else { return D9Hit(value: nil) }
            let c = ruleSetCount(ruleSet: rs)
            switch field {
            case "count": return D9Hit(value: json(Int(c.count)))
            case "unread": return D9Hit(value: json(Int(c.unread)))
            default: return D9Hit(value: c.newestAtMs.flatMap { json($0) })
            }
        }
        return lock.withLock { () -> D9Hit? in
            settleOutboxLocked()
            if let hit = outboxStateD9(p) { return hit }
            if let hit = flowStateLocked(p) { return hit }   // delta 17
            if let hit = repliesStateLocked(p) { return hit }   // delta 20
            if p.count == 2, p[0] == "decision_consent", let k = [DecisionProviderKind.jev, .local, .rules].first(where: { String(describing: $0).lowercased() == p[1] }) { return D9Hit(value: decisionConsentJson(provider: k, given: decisionConsentKey(provider: k).map { decisionConsentGiven.contains($0) } ?? false)) }   // delta 23
            if p.count == 3, p[0] == "attachment", p[2] == "transfer" { return D9Hit(value: attachmentTransferStateLocked(p[1])) }   // delta 23
            if p.count == 3, p[0] == "draft", p[2] == "attachments", p[1] != "current" { return D9Hit(value: outgoingAttachmentsJson(list: refreshedFilesLocked(draftFileRows[p[1]] ?? []))) }   // delta 22
            if let hit = importStateLocked(p) { return hit }   // delta 18
            if let hit = mailActionsStateLocked(p) { return hit }
            switch (p.first, p.count) {
            case ("labels", 2) where p[1] == "tree":
                return D9Hit(value: json(treeJSON(labelsLocked(), parent: nil)))
            case ("inbox", 2) where p[1] == "tabs":   // delta 16: the strip, left to right, with each tab's counts
                let c = countsLocked().tabs
                return D9Hit(value: json(inboxTabsLocked().map { t in
                    ["id": t.id, "name": t.name, "kind": t.kind == .view ? "view" : "label",
                     "unread": Int(c.first { $0.tabId == t.id }?.unread ?? 0), "waiting": Int(c.first { $0.tabId == t.id }?.waiting ?? 0)] as [String: Any] }))
            case ("view", 3) where inboxViewFromId(id: p[1]) != nil:   // view/<primary|all>/name|unread|waiting|count|is_label
                guard let v = inboxViewFromId(id: p[1]) else { return D9Hit(value: nil) }
                let t = countsLocked().tabs.first { $0.tabId == p[1] }
                switch p[2] {
                case "name": return D9Hit(value: json(inboxViewName(view: v)))
                case "unread": return D9Hit(value: json(Int(t?.unread ?? 0)))
                case "waiting": return D9Hit(value: json(Int(t?.waiting ?? 0)))
                case "count": return D9Hit(value: json(order.filter { !gone($0) && belongs($0, toTab: p[1]) }.count))
                case "is_label": return D9Hit(value: json(false))
                default: return D9Hit(value: nil)
                }
            case ("labels", 2) where p[1] == "pinned":
                return D9Hit(value: json(pinnedInTreeOrder(labels: labelsLocked()).map { ["id": $0.id, "name": tabNames[$0.id] ?? $0.name, "unread": Int($0.unread)] as [String: Any] }))
            case ("labels", 3) where p[2] == "rerun_count":
                return D9Hit(value: labelRows.contains { $0.id == p[1] } ? json(filedUnderLocked(p[1]).count) : nil)
            case ("label", 3):
                guard let l = labelsLocked().first(where: { $0.id == p[1] }) else { return D9Hit(value: nil) }
                switch p[2] {
                case "description": return D9Hit(value: json(l.description))
                case "pinned": return D9Hit(value: json(l.pinned))
                case "scope": return D9Hit(value: json(scopeWords(l.scope)))
                case "kind": return D9Hit(value: json(l.kind == .builtIn ? "built_in" : "owner"))
                case "order": return D9Hit(value: json(Int(l.order)))
                case "colour": return D9Hit(value: l.colour.flatMap { json($0) })
                case "path": return D9Hit(value: json(l.path))
                case "name": return D9Hit(value: json(l.name))   // main104 J5.10 read label/needs-reply/name as null: this switch answered every field but the name
                case "parent": return D9Hit(value: l.parentId.flatMap { json($0) })
                case "rules": return D9Hit(value: labelRuleSets[l.id]?.first.flatMap { json(FixtureRuleJSON.encode($0)) })
                case "rule_sets": return D9Hit(value: json(labelRuleSets[l.id]?.count ?? 0))
                default: return D9Hit(value: nil)
                }
            case ("label", 4) where p[2] == "rerun" && p[3] == "preview":   // mirrors the engine's `label/<id>/rerun/preview` (rerunPreviewLocked, FixtureAgents.swift)
                return D9Hit(value: rerunPreviewLocked(p[1]))
            case ("label", 4) where p[2] == "rerun":
                guard let r = rerunState[p[1]] else { return D9Hit(value: nil) }
                switch p[3] {
                case "filed": return D9Hit(value: json(r.filed))
                case "refiled": return D9Hit(value: json(r.refiled))
                case "unfiled": return D9Hit(value: json(r.unfiled))
                case "model_pending": return D9Hit(value: json(r.modelPending))
                default: return D9Hit(value: nil)
                }
            case ("message", 3) where p[2] == "category":
                guard let m = fxMessage(p[1]) else { return D9Hit(value: nil) }
                return D9Hit(value: json(category[p[1]] ?? m.tab ?? "primary"))
            case ("message", 5) where p[2] == "label" && p[4] == "decided_by":
                return D9Hit(value: filedBy[p[1]]?[p[3]].flatMap { json(decidedWords($0)) })
            case ("suggestion", 2) where p[1] == "count":
                return D9Hit(value: json(suggestionRows.filter { !$0.accepted }.count))
            case ("suggestion", 2) where p[1] == "last":
                return D9Hit(value: suggestionRows.last.flatMap { json($0.id) })
            case ("suggestion", 3):
                guard let s = suggestionRows.first(where: { $0.id == p[1] }) else { return D9Hit(value: nil) }
                switch p[2] {
                case "kind": return D9Hit(value: json("\(s.kind)"))
                case "subject": return D9Hit(value: json(s.subject))
                case "accepted": return D9Hit(value: json(s.accepted))
                case "label_id": return D9Hit(value: s.labelId.flatMap { json($0) })
                case "offered_at_ms": return D9Hit(value: json(s.offeredAtMs))
                case "words": return D9Hit(value: json(suggestionWords(kind: s.kind, subject: s.subject)))
                default: return D9Hit(value: nil)
                }
            case ("block", 2) where p[1] == "count": return D9Hit(value: json(blockRows.count))
            case ("block", 2) where p[1] == "list":
                return D9Hit(value: json(blockRows.map { b -> String in if case .sender(let a) = b.target { return "sender:\(a)" } else if case .domain(let d) = b.target { return "domain:\(d)" } else { return "" } }))
            case ("block", 3) where p[2] == "blocked": return D9Hit(value: json(isBlocked(blocks: blockRows, address: p[1])))
            case ("mailbox", 3) where p[2] == "count":
                guard let mb = Self.mailboxNamed(p[1]) else { return D9Hit(value: nil) }
                return D9Hit(value: json(Int(countsLocked().mailboxes[mailboxKey(mailbox: mb)] ?? 0)))
            case ("mailbox", 3) where p[2] == "rows":
                guard let mb = Self.mailboxNamed(p[1]) else { return D9Hit(value: nil) }
                return D9Hit(value: json(mailboxRowsLocked(mb, accountIds: nil).count))
            default: return nil
            }
        }
    }

    static func mailboxNamed(_ key: String) -> Mailbox? {
        [Mailbox.outbox, .vips, .pinned, .snoozed, .drafts, .sent, .junk, .trash, .archive, .starred, .muted, .allMail, .reminders].first { mailboxKey(mailbox: $0) == key }   // every mailbox the rows switch serves (delta 10 views, delta 18 Reminders): a name the list lacked read as nil and ended the state read
    }
}

// MARK: delta 9, groups 8 and 9, and the smart folders' menu

extension FixtureCore {
    /// The owner's own addresses: every account the fixture knows.
    func ownAddressesLocked() -> [String] { fx.accounts.map(\.address) + added.map(\.address) }

    func whenWords(_ id: String) -> String {
        let f = DateFormatter(); f.locale = Locale(identifier: "en_US_POSIX"); f.dateFormat = "EEE d MMM, HH:mm"
        return f.string(from: Date(timeIntervalSince1970: Double(receivedAt[id] ?? clock.nowMs()) / 1000))
    }

    /// Reply all and Forward: the draft is opened through the SaveDraft path's own store (`draftRows`), so it reads like any saved draft.
    func composeFromLocked(_ id: String, forward: Bool) throws -> CommandOutcome {
        guard let m = fxMessage(id) else { throw CoreError.NotFound(kind: "message", id: id) }
        let acc = account[id] ?? m.account
        let own = ownAddressesLocked()
        let ownerAddress = fx.accounts.first { $0.id == acc }?.address ?? added.first { $0.id == acc }?.address ?? ""
        let text = (m.body ?? [m.preview]).joined(separator: "\n\n")
        let draftId = newId("draft")
        if forward {
            let body = forwardBody(fromName: m.sender, fromAddress: m.from, subject: m.subject, whenWords: whenWords(id), attachmentNames: m.attachments ?? [], originalBody: text)
            draftRows[draftId] = (acc, [], [], prefixedSubject(subject: m.subject, prefix: "Fwd"), body, nil)
            draftSavedAt[draftId] = clock.nowMs()
        } else {
            let r = replyAllRecipients(fromAddress: m.from, to: m.to ?? [ownerAddress], cc: m.cc ?? [], ownAddresses: own)
            let body = quotedReplyBody(senderName: m.sender, whenWords: whenWords(id), originalBody: text)
            draftRows[draftId] = (acc, r.to, r.cc, prefixedSubject(subject: m.subject, prefix: "Re"), body, threadIdOf(m))
            draftSavedAt[draftId] = clock.nowMs()
        }
        if forward {   // delta 22: every attachment of the original is on the forward draft, removable
            let files = forwardAttachments(draftId: draftId, original: attachmentFilesLocked(id))
            if !files.isEmpty { draftFileRows[draftId] = files }
        }
        return CommandOutcome(undoEntryId: nil, createdId: draftId)
    }

    func renameSmartFolderLocked(_ id: String, _ name: String) throws -> CommandOutcome {
        guard let i = folderRows.firstIndex(where: { $0.id == id }) else { throw CoreError.NotFound(kind: "smart folder", id: id) }
        let n = name.trimmingCharacters(in: .whitespacesAndNewlines)
        if n.isEmpty { throw CoreError.Invalid(field: "name", reasonWords: "a smart folder needs a name") }
        if folderRows.contains(where: { $0.id != id && $0.name.caseInsensitiveCompare(n) == .orderedSame }) {
            throw CoreError.Invalid(field: "name", reasonWords: "there is already a smart folder named \"\(n)\"")
        }
        folderRows[i].name = n
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    func deleteSmartFolderLocked(_ id: String) throws -> CommandOutcome {
        guard let i = folderRows.firstIndex(where: { $0.id == id }) else { throw CoreError.NotFound(kind: "smart folder", id: id) }
        let parent = folderRows[i].parentId
        for k in folderRows.indices where folderRows[k].parentId == id { folderRows[k].parentId = parent }
        folderRows.remove(at: i)
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    // MARK: group 8, fixture insert doors (the socket verbs that call these are the appshell's: see docs/lanes/contracts.md)

    private func insertMessageLocked(_ id: String, account acc: String, from: String, sender: String, subject: String, body: String, code: String? = nil) throws {
        guard !messages.contains(where: { $0.id == id }) else { throw CoreError.Invalid(field: "id", reasonWords: "there is already a message \(id)") }
        guard accountIdsLocked().contains(acc) else { throw CoreError.NotFound(kind: "account", id: acc) }
        var m = FixtureFile.Message(id: id, sender: sender, from: from, account: acc, subject: subject, preview: String(body.prefix(160)), time: "", unread: true, pinned: false,
                                    attachments: nil, thread_count: 1, trackers_blocked: 0, tab: "primary", body: [body], quoted: nil, date: nil, thread: nil)
        m.login_code = code   // an arrival that names its sign-in code carries it (the engine reads it from the mail)
        messages.insert(m, at: 0)
        order.insert(id, at: 0)
        account[id] = acc
        receivedAt[id] = clock.nowMs()
        pinned[id] = false
        inserted.insert(id)
    }

    /// `fixture deliver`'s insert (delta 13): the same door as the other inserts, reachable from FixtureFiling.swift.
    func insertArrivalLocked(_ id: String, account acc: String, from: String, sender: String, subject: String, body: String, code: String? = nil) throws {
        try insertMessageLocked(id, account: acc, from: from, sender: sender, subject: subject, body: body, code: code)
    }

    /// A new message that arrives already muted: it stays out of the inbox and every count (`message/<id>/muted` is true).
    public func insertMutedMessage(id: String, account: String, from: String, sender: String, subject: String, body: String) throws {
        try lock.withLock { try insertMessageLocked(id, account: account, from: from, sender: sender, subject: subject, body: body); muted.insert(id) }
        emit()
    }

    /// A new inbox message whose archive the server refuses: `Archive` on it throws an action failure carrying `reasonWords`, and the message stays.
    public func insertArchiveRefusedMessage(id: String, account: String, from: String, sender: String, subject: String, body: String, reasonWords: String) throws {
        try lock.withLock { try insertMessageLocked(id, account: account, from: from, sender: sender, subject: subject, body: body); archiveRefused[id] = reasonWords }
        emit()
    }

    /// An outbox entry whose send failed: state `failed`, with the reason in words (state `outbox/<id>/state` and `outbox/<id>/reason`). Returns its id.
    @discardableResult
    public func insertFailedSend(account acc: String? = nil, to: [String], subject: String, body: String, reasonWords: String, code: String? = nil) throws -> String {
        let id: String = try lock.withLock {
            let known = accountIdsLocked()
            let a = acc ?? known.first ?? ""
            guard known.contains(a) else { throw CoreError.NotFound(kind: "account", id: a) }
            let entryId = newId("outbox")
            let draft = Draft(id: newId("draft"), fromAccountId: a, to: to, cc: [], subject: subject, body: body, attachmentIds: [], aiMarks: [], replyToThreadId: nil)
            let failure = ActionFailed(accountName: accountName(a), reasonWords: reasonWords, retryable: true, code: code)
            entries.append(OutboxEntry(id: entryId, draft: draft, state: .failed(failure: failure, attempts: 1), createdAtMs: clock.nowMs()))
            return entryId
        }
        emit()
        return id
    }

    func outboxStateD9(_ p: [String]) -> D9Hit? {
        guard p.first == "outbox", p.count == 3, let e = entries.first(where: { $0.id == p[1] }) else { return nil }
        if p[2] == "attachments" { return D9Hit(value: outgoingAttachmentsJson(list: draftFileRows[e.id] ?? [])) }   // delta 22: what the queued message carries
        switch (p[2], e.state) {
        case ("state", .failed): return D9Hit(value: "\"failed\"")
        case ("reason", .failed(let f, _)): return D9Hit(value: try? String(data: JSONSerialization.data(withJSONObject: f.reasonWords, options: [.fragmentsAllowed]), encoding: .utf8))
        case ("attempts", .failed(_, let n)): return D9Hit(value: "\(n)")
        case ("code", .failed(let f, _)): return D9Hit(value: f.code.map { "\"\($0)\"" } ?? "null")
        default: return nil
        }
    }
}
