import Foundation

/// Contracts delta 13 and 17, SPEC A140: the Labels page under --fixture-core files mail through the label's three steps, each skipped when switched off.
///   Step 1, Entry rules, PICKS mails: the chains are alternatives, the conditions of a chain follow its mode; with no chain, or the step off, every mail passes.
///   Step 2, AI sorting, is asked only about the mails step 1 picked and rules on each (accept or not) from the label's description. The fixture model is
///   deterministic: a label's meaning is the words of five letters or more in its description; a message fits a label when its subject, sender or body holds
///   at least one of them, the label with the most such words wins, ties go to the label listed first. Not a model; it lets a driven check see a ruling.
///   Step 3, Exit filter, applies its fixed rule to the mails the AI accepted (to the mails step 1 picked when the AI is off); a mail it rejects goes to
///   the label's "others go to" target.
/// Between labels: the labels whose AI is off and whose step 1 picked the mail win first, in sidebar order (step 1 alone files, then step 3); else the AI
/// chooses among the AI-on labels whose step 1 the mail passed. A label with no entry chain and the AI off files nothing.
extension FixtureCore {
    /// What step 1 of a label says about one message.
    enum EntryRead: Equatable {
        case everyMail          // step off or no chain: every mail passes, nothing is "picked"
        case picked(UInt32)     // the chain (lowest number) that matched
        case notPicked
    }

    /// Step 1 for one label and one message. Chains with no condition match nothing (the delta 9 rule) and are not chains.
    func entryReadLocked(_ l: MailLabel, _ f: MessageFacts) -> EntryRead {
        let chains = l.entryEnabled ? entryChainsLocked(l.id).filter { !$0.rules.conditions.isEmpty } : []
        if chains.isEmpty { return .everyMail }
        if let c = firstMatchingChain(chains: chains, facts: f) { return .picked(c) }
        return .notPicked
    }

