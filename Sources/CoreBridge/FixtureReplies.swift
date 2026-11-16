import Foundation

// Contracts delta 20 under --fixture-core: AI replies (docs/lanes/replies-contract.md; docs/JOURNEYS.md J10; the approved screens docs/design/boards/ai-replies-r7).
// The rules are the contract's pure functions (checkReplyScopeLabels, validateReplyRule, checkSetReplyCard, replyEntryStep, replyAiStep, replyExitStep,
// countReplySteps, applyRetrievalChange, checkReplyPromptText, replyDraftAfterChange ...): the Rust core calls the same ones. The JSON of every read is the
// contract's own encoders (replyUseJson, replyFilterJson, ...): a key cannot be spelt differently here. This file keeps the state and the sample data.
// No model runs: a draft is a scripted text, the "model" says yes to a message that waits for a reply (the Unanswered fact), retrieval finds canned excerpts.
// The figures of the auto pane (128 / 31 / 12 / 9 / 9) come from 128 scripted passes, derived by countReplySteps: the 16-message corpus cannot hold them.

final class FixtureReplyBook: @unchecked Sendable {
    static let day: Int64 = 86_400_000
    var uses: [ReplyUseKind: ReplyUse] = [
        .auto: ReplyUse(kind: .auto, enabled: true, toneId: "voice", matchThread: true, promptId: "replies"),
        .onDemand: ReplyUse(kind: .onDemand, enabled: true, toneId: "voice", matchThread: true, promptId: "replies")]
    var filter: ReplyFilter
    var onDemand = defaultOnDemandOptions()
    var passes: [ReplyPass] = []
    var mailFolders: [ReplySource] = []
    var localFolders: [ReplySource] = []
    var calendars: [ReplyCalendar] = [
        ReplyCalendar(calendarId: "cal-work", name: "Travail", accountLabel: "assem@nexus-example.com", colour: WhooshCalendarColour.fixtureWork, enabled: true),
        ReplyCalendar(calendarId: "cal-personal", name: "Personnel", accountLabel: "assem@icloud.com", colour: WhooshCalendarColour.fixturePersonal, enabled: false)]
    var retrieval: RetrievalSettings
    var voice: ReplyVoice
    var analysis: ReplyVoiceAnalysis = .idle
    var tones: [ReplyTone] = [ReplyTone(id: "tone-formal", name: "Formal", text: "Formal address, full sentences, a polite closing."),
                              ReplyTone(id: "tone-brief", name: "Brief", text: "Two sentences at most, no closing formula.")]
    var prompts: [ReplyPrompt]
    var notes: [FixedNote] = [FixedNote(id: "note-1", title: "Prices", text: "Never quote a price without approval."),
                              FixedNote(id: "note-2", title: "Availability", text: "Meetings on Tuesday and Thursday afternoons.")]
    var drafts: [ReplyDraft] = []
    var tries: [String: ReplyTryReport] = [:]
    var goneOwnerChoice: [String: Bool] = [:]   // delta 24: source key -> the owner's switch while the folder is gone (a key here = gone)
    var calendarEvents: [CalendarEvent] = []   // delta 21: what the shell pushed for the enabled calendars (SyncReplyCalendars)
    var counter = 0

    /// `accounts`: the first two account ids of the fixture file; `scopeLabels`: the label ids the board ticks (À répondre, Clients).
    init(nowMs: Int64, accounts: [String], scopeLabels: [String], openThread: (thread: String, message: String, account: String)?) {
        firstAccountId = accounts.first ?? ""
        // the filter of the page "Quels messages": the two ticked labels, entry rule "not a mailing list", exit rule "not already answered by me"
        var f = defaultReplyFilter()
        f.scope = .labels(labelIds: scopeLabels)
        f.entryRule = RuleSet(conditions: setFact(conditions: [], fact: .mailingList, state: .no), matchMode: .all)
        f.exitRule = RuleSet(conditions: setFact(conditions: [], fact: .repliedByMe, state: .no), matchMode: .all)
        f.aiDescription = "Emails that wait for a reply from me: a question, a request, a decision that someone is waiting for."   // the real engine's seed (0013_ai_replies.sql:92); the pane shows it in the app's language (hid-ai round 6)
        filter = f
        // the Recherche pane
        // hid-ai round 17 ("don't mix languages", main107 d58d8ad5): the seed is English, the real engine's own defaults (reply_ai_logic.rs:264 and :268), not French
        retrieval = defaultRetrievalSettings()
        // the Voix pane: analysed 3 November over 100 sent messages
        voice = ReplyVoice(toneText: "Short sentences. I go straight to the answer, the detail comes after.\nFormal with clients, informal with the team. No long opening formula.\nI end with “Best regards” and my first name.",
                           sampleSize: 100, analysedAtMs: 1_793_696_400_000, samePersonExamples: 3)
        // the Prompt pane: version 3, current; version 1 is the real engine's shipped prompt (hid-ai round 18: restoring it shows the default in the app's language)
        prompts = [
            ReplyPrompt(promptId: "replies", version: 1, text: defaultReplyPromptText(), createdAtMs: nowMs - 20 * Self.day, current: false),
            ReplyPrompt(promptId: "replies", version: 2, text: "Reply to {{message}} using {{excerpts}}.", createdAtMs: nowMs - 9 * Self.day, current: false),
            ReplyPrompt(promptId: "replies", version: 3, text: "## What you know\nYou are replying to {{message}}. You have read {{thread}} and {{excerpts}}. My availability is in {{calendar}}.\nIf two excerpts disagree, rely on the most recent one and tell me.\n## How to write\nWrite like {{tone}}. Follow {{instruction}} when there is one.\nAnswer the question first. Never promise a price or a deadline.\n// One sentence per rule. This line is a note: the model does not read it.", createdAtMs: nowMs - 1_000, current: true)]
        // the Sources pane: the board's tiles. First account 4 of 6 folders (18 096 messages), second 2 of 5.
        func mail(_ account: String, _ folder: String, _ on: Bool, _ n: UInt64, _ p: ReplySyncProgress) -> ReplySource {
            ReplySource(source: .mailFolder(accountId: account, folder: folder), enabled: on, itemCount: n, progress: p)
        }
        if let a = accounts.first {
            mailFolders += [mail(a, "INBOX", true, 9_204, .upToDate), mail(a, "Sent", true, 4_812, .upToDate), mail(a, "Clients", true, 2_960, .syncing(percent: 62)),
                            mail(a, "Clients/Lorvan Hotels", true, 1_120, .syncing(percent: 62)), mail(a, "Clients/Oranta", false, 1_840, .notStarted), mail(a, "Newsletters", false, 6_310, .notStarted)]
        }
        if accounts.count > 1 {
            let b = accounts[1]
            mailFolders += [mail(b, "INBOX", true, 1_204, .upToDate), mail(b, "Sent", true, 640, .upToDate), mail(b, "Family", false, 388, .notStarted),
                            mail(b, "Invoices", false, 512, .notStarted), mail(b, "Archive", false, 2_906, .notStarted)]
        }
        localFolders = [ReplySource(source: .localFolder(path: "~/Documents/Contrats"), enabled: true, itemCount: 214, progress: .upToDate),
                        ReplySource(source: .localFolder(path: "~/Documents/Propositions"), enabled: true, itemCount: 86, progress: .upToDate)]
        // 128 passes in the last 6 days: 97 stopped by the entry card, 19 by the AI, 3 by the exit card, 9 drafts
        func pass(_ i: Int, _ e: ReplyStepResult, _ a: ReplyStepResult, _ x: ReplyStepResult, draft: Bool) -> ReplyPass {
            ReplyPass(messageId: "fx-pass-\(i)", receivedAtMs: nowMs - Int64(i % 6 + 1) * Self.day + Int64(i), entry: e, ai: a, exit: x, draftId: draft ? "fx-draft-\(i)" : nil)
        }
        var i = 0
        for _ in 0..<97 { passes.append(pass(i, .stopped, .notReached, .notReached, draft: false)); i += 1 }
        for _ in 0..<19 { passes.append(pass(i, .passed, .stopped, .notReached, draft: false)); i += 1 }
        for _ in 0..<3 { passes.append(pass(i, .passed, .passed, .stopped, draft: false)); i += 1 }
        for _ in 0..<9 { passes.append(pass(i, .passed, .passed, .passed, draft: true)); i += 1 }
        // the draft the thread view shows (J10.8): written alone, proposed, nothing sent. Seeded on msg-16 (its own thread: the
        // design fixture's last row, a question to answer), which no older check opens with R (lane reply-thread, lead ruling).
        if openThread != nil {
            drafts = [ReplyDraft(id: "reply-draft-seed", threadId: "msg-16", messageId: "msg-16",
                                 text: "Bonjour,\n\nMerci pour votre message. Je vous envoie la proposition mise à jour avant jeudi.\n\nBien à vous,\nAssem",
                                 madeBy: .auto, excerptIds: [chunkExcerptId(chunkId: 1), chunkExcerptId(chunkId: 2)], instruction: nil, state: .proposed, revision: 0, createdAtMs: nowMs - 3_600_000, updatedAtMs: nowMs - 3_600_000)]
        }
    }