    /// Step 2 is on for this label: its AI card is on and it has a description (the AI rules from the description; with none the engine does not ask it,
    /// `ai_wanted` of the re-run preview, and step 1 alone files). A built-in category (Needs Reply) is described when the file says so, as the example is.
    func aiRulesIn(_ l: MailLabel) -> Bool {
        l.aiEnabled && !l.description.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    /// The label whose AI is off and whose step 1 picked this message (the first in sidebar order), with the chain, or nil. Step 1 alone files it.
    func entryOnlyHitLocked(_ f: MessageFacts) -> (label: String, chain: UInt32)? {
        for l in labelRows where !aiRulesIn(l) {
            if case .picked(let c) = entryReadLocked(l, f) { return (l.id, c) }
        }
        return nil
    }

    /// The AI-on labels the AI may choose among for this message: step 1 off, no chain, or a chain that picked it. Each with the picking chain (nil = every mail passes).
    func aiCandidatesLocked(_ f: MessageFacts) -> [(label: MailLabel, chain: UInt32?)] {
        var out: [(label: MailLabel, chain: UInt32?)] = []
        for l in labelRows where aiRulesIn(l) && Self.visible(l, to: f.accountId) {
            switch entryReadLocked(l, f) {
            case .notPicked: continue
            case .everyMail: out.append((l, nil))
            case .picked(let c): out.append((l, c))
            }
        }
        return out
    }

    /// The described label the fixture model picks among `among` (label ids; nil = every AI-on label), or nil when no meaning fits.
    func meaningLabelLocked(_ f: MessageFacts, among: Set<String>? = nil) -> String? {
        let text = Set("\(f.subject) \(f.fromName) \(f.fromAddress) \(f.body)".lowercased().split(whereSeparator: { !$0.isLetter && !$0.isNumber }).map(String.init))
        var best: (id: String, score: Int)?
        for l in labelRows where aiRulesIn(l) && Self.visible(l, to: f.accountId) && (among?.contains(l.id) ?? true) {   // delta 17: an off AI card is not offered
            let meaning = Set(l.description.lowercased().split(whereSeparator: { !$0.isLetter }).map(String.init).filter { $0.count >= 5 })
            let score = meaning.intersection(text).count
            if score > 0, score > (best?.score ?? 0) { best = (l.id, score) }
        }
        return best?.id
    }

    /// An account's label is offered only on that account's mail (as the decision crate's `LabelNode.visible_to`).
    static func visible(_ l: MailLabel, to accountId: String) -> Bool {
        if case .account(let id) = l.scope { return id == accountId }
        return true
    }

    /// Files one message as it arrives, through the steps in the order above. Called with the lock held.
    func fileOnArrivalLocked(_ id: String) {
        guard let m = fxMessage(id) else { return }
        fileThroughStepsLocked(id, facts(m))
    }

    /// The order between labels: step 1 alone for the AI-off labels first, else the AI's ruling over the picked mails.
    func fileThroughStepsLocked(_ id: String, _ f: MessageFacts) {
        if let hit = entryOnlyHitLocked(f) {
            fileByModelLocked(id, hit.label, f, byEntry: true)
            return
        }
        aiRulesLocked(id, f)
    }

    /// Step 2 for one message: the AI is asked about the labels whose step 1 the message passed; the accepted one goes through step 3. A label step 1 picked
    /// and the AI did not accept keeps a "ruled, not accepted" mark (an entry card record with no filing), so the counts know the AI has answered.
    func aiRulesLocked(_ id: String, _ f: MessageFacts) {
        let candidates = aiCandidatesLocked(f)
        let winner = meaningLabelLocked(f, among: Set(candidates.map { $0.label.id }))
        for c in candidates where c.label.id != winner {
            if let chain = c.chain, filedBy[id]?[c.label.id] == nil { flowCard[id, default: [:]][c.label.id] = (.entry, chain) }
        }
        if let w = winner { fileByModelLocked(id, w, f) }
    }

    /// A mail `labelId` accepted (the AI's yes, or step 1 alone when the AI is off, `byEntry`): with the exit filter off it files; with it on, the exit
    /// chains keep it or send it to the label's fallback (a label id files it there with the fallback card; Primary and Junk leave it under no label,
    /// the rejection is recorded).
    func fileByModelLocked(_ id: String, _ labelId: String, _ f: MessageFacts, byEntry: Bool = false) {
        guard let label = labelRows.first(where: { $0.id == labelId }) else { return }
        let by: DecidedBy = byEntry ? .rule(ruleId: "rule-\(labelId)") : localDecider(.laya)   // SPEC A133: the on-device model in use is named
        guard label.exitEnabled else {
            var chain: UInt32?
            if byEntry, case .picked(let c) = entryReadLocked(label, f) { chain = c }
            let plain: WhyCard = byEntry ? .entry : .ai
            fileMessage(id, under: labelId, by: by); flowCard[id, default: [:]][labelId] = (plain, chain); flowRejected[id] = nil
            return
        }
        switch exitVerdict(chains: exitChainsLocked(labelId), facts: f) {
        case .kept(let chain):
            fileMessage(id, under: labelId, by: by); flowCard[id, default: [:]][labelId] = (.exit, chain); flowRejected[id] = nil
        case .rejected:
            unfileMessage(id, from: labelId)
            let to = label.exitFallback ?? "primary"
            flowRejected[id] = (ai: labelId, to: to, chain: nil)
            if labelRows.contains(where: { $0.id == to }) { fileMessage(id, under: to, by: localDecider(.laya)); flowCard[id, default: [:]][to] = (.fallback, nil) }
        }
    }

    /// A re-run of one label (the steps over the whole corpus): a message the label's steps accept is filed (the exit filter asked again for one already
    /// there), a message they no longer accept leaves (an owner's own filing stays). Asks the AI again about every picked mail (the owner pressed Re-run).
    /// Returns (filed, unfiled).
    func rerunMeaningLocked(_ labelId: String) -> (Int, Int) {
        guard let label = labelRows.first(where: { $0.id == labelId }) else { return (0, 0) }
        let aiOn = aiRulesIn(label)
        var filed = 0, unfiled = 0
        for m in messages where !deleted.contains(m.id) {
            let f = facts(m)
            var accepted = false
            if entryOnlyHitLocked(f) != nil {
                if !aiOn, entryOnlyHitLocked(f)?.label == labelId { accepted = true }   // step 1 alone files it; an earlier label with its AI off wins
            } else if aiOn {
                let candidates = aiCandidatesLocked(f)
                accepted = meaningLabelLocked(f, among: Set(candidates.map { $0.label.id })) == labelId
                if !accepted, let c = candidates.first(where: { $0.label.id == labelId })?.chain, filedBy[m.id]?[labelId] == nil { flowCard[m.id, default: [:]][labelId] = (.entry, c) }   // ruled, not accepted
            }
            let nowFiled = (filedBy[m.id]?[labelId] != nil)
            if accepted, !nowFiled {
                fileByModelLocked(m.id, labelId, f, byEntry: !aiOn)
                if (filedBy[m.id]?[labelId] != nil) { filed += 1 }
            } else if accepted, nowFiled, label.exitEnabled {   // delta 17: a re-run asks the exit filter again
                fileByModelLocked(m.id, labelId, f, byEntry: !aiOn)
                if !(filedBy[m.id]?[labelId] != nil) { unfiled += 1 }
            } else if !accepted, nowFiled {
                if case .owner? = filedBy[m.id]?[labelId] { continue }
                if !aiOn, case .everyMail = entryReadLocked(label, f) { continue }   // step 1 cannot pick (off or no chain): a re-run does not strip what is filed
                unfileMessage(m.id, from: labelId); unfiled += 1
            }
        }
        return (filed, unfiled)
    }

    /// `fixture deliver`: the arrival becomes an inbox row, filed through the label rules and the fixture model at arrival.
    /// Returns the labels it carries after filing.
    public func deliverArrival(id: String, account acc: String, from: String, sender: String, subject: String, body: String, code: String? = nil) throws -> [String] {
        let labels: [String] = try lock.withLock {
            try insertArrivalLocked(id, account: acc, from: from, sender: sender, subject: subject, body: body, code: code)
            // delta 18: new mail in a muted thread arrives read and out of the inbox (`arrivalInThread`); the fixture reads a thread by its subject
            if mutedThreadMatchesLocked(subject: subject, except: id) {
                let t = arrivalInThread(threadMuted: true)
                if t.markRead { read.insert(id) }
                if !t.inInbox { muted.insert(id) }
            }
            fileOnArrivalLocked(id)
            #if os(iOS)
            pending.append((.inbox, .arrivalNotice(messageId: id)))   // the engine's notice of a new mail: the phone's test copy routes it to its recording poster (PushClientRig); not on the Mac, whose notch is fed by `fixture deliver` itself
            #endif
            writeArrivalDraftLocked(id)   // gap-replies: the automatic draft of an arrival (FixtureCoreArrivalDraft.swift); nothing is sent
            return assigned[id] ?? []
        }
        emit()
        return labels
    }
}

// MARK: smart folders (contracts delta 14)

extension FixtureCore {
    /// The message ids a rule in words finds, newest first, over the mail still in the store (deleted rows excluded).
    /// One reading for the folder's list and counts, the editor's count and the search screen:
    /// a wholly quoted rule is an exact phrase, matched by the search screen's own `SearchEngine.exactRanges` over subject and body
    /// ("0.84 EUR" is not inside "110.84 EUR" or "0.84 USD"); any other words go to the fixture search, text and meaning.
    /// Kept per words until the mail set changes (rows in or deleted), so a count read again and again does not search again.
    /// Called with the lock held.
    func ruleMatchIdsLocked(_ words: String) -> [String] {
        let q = words.trimmingCharacters(in: .whitespacesAndNewlines)
        let stamp = "\(order.count)|\(deleted.count)|\(order.first ?? "")|\(order.last ?? "")"
        if let memo = ruleMatchMemo[q], memo.stamp == stamp { return memo.ids }
        let rows: [FixtureSearchBook.Row] = order.compactMap { id in
            guard !deleted.contains(id), let m = fxMessage(id) else { return nil }
            return FixtureSearchBook.Row(id: id, subject: m.subject, body: (m.body ?? [m.preview]).joined(separator: " "), sender: m.sender, attachments: m.attachments ?? [], receivedMs: receivedAt[id] ?? 0)
        }
        let inner = q.count > 2 && q.hasPrefix("\"") && q.hasSuffix("\"") ? String(q.dropFirst().dropLast()).trimmingCharacters(in: .whitespacesAndNewlines) : ""
        let ids: [String]
        if !inner.isEmpty, !inner.contains("\"") {
            ids = rows.filter { !SearchEngine.exactRanges(inner, in: $0.subject + " · " + $0.body).isEmpty }.map(\.id)
        } else {
            // A rule match is a READ: it runs on a scratch book, so the app's search book (its last search) stays as it was. main81: this read wrote
            // `searchBook.last`, the state digest after a reset then differed from the launch digest, and 83 cases of par-macshell were not judged.
            ids = ((try? FixtureSearchBook().run(q, rows: rows, meaning: [], attachmentHits: [])) ?? []).map(\.messageId)
        }
        let sorted = ids.sorted { (receivedAt[$0] ?? 0) > (receivedAt[$1] ?? 0) }
        if ruleMatchMemo.count > 32 { ruleMatchMemo = [:] }
        ruleMatchMemo[q] = (stamp, sorted)
        return sorted
    }

    /// What a rule in words lists and counts: its matches minus mail that left the inbox (archived, deleted, snoozed, muted, spam)
    /// and minus sent copies. The saved folder's list, `smart_folder/<id>/count|unread` and the editor's `ruleMatchCount` all read this.
    func ruleLiveIdsLocked(_ words: String) -> [String] {
        ruleMatchIdsLocked(words).filter { !gone($0) && listable($0) }
    }

    /// The message ids a smart folder's saved words find, newest first (`ruleMatchIdsLocked`). Unknown folder = none. Called with the lock held.
    func smartFolderIdsLocked(_ folderId: String) -> [String] {
        guard let f = folderRows.first(where: { $0.id == folderId }) else { return [] }
        return ruleMatchIdsLocked(f.ruleWords)
    }
}