    func isLive(_ d: ReplyDraft) -> Bool { d.state == .proposed || d.state == .edited }
    let firstAccountId: String
    /// the two sample chunks (ids 1 and 2: a chunk id is its own key, decimal in `excerptIds`)
    func chunk(_ id: Int64) -> Chunk? {
        id == 1 ? Chunk(id: 1, messageId: "fx-msg-1", part: .body, chunkNo: 0, receivedAtMs: 1_792_000_000_000, sender: "Laura Meier", folder: "Clients", subject: "Devis Q4",
                        text: "Le devis Q4 est de 4 200 EUR, valable jusqu’au 30 novembre.", embeddedAtMs: 1_792_000_100_000)
        : id == 2 ? Chunk(id: 2, messageId: "fx-msg-2", part: .attachment(attachmentId: "att-1"), chunkNo: 0, receivedAtMs: 1_790_000_000_000, sender: "", folder: "Propositions", subject: "Proposition Q4",
                          text: "La proposition couvre trois ateliers et une restitution.", embeddedAtMs: nil)
        : nil
    }
    /// delta 23: the two sample excerpts that are not mail (a file of a source folder, a calendar event); ids 1 and 2, excerpt ids "s1" and "s2"
    func sourceChunk(_ id: Int64) -> SourceChunk? {
        id == 1 ? SourceChunk(id: 1, origin: .file(folderPath: "~/Documents/Propositions", itemPath: "2026/Proposition Q4.pdf"), chunkNo: 0, dateMs: 1_790_000_000_000,
                              title: "Proposition Q4.pdf", text: "La proposition couvre trois ateliers et une restitution.", embeddedAtMs: 1_790_000_100_000)
        : id == 2 ? SourceChunk(id: 2, origin: .event(calendarId: "cal-work", eventId: "ev-1"), chunkNo: 0, dateMs: 1_792_300_000_000, title: "Point client",
                                text: "Point client mardi 14 h avec Laura Meier.", embeddedAtMs: nil)
        : nil
    }
    /// folders' progress as the crawl and the indexer would report it (stored, bodies, indexed), from each source's scripted progress
    func folderProgress() -> [FolderProgress] {
        mailFolders.compactMap { s -> FolderProgress? in
            guard case .mailFolder(let a, let f) = s.source else { return nil }
            switch s.progress {
            case .upToDate: return FolderProgress(accountId: a, folder: f, messages: s.itemCount, bodies: s.itemCount, indexed: s.itemCount)
            case .syncing(let pct): return FolderProgress(accountId: a, folder: f, messages: s.itemCount, bodies: s.itemCount, indexed: s.itemCount * UInt64(pct) / 100)
            default: return FolderProgress(accountId: a, folder: f, messages: s.itemCount, bodies: 0, indexed: 0)
            }
        }
    }
    func sample(_ id: String) -> ReplyExcerpt {
        id == "chunk-1"
            ? ReplyExcerpt(chunkId: chunkExcerptId(chunkId: 1), source: .mailFolder(accountId: firstAccountId, folder: "Clients"),
                           dateMs: 1_792_000_000_000, sender: "Laura Meier", folder: "Clients", subject: "Devis Q4", text: "Le devis Q4 est de 4 200 EUR, valable jusqu’au 30 novembre.")
            : ReplyExcerpt(chunkId: chunkExcerptId(chunkId: 2), source: .localFolder(path: "~/Documents/Propositions"), dateMs: 1_790_000_000_000, sender: nil, folder: "Propositions", subject: "Proposition Q4",
                           text: "La proposition couvre trois ateliers et une restitution.")
    }
}

extension FixtureCore {
    // MARK: the book

    func replyBookLocked() -> FixtureReplyBook {
        if let b = replyBookStore { return b }
        let thread = fx.open_thread.flatMap { t -> (thread: String, message: String, account: String)? in
            guard let m = (t.messages ?? []).compactMap(\.id).last else { return nil }
            return (t.id, m, t.account)
        }
        let scope = ["needs-reply"] + (labelRows.first { $0.name == "Clients" }.map { [$0.id] } ?? [])
        let b = FixtureReplyBook(nowMs: clock.nowMs(), accounts: Array(fx.accounts.prefix(2).map(\.id)), scopeLabels: scope, openThread: thread)
        if newInstall { b.makeNewInstall(accounts: fx.accounts.map(\.id)) }
        replyBookStore = b
        return b
    }

    private func replyEvent(_ kind: StreamKind, _ e: CoreEvent) { pending.append((kind, e)) }
    private func replyOutcome(_ created: String? = nil) -> CommandOutcome { CommandOutcome(undoEntryId: nil, createdId: created) }

    // MARK: the commands (called inside the lock by `run`)

    func repliesCommandLocked(_ command: Command) throws -> CommandOutcome {
        let b = replyBookLocked()
        let now = clock.nowMs()
        switch command {
        case .setReplyUseEnabled(let kind, let enabled): b.uses[kind]?.enabled = enabled
        case .setReplyUseTone(let kind, let toneId):
            try checkToneChoice(toneId: toneId, tones: b.tones)
            b.uses[kind]?.toneId = toneId
        case .setReplyUseMatchThread(let kind, let on): b.uses[kind]?.matchThread = on
        case .setReplyUsePrompt(let kind, let promptId):
            guard b.prompts.contains(where: { $0.promptId == promptId }) else { throw CoreError.NotFound(kind: "prompt", id: promptId) }
            b.uses[kind]?.promptId = promptId
        case .setReplyFilterScope(let scope):
            try checkReplyScopeLabels(scope: scope, labels: labelRows)
            b.filter.scope = scope
        case .setReplyFilterCard(let card, let enabled):
            try checkSetReplyCard(filter: b.filter, card: card, enabled: enabled)
            switch card { case .entry: b.filter.entryEnabled = enabled; case .ai: b.filter.aiEnabled = enabled; case .exit: b.filter.exitEnabled = enabled }
        case .setReplyFilterEntryRule(let rule):
            try validateReplyRule(rule: rule)
            b.filter.entryRule = rule
        case .setReplyFilterExitRule(let rule):
            try validateReplyRule(rule: rule)
            b.filter.exitRule = rule
        case .setReplyFilterAiReads(let reads):
            try checkReplyAiReads(reads: reads)
            b.filter.aiReads = reads
        case .setReplyFilterAiDescription(let description):
            try checkReplyAiDescription(aiEnabled: b.filter.aiEnabled, description: description)
            b.filter.aiDescription = description.trimmingCharacters(in: .whitespacesAndNewlines)
        case .setOnDemandOption(let option, let enabled):
            switch option { case .instructionBefore: b.onDemand.instructionBefore = enabled; case .changeAfter: b.onDemand.changeAfter = enabled }
        case .setRetrievalSetting(let change): b.retrieval = try applyRetrievalChange(settings: b.retrieval, change: change)
        case .setReplySourceEnabled(let source, let enabled): try setReplySourceEnabledLocked(b, source, enabled)
        case .addReplyLocalFolder(let path):
            let p = try checkLocalFolderPath(path: path)
            let ref = ReplySourceRef.localFolder(path: p)
            if let i = b.localFolders.firstIndex(where: { $0.source == ref }) { b.localFolders[i].enabled = true; return replyOutcome(replySourceKey(source: ref)) }
            let added = ReplySource(source: ref, enabled: true, itemCount: 0, progress: .syncing(percent: 0))
            b.localFolders.append(added)
            replyEvent(.syncStatus, .replySourceProgress(source: ref, itemCount: 0, progress: added.progress))
            return replyOutcome(replySourceKey(source: ref))
        case .removeReplyLocalFolder(let path):
            let ref = ReplySourceRef.localFolder(path: try checkLocalFolderPath(path: path))
            guard b.localFolders.contains(where: { $0.source == ref }) else { throw CoreError.NotFound(kind: "folder", id: path) }
            b.localFolders.removeAll { $0.source == ref }
        case .runVoiceAnalysis(let sampleSize):
            _ = try checkVoiceSampleSize(n: sampleSize)
            if case .running = b.analysis { throw CoreError.Invalid(field: "voice_analysis", reasonWords: "an analysis is already running") }
            b.voice.sampleSize = sampleSize
            b.voice.analysedAtMs = now
            if b.voice.toneText.trimmingCharacters(in: .whitespaces).isEmpty { b.voice.toneText = "Short sentences. I go straight to the answer." }
        case .setVoiceText(let text): b.voice.toneText = text
        case .setVoiceSamePersonExamples(let count): b.voice.samePersonExamples = try checkSamePersonExamples(n: count)
        case .saveReplyTone(let tone):
            try checkReplyTone(tone: tone, existing: b.tones)
            var t = tone
            if t.id.isEmpty { t.id = newId("tone") }
            if let i = b.tones.firstIndex(where: { $0.id == t.id }) { b.tones[i] = t } else { b.tones.append(t) }
            return replyOutcome(t.id)
        case .deleteReplyTone(let id):
            guard b.tones.contains(where: { $0.id == id }) else { throw CoreError.NotFound(kind: "tone", id: id) }
            b.tones.removeAll { $0.id == id }
            for k in Array(b.uses.keys) where b.uses[k]?.toneId == id { b.uses[k]?.toneId = "voice" }   // a removed tone sends the uses back to the voice
        case .saveReplyPromptVersion(let promptId, let text):
            try checkReplyPromptText(text: text)
            return replyOutcome(try saveReplyPromptLocked(b, promptId, text, now))
        case .restoreReplyPromptVersion(let promptId, let version):
            let mine = b.prompts.filter { $0.promptId == promptId }
            try checkPromptVersionExists(existing: mine.map(\.version), version: version)
            return replyOutcome(try saveReplyPromptLocked(b, promptId, mine.first { $0.version == version }?.text ?? "", now))
        case .saveFixedNote(let note):
            try checkFixedNote(note: note)
            var n = note
            // hid-ai: a new note never takes the id of a seeded one (the counter starts at 1 again on `fixture reset`, so "note-1" replaced the seed's note-1)
            if n.id.isEmpty { repeat { n.id = newId("note") } while b.notes.contains(where: { $0.id == n.id }) }
            if let i = b.notes.firstIndex(where: { $0.id == n.id }) { b.notes[i] = n } else { b.notes.append(n) }
            return replyOutcome(n.id)
        case .deleteFixedNote(let id):
            guard b.notes.contains(where: { $0.id == id }) else { throw CoreError.NotFound(kind: "note", id: id) }
            b.notes.removeAll { $0.id == id }
        case .draftReplyOnDemand(let threadId, let instruction): return replyOutcome(try draftOnDemandLocked(b, threadId, instruction, now))
        case .requestReplyChange(let draftId, let text):
            let i = try replyDraftIndex(b, draftId)
            let wanted = try checkChangeRequest(options: b.onDemand, draft: b.drafts[i], text: text)
            let rewritten = try replyDraftAfterChange(draft: b.drafts[i], newText: b.drafts[i].text + "\n\n(Modifié : \(wanted))", nowMs: now)
            b.drafts[i] = rewritten
            replyEvent(.thread, .replyDraftChanged(threadId: rewritten.threadId, draft: rewritten))
        case .acceptReplyDraft(let draftId):
            let i = try replyDraftIndex(b, draftId)
            let taken = try replyDraftAfterAccept(draft: b.drafts[i], nowMs: now)
            b.drafts[i] = taken
            // the text becomes an ordinary composer draft in the same thread, as SaveDraft does
            let composerId = newId("draft")
            let account = threadLocked(taken.threadId)?.accountId ?? fx.accounts.first?.id ?? ""
            let from = fxMessage(taken.messageId)?.from ?? ""
            draftRows[composerId] = (account, from.isEmpty ? [] : [from], [], "Re: " + (fxMessage(taken.messageId)?.subject ?? ""), taken.text, taken.threadId)
            draftSavedAt[composerId] = now
            replyEvent(.thread, .replyDraftChanged(threadId: taken.threadId, draft: taken))
            return replyOutcome(composerId)
        case .discardReplyDraft(let draftId):
            let i = try replyDraftIndex(b, draftId)
            let gone = try replyDraftAfterDiscard(draft: b.drafts[i], nowMs: now)
            b.drafts[i] = gone
            replyEvent(.thread, .replyDraftChanged(threadId: gone.threadId, draft: gone))
        case .tryReply(let messageId, let kind): return replyOutcome(try tryReplyLocked(b, messageId, kind))
        case .syncReplyCalendars(let calendars, let start, let end, let events):   // delta 21: the contract's own rule; events of a calendar that is off are dropped
            let r = try applyCalendarSync(known: b.calendars, calendars: calendars, windowStartMs: start, windowEndMs: end, events: events)
            b.calendars = r.calendars
            b.calendarEvents = r.events
        default: throw CoreError.Invalid(field: "command", reasonWords: "this is not an AI replies command")
        }
        return replyOutcome()
    }

    private func replyDraftIndex(_ b: FixtureReplyBook, _ id: String) throws -> Int {
        guard let i = b.drafts.firstIndex(where: { $0.id == id }) else { throw CoreError.NotFound(kind: "reply draft", id: id) }
        return i
    }

    /// delta 24, FOR A `fixture` VERB (the control socket is not this lane's): a mail folder removed on the server (`gone` true) or back (false). The sample has none.
    func fixtureSetReplyFolderGoneLocked(account: String, folder: String, gone: Bool) throws {
        let b = replyBookLocked()
        let ref = ReplySourceRef.mailFolder(accountId: account, folder: folder)
        guard let i = b.mailFolders.firstIndex(where: { $0.source == ref }) else { throw CoreError.NotFound(kind: "source", id: replySourceKey(source: ref)) }
        let key = replySourceKey(source: ref)
        let cur = SourcePresence(enabled: b.mailFolders[i].enabled, ownerEnabled: b.goneOwnerChoice[key], gone: b.goneOwnerChoice[key] != nil)
        let next = sourcePresenceAfterList(cur: cur, present: !gone)
        b.mailFolders[i] = replySourceWithPresence(source: b.mailFolders[i], p: next)
        if !next.gone, case .gone = b.mailFolders[i].progress { b.mailFolders[i].progress = .upToDate }
        b.goneOwnerChoice[key] = next.gone ? (next.ownerEnabled ?? false) : nil
        replyEvent(.syncStatus, .replySourceProgress(source: ref, itemCount: b.mailFolders[i].itemCount, progress: b.mailFolders[i].progress))
    }

    /// the entry the `fixture reply-folder-gone <account> <folder> [back]` verb calls (delta 24; the verb itself belongs to the control socket lane)
    public func fixtureReplyFolderGone(account: String, folder: String, gone: Bool) throws {
        try lock.withLock { try fixtureSetReplyFolderGoneLocked(account: account, folder: folder, gone: gone) }
        emit()
    }

    private func setReplySourceEnabledLocked(_ b: FixtureReplyBook, _ source: ReplySourceRef, _ enabled: Bool) throws {
        try checkSourceRef(source: source)
        if case .calendar(let id) = source {
            guard let i = b.calendars.firstIndex(where: { $0.calendarId == id }) else { throw CoreError.NotFound(kind: "calendar", id: id) }
            b.calendars[i].enabled = enabled
            replyEvent(.syncStatus, .replySourceProgress(source: source, itemCount: 0, progress: enabled ? .upToDate : .notStarted))
            return
        }
        if let i = b.mailFolders.firstIndex(where: { $0.source == source }) {
            let key = replySourceKey(source: source)
            if b.goneOwnerChoice[key] != nil { b.goneOwnerChoice[key] = enabled; return }   // delta 24: a gone folder only remembers the choice, it stays off
            b.mailFolders[i].enabled = enabled
            // switching a folder on starts its background sync; off keeps its count
            if enabled, case .notStarted = b.mailFolders[i].progress { b.mailFolders[i].progress = .syncing(percent: 0) }
            replyEvent(.syncStatus, .replySourceProgress(source: source, itemCount: b.mailFolders[i].itemCount, progress: b.mailFolders[i].progress))
        } else if let i = b.localFolders.firstIndex(where: { $0.source == source }) {
            b.localFolders[i].enabled = enabled
        } else {
            throw CoreError.NotFound(kind: "source", id: replySourceKey(source: source))
        }
    }

    private func saveReplyPromptLocked(_ b: FixtureReplyBook, _ promptId: String, _ text: String, _ now: Int64) throws -> String {
        let mine = b.prompts.filter { $0.promptId == promptId }
        guard !mine.isEmpty else { throw CoreError.NotFound(kind: "prompt", id: promptId) }
        let version = nextPromptVersion(existing: mine.map(\.version))
        for i in b.prompts.indices where b.prompts[i].promptId == promptId { b.prompts[i].current = false }   // one current per prompt
        b.prompts.append(ReplyPrompt(promptId: promptId, version: version, text: text, createdAtMs: now, current: true))
        return String(version)
    }

    private func draftOnDemandLocked(_ b: FixtureReplyBook, _ threadId: String, _ instruction: String?, _ now: Int64) throws -> String {
        guard let thread = threadLocked(threadId), let messageId = thread.messageIds.last else { throw CoreError.NotFound(kind: "thread", id: threadId) }
        let wanted = try checkOnDemandInstruction(options: b.onDemand, instruction: instruction)
        // a new draft replaces the thread's live one: the old becomes Discarded (one live draft per thread)
        if let i = b.drafts.firstIndex(where: { $0.threadId == threadId && b.isLive($0) }), let old = replyDraftReplaced(live: b.drafts[i], nowMs: now) {
            b.drafts[i] = old
            replyEvent(.thread, .replyDraftChanged(threadId: threadId, draft: old))
        }
        let id = newId("reply-draft")
        let text = "Bonjour,\n\n" + (wanted.map { "Suite à votre message : \($0).\n\n" } ?? "Merci pour votre message.\n\n") + "Bien à vous,\nAssem"
        let d = ReplyDraft(id: id, threadId: threadId, messageId: messageId, text: text, madeBy: .onDemand, excerptIds: [chunkExcerptId(chunkId: 1)], instruction: wanted,
                           state: .proposed, revision: 0, createdAtMs: now, updatedAtMs: now)
        b.drafts.append(d)
        replyEvent(.thread, .replyDraftReady(threadId: threadId, draftId: id))
        replyEvent(.thread, .replyDraftChanged(threadId: threadId, draft: d))
        return id
    }

    /// "Essayer sur un message": retrieval and the three steps on one message. Changes nothing and stores no draft.
    private func tryReplyLocked(_ b: FixtureReplyBook, _ messageId: String, _ kind: ReplyUseKind) throws -> String {
        guard let m = fxMessage(messageId), !gone(messageId) else { throw CoreError.NotFound(kind: "message", id: messageId) }
        let f = facts(m)
        let labelIds = (assigned[m.id] ?? []) + [category[m.id] ?? m.tab ?? "primary"]
        var entry = ReplyStepResult.skipped, ai = ReplyStepResult.skipped, exit = ReplyStepResult.skipped   // on demand the owner picked the thread: no filter applies
        if kind == .auto {
            entry = replyEntryStep(filter: b.filter, messageLabelIds: labelIds, facts: f)
            ai = (entry == .passed || entry == .skipped) ? replyAiStep(filter: b.filter, modelSaysReplyWanted: f.unanswered) : .notReached
            exit = (ai == .passed || ai == .skipped) ? replyExitStep(filter: b.filter, facts: f) : .notReached
        }
        b.counter += 1
        let run = "reply-try-\(b.counter)"
        let through = { (r: ReplyStepResult) in r == .passed || r == .skipped }
        let kept = ["chunk-1", "chunk-2"].map { b.sample($0) }
        let draft = (through(entry) && through(ai) && through(exit)) ? "Bonjour,\n\nMerci pour votre message.\n\nBien à vous,\nAssem" : nil
        b.tries[run] = ReplyTryReport(
            runId: run, messageId: messageId, kind: kind,
            retrieval: ReplyRetrievalReport(questions: ["Quel prix avons-nous proposé à ce client ?", "Quand avons-nous parlé de ce projet pour la dernière fois ?"], found: 5, kept: kept, droppedByAiFilter: 1),
            entry: ReplyStepReport(result: entry, matched: entry == .passed ? b.filter.entryRule.conditions : [], modelNote: nil),
            ai: ReplyStepReport(result: ai, matched: [], modelNote: ai == .notReached ? nil : (f.unanswered ? "Le message pose une question." : "Aucune réponse n’est attendue.")),
            exit: ReplyStepReport(result: exit, matched: exit == .passed ? b.filter.exitRule.conditions : [], modelNote: nil),
            draftText: draft)
        return run
    }

    // MARK: state reads (the contract's encoders; paths in reply_ai_state.rs)

    private func replyString(_ s: String) -> String {
        guard let d = try? JSONSerialization.data(withJSONObject: s, options: [.fragmentsAllowed]) else { return "\"\"" }
        return String(data: d, encoding: .utf8) ?? "\"\""
    }

    func repliesStateLocked(_ p: [String]) -> D9Hit? {
        guard p.first == "reply" else { return nil }
        let b = replyBookLocked()
        func hit(_ s: String?) -> D9Hit? { D9Hit(value: s) }
        switch (p.count, p.count > 1 ? p[1] : "") {
        case (3, "use"):
            guard let kind = replyUseKindFromKey(key: p[2]), let u = b.uses[kind] else { return hit(nil) }
            return hit(replyUseJson(u: u))
        case (2, "filter"): return hit(replyFilterJson(f: b.filter))
        case (2, "on_demand"): return hit(onDemandJson(o: b.onDemand))
        case (2, "counts"): return hit(replyCountsJson(c: countReplySteps(passes: b.passes, nowMs: clock.nowMs(), days: replyCountsDays())))
        case (2, "sources"):
            let accountIds = b.mailFolders.compactMap { s -> String? in if case .mailFolder(let a, _) = s.source { return a } else { return nil } }
            var seen: [String] = []
            for a in accountIds where !seen.contains(a) { seen.append(a) }
            let cards = seen.map { replyAccountSources(accountId: $0, folders: b.mailFolders) }
            return hit(replySourcesJsonWithChoices(r: replySourcesRead(accounts: cards, localFolders: b.localFolders, calendars: b.calendars, scope: b.retrieval.scope), choices: b.goneOwnerChoice.map { SourceChoice(sourceKey: $0.key, ownerEnabled: $0.value) }.sorted { $0.sourceKey < $1.sourceKey }))   // delta 24: owner_enabled of a gone folder
        case (2, "retrieval"): return hit(retrievalJson(r: b.retrieval))
        case (2, "voice"): return hit(replyVoiceJson(voice: b.voice, analysis: b.analysis, tones: b.tones))
        case (3, "prompt"):
            let mine = b.prompts.filter { $0.promptId == "replies" }
            switch p[2] {
            case "current": return hit(mine.first { $0.current }.map { replyPromptJson(p: $0) })
            case "versions": return hit(replyPromptVersionsJson(versions: mine))
            case "variables": return hit(replyPromptVariablesJson(vars: replyPromptVariables()))
            case "preview":   // what the model receives for the newest message: the variables replaced, the note lines dropped
                guard let text = mine.first(where: { $0.current })?.text else { return hit(nil) }
                let m = messages.first { !deleted.contains($0.id) }
                let values = [PromptValue(id: "message", value: m.map { "De \($0.sender) : \($0.subject)\n\($0.preview)" } ?? ""), PromptValue(id: "thread", value: m?.preview ?? ""),
                              PromptValue(id: "instruction", value: ""), PromptValue(id: "excerpts", value: b.sample("chunk-1").text + "\n" + b.sample("chunk-2").text),
                              PromptValue(id: "calendar", value: "mardi après-midi, jeudi après-midi"), PromptValue(id: "tone", value: b.voice.toneText),
                              PromptValue(id: "notes", value: b.notes.map { "\($0.title) : \($0.text)" }.joined(separator: "\n"))]
                return hit(replyString(renderReplyPrompt(text: text, values: values)))
            default: return hit(nil)
            }
        case (2, "notes"): return hit(fixedNotesJson(notes: b.notes))
        case (3, "source_chunk"): return hit(Int64(p[2]).flatMap { b.sourceChunk($0) }.map { sourceChunkJson(c: $0) })   // delta 23
        case (3, "calendars") where p[2] == "enabled": return hit(enabledCalendarIdsJson(calendars: b.calendars))   // delta 21
        case (3, "calendars") where p[2] == "events": return hit(calendarEventsJson(events: b.calendarEvents))
        case (2, "progress"): return hit(folderProgressJson(rows: b.folderProgress()))
        case (3, "chunk"): return hit(Int64(p[2]).flatMap { b.chunk($0) }.map { chunkJson(c: $0) })
        case (3, "draft"):   // the thread's LIVE draft; absent when there is none
            return hit(b.drafts.last { $0.threadId == p[2] && b.isLive($0) }.map { replyDraftJson(d: $0) })
        case (3, "try"): return hit(b.tries[p[2]].map { replyTryJson(t: $0) })
        default: return hit(nil)
        }
    }
}
