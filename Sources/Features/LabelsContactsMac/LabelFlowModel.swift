import Foundation
#if os(macOS)
import AppKit
#endif

/// Which chain of the open label an edit is about: chain 0 of the Entry card, another Entry chain (an alternative), the Exit card's.
enum ChainRef: Hashable, Sendable { case entry, other(UUID), exit }

/// What the AI card can read of a message.
enum AiInput: CaseIterable, Sendable { case subject, body, restOfThread, attachments }

/// The refusal codes of the flow, as the core names them (`field` of the `CoreError`; core/crates/contracts/src/label_flow.rs).
enum FlowRefusal {
    static let exitNeedsFallback = "exit_needs_fallback"
    static let badFallback = "bad_exit_fallback"
    static let badChain = "bad_chain"
    static let aiReadsNothing = "ai_reads_nothing"
    static let codes: Set<String> = [exitNeedsFallback, badFallback, badChain, aiReadsNothing]
    /// The core's own English sentences are the catalogue keys, so the words are the owner's language and never a mix.
    static var exitWords: String { Words.text("Say where the other messages go before you turn the exit filter on") }
    static var aiWords: String { Words.text("the AI has to read at least one of: subject, body, attachments. To stop the AI sorting here, switch the card off") }
}

/// The core's refusal of the AI card (`{"code":"ai_no_model","words":"..."}` or null): its sentence is a catalogue key, so the owner reads it in their language.
enum AiRefusal {
    /// The core's machine code of the refusal (`ai_no_model`, `ai_under_trust`, `ai_needs_apple`...), nil when there is none.
    static func code(_ json: String?) -> String? {
        (StoredRules.object(json) as? [String: Any])?["code"] as? String
    }
    static func words(_ json: String?) -> String? {
        guard let o = StoredRules.object(json) as? [String: Any], let w = o["words"] as? String, !w.isEmpty else { return nil }
        // `ai_under_trust`: the AI judged mails and was sure of none; the sentence carries three numbers (judged, its best percent, the trust percent it files from)
        if o["code"] as? String == "ai_under_trust", let n = (o["judged"] as? NSNumber)?.intValue,
           let best = (o["highest_percent"] as? NSNumber)?.intValue, let trust = (o["trust_percent"] as? NSNumber)?.intValue {
            return Words.text(w, n, best, trust)
        }
        return Words.text(w)
    }
}

extension LabelInspectorModel {
    /// The words the AI card and the empty list say while the AI cannot sort: only when the card is on in the draft.
    var aiRefusalShown: String? {
        guard draft.flow.aiOn else { return nil }
        return aiRefusal ?? layaWords
    }

    /// The on-device decision model is the one in force (the saved choice, else the shipped Local) and its weights are not on this Mac (or are coming):
    /// the sentence for it. The core says nothing about it before the first judgement fails, so the shell, which holds the weights, says it first.
    var layaWords: String? {
        guard CoreHost.shared.kind == .rust, (core.decisionModel()?.provider ?? .local) == .local else { return nil }
        switch layaState {
        case .ready: return nil
        case .notDownloaded: return LayaOnDevice.notDownloadedWords
        case .downloading, .failed: return layaState.words
        }
    }

    /// The AI card is on and the engine says it cannot rule (no model, Apple Intelligence off, the judge failed, or the Laya weights are not here). Under trust is not one:
    /// the AI did rule, on what it was given, and was sure of none.
    var aiCannotRule: Bool { aiRefusalShown != nil && aiRefusalCode != "ai_under_trust" }

    /// The head line of the Re-run preview (a saved label whose rules moved). It never promises what the card denies: with the AI on and unable to rule it counts the picks
    /// and 0 joins by the AI ("0 picked · 0 by the AI", the engine's own `by_ai`, which is 0 then), and the AI card says why in its one sentence; with the AI able to rule it says
    /// the AI rules on each pick after Save; with the AI off it counts joins, leaves and stays.
    var previewLine: String? {
        // The counts are read into plain numbers first, from ONE copy of the proof: the Release compiler (Swift 6.3.3, CopyPropagation) crashed on
        // this getter when it read `proof.joinByRule` and `proof.joinByAi ?? 0` inside the branches (main107, "Found outside of lifetime use").
        let p: LabelProof = proof
        guard p.hasPreview else { return nil }
        let byRule: Int = p.joinByRule
        let byAi: Int = p.joinByAi ?? 0
        let leaves: Int = p.leaveCount
        let stays: Int = p.stayCount
        // BOTH say the AI cannot rule: the card's refusal, and the engine's own preview sentence (`ai_words`, "The AI cannot sort yet: no model on this Mac."), which can come
        // while the card shows none (main107's pictures: the head promised "the AI rules on each after Save" right above that sentence)
        let engine: EnginePreview? = p.engine
        let engineSays: Bool = engine?.aiWords != nil
        let cardSays: Bool = aiCannotRule
        return Self.previewWords(aiOn: draft.flow.aiOn, cannotRule: cardSays || engineSays, byRule: byRule, byAi: byAi, leaves: leaves, stays: stays)
    }

    /// The words of `previewLine`, from numbers alone.
    static func previewWords(aiOn: Bool, cannotRule: Bool, byRule: Int, byAi: Int, leaves: Int, stays: Int) -> String {
        if !aiOn { return Words.text("%lld join · %lld leaves · %lld stay", byRule + byAi, leaves, stays) }
        if cannotRule { return [Words.text("%lld picked", byRule), Words.text("%lld by the AI", byAi)].joined(separator: " · ") }
        return Words.text("%lld picked · the AI rules on each after Save", byRule)
    }

    /// The control under the refusal: download the weights (Laya missing), else open Decisions.
    var refusalDoor: (title: String, id: String)? {
        guard aiRefusalShown != nil else { return nil }
        if aiRefusal == nil, layaWords != nil {
            if case .downloading = layaState { return nil }
            return (Words.text("Download the decision model"), "laya.download")
        }
        // Sorting mail into labels is judged by Apple's on-device model, and Apple Intelligence is off: the pane that turns it on is the fix, not Decisions
        if aiRefusalCode == "ai_needs_apple" { return (Words.text("Open System Settings"), "apple.settings") }
        return (Words.text("Open Decisions"), "decisions")
    }

    /// The pane of System Settings that holds Apple Intelligence: the Settings extension `com.apple.Siri-Settings.extension` (SiriPreferenceExtension.appex in
    /// /System/Library/ExtensionKit/Extensions on macOS 26, extension point `com.apple.Settings.extension.ui`), opened with the `x-apple.systempreferences:` scheme.
    static let appleIntelligencePane = "x-apple.systempreferences:com.apple.Siri-Settings.extension"

    /// The door to that pane. It never opens on a test copy: there it records the URL it would open (`apple-settings` row), as the Calendars door only counts its press.
    @discardableResult
    func openAppleSettings() -> Bool {
        guard aiRefusalCode == "ai_needs_apple" else { return false }
        appleSettingsOpened.append(Self.appleIntelligencePane)
        #if os(macOS)
        if CoreHost.shared.kind != .fixture, let url = URL(string: Self.appleIntelligencePane) { NSWorkspace.shared.open(url) }
        #endif
        return true
    }

    /// Runs the control `refusalDoor` named.
    @discardableResult
    func runRefusalDoor(_ id: String) -> Bool {
        switch id {
        case "decisions": return openDecisions()
        case "apple.settings": return openAppleSettings()
        default: return downloadLaya()
        }
    }

    /// The door that downloads Laya once, with progress; when the weights are there the open label is re-run (the owner asked for it by downloading).
    @discardableResult
    func downloadLaya() -> Bool {
        guard layaWords != nil, aiRefusal == nil else { return false }
        if case .downloading = layaState { return false }
        layaState = .downloading(received: 0, total: -1)
        Task { [weak self] in
            let end = await LayaOnDevice.shared.download { st in Task { @MainActor in self?.layaState = st } }
            await MainActor.run {
                guard let self else { return }
                self.layaState = end
                self.refreshCounts()
                if case .ready = end, let id = self.labelId, (try? self.core.run(command: .rerunLabel(labelId: id))) != nil { self.watchRerun(id, joined: []) }
            }
        }
        return true
    }

    /// Opens Settings on the Decisions pane, where the decision model is chosen.
    @discardableResult
    func openDecisions() -> Bool { WindowBridge.open(.macSettingsAI, in: app) }

    func chain(_ ref: ChainRef) -> ChainDraft? {
        switch ref {
        case .entry: draft.entry
        case .other(let id): draft.flow.others.first { $0.id == id }
        case .exit: draft.flow.exits.first
        }
    }

    func editChain(_ ref: ChainRef, _ change: (inout ChainDraft) -> Void) {
        edit { d in
            switch ref {
            case .entry: change(&d.entry)
            case .other(let id): if let i = d.flow.others.firstIndex(where: { $0.id == id }) { change(&d.flow.others[i]) }
            case .exit:
                if d.flow.exits.isEmpty { d.flow.exits = [ChainDraft(rows: [])] }
                change(&d.flow.exits[0])
            }
        }
    }

    func editRule(_ ref: ChainRef, _ id: UUID, _ change: (inout RuleDraft) -> Void) {
        editChain(ref) { c in
            guard let i = c.rows.firstIndex(where: { $0.id == id }) else { return }
            change(&c.rows[i])
        }
    }

    /// Return in a rule field: the text typed so far becomes one value, and the field is empty for the next.
    func commitPending(_ ref: ChainRef, _ id: UUID) {
        editRule(ref, id) { r in
            let t = r.pending.trimmingCharacters(in: .whitespacesAndNewlines)
            if !t.isEmpty, !r.values.contains(t) { r.values.append(t) }
            r.pending = ""
        }
    }

    func setFact(_ ref: ChainRef, _ index: Int, _ state: FactState) {
        editChain(ref) { if $0.facts.indices.contains(index) { $0.facts[index] = state } }
    }
    func setMode(_ ref: ChainRef, _ mode: MatchMode) { editChain(ref) { $0.matchMode = mode } }
    func addRule(_ ref: ChainRef) { editChain(ref) { $0.rows.append(RuleDraft(kind: .domain)) } }
    func removeRule(_ ref: ChainRef, _ id: UUID) { editChain(ref) { $0.rows.removeAll { $0.id == id } } }

    /// « Autre règle » (A): a second chain on the Entry card. The chains of a card are alternatives; inside one, its mode decides.
    @discardableResult
    func addOtherChain() -> Bool {
        guard isOpen else { return false }
        edit { $0.flow.others.append(ChainDraft()) }
        return true
    }
    func removeChain(_ id: UUID) { edit { $0.flow.others.removeAll { $0.id == id } } }

    func setCard(_ card: FlowCard, _ on: Bool) {
        edit { switch card { case .entry: $0.flow.entryOn = on; case .ai: $0.flow.aiOn = on; case .exit: $0.flow.exitOn = on } }
    }
    func toggleRead(_ input: AiInput) {
        edit { switch input { case .subject: $0.flow.reads.subject.toggle(); case .body: $0.flow.reads.body.toggle(); case .restOfThread: $0.flow.reads.restOfThread.toggle(); case .attachments: $0.flow.reads.attachments.toggle() } }
    }
    func setFallback(_ id: String?) { edit { $0.flow.fallback = id } }
    /// The refusals `check_set_card` and `check_set_ai_reads` would give, read before any write.
    func flowRefusal() -> (field: String, words: String)? {
        let f = draft.flow
        // the thread alone is not a reading: it is read WITH the message (`check_set_ai_reads`)
        if !(f.reads.subject || f.reads.body || f.reads.attachments) { return (FlowRefusal.aiReadsNothing, FlowRefusal.aiWords) }
        if f.exitOn && (f.fallback ?? "").isEmpty { return (FlowRefusal.exitNeedsFallback, FlowRefusal.exitWords) }
        return nil
    }

    func refusal(for codes: Set<String>) -> String? { refusal.flatMap { codes.contains($0.field) ? $0.words : nil } }

    func fallbackName(_ id: String?) -> String? {
        guard let id, !id.isEmpty else { return nil }
        return id == "primary" ? Words.text("Primary") : (id == "junk" ? Words.text("Spam") : core.labels().first { $0.id == id }?.name)
    }

    @discardableResult
    func toggleWhy() -> Bool {
        guard isOpen else { return false }
        whyOpen.toggle()
        if whyOpen { refreshWhy() }
        return true
    }
    func refreshWhy() { whyLines = LabelWhy.lines(core: core, label: labelId, message: app.focusedMessageId) }

    // what the cards and the foot say, once: the screen and the socket read the same words
    var entryCountText: String { counts.map { Words.text("picks %lld", $0.entry) } ?? "" }
    var aiCountText: String {
        guard let c = counts else { return "" }
        let accepted = c.ai, waiting = waitingForAi, aside = setAsideByApple      // plain Ints first (the Release compiler)
        return LabelsWords.accepted(accepted, waiting: waiting, aside: aside)
    }
    /// The picked mails the judge refused for themselves and set aside (`label/<id>/flow/ai_set_aside`, a number); 0 when the AI card is off or the core does not say.
    var setAsideByApple: Int {
        guard saved.flow.aiOn, let id = labelId else { return 0 }
        return Int(core.state(path: "label/\(id)/flow/ai_set_aside") ?? "") ?? 0
    }
    /// "keeps 23 of 31": of the mails the AI accepted, or of the mails step 1 picked when the AI card is off (SPEC A140).
    var exitCountText: String { counts.map { Words.text("keeps %lld of %lld", $0.kept, saved.flow.aiOn ? $0.ai : $0.entry) } ?? "" }

    /// The mails step 1 picked that the AI has not ruled on yet (`waiting_for_ai` beside the four figures of `label/<id>/flow/counts`). 0 when the AI card is off,
    /// when every picked mail is ruled (the read is skipped: picks never outnumber accepts then), or when the core does not say.
    var waitingForAi: Int {
        guard let c = counts, saved.flow.aiOn, c.entry > c.ai, let id = labelId else { return 0 }
        let o = StoredRules.object(core.state(path: "label/\(id)/flow/counts")) as? [String: Any]
        return (o?["waiting_for_ai"] as? NSNumber)?.intValue ?? 0
    }

    /// The figures after Save and a re-run, from the proof the list already holds (one computation, never a second pass over the mail); nil while nothing is changed.
    var countsAfter: CountsAfter? {
        CountsAfter.of(counts: counts, proof: proof, draft: draft, dirty: dirty, isNew: isNew, aiOn: saved.flow.aiOn, exitOn: saved.flow.exitOn)
    }
    var entryAfterText: String? { countsAfter?.entry.map { Words.text("%lld after Save", $0) } }
    var aiAfterText: String? { countsAfter?.ai.map { Words.text("%lld after Save", $0) } }
    /// The foot while changes are not saved. AI sorting on: "Now 0 → the AI rules on 48" (the picked mails the AI will rule on). Off: "Now 0 → 48 by rule", then
    /// "· 0 by the AI" when the preview knows it. `short` drops that last part.
    func nowAfterLine(short: Bool = false) -> String? {
        guard let a = countsAfter, let e = a.entry else { return nil }
        if draft.flow.aiOn { return Words.text("Now %lld → the AI rules on %lld", a.now, e) }
        if let ai = a.ai, !short { return Words.text("Now %lld → %lld by rule · %lld by the AI", a.now, e, ai) }
        return Words.text("Now %lld → %lld by rule", a.now, e)
    }

    /// The foot's four figures of the saved label's flow, each a sentence with its number: "24 messages", "1 by rule", "31 by the AI", "8 to Primary"
    /// (AI sorting on: "24 messages", "48 picked", "31 accepted", "8 to Primary").
    /// The last only while the exit filter is on and knows where the others go.
    var provenanceParts: [String]? {
        guard let c = counts, !isNew else { return nil }
        let s = saved.flow, held = c.held(aiOn: s.aiOn, exitOn: s.exitOn)
        var parts = [held == 1 ? LabelsWords.oneMessage : LabelsWords.messages(held)]
        if s.aiOn { parts += [Words.text("%lld picked", c.entry), Words.text("%lld accepted", c.ai)] }   // A140: step 1 picks, the AI accepts
        else { parts += [Words.text("%lld by rule", c.entry), Words.text("%lld by the AI", c.ai)] }
        if s.exitOn, let to = fallbackName(s.fallback) { parts.append(Words.text("%lld to %@", c.sent, to)) }
        return parts
    }
}

/// What the three cards and the foot say after Save and a re-run, while the inspector holds unsaved changes. Pure: the same `LabelProof` the list
/// ("48 picked · the AI rules on each after Save") reads gives every figure, so the cards and the list cannot disagree.
struct CountsAfter: Equatable, Sendable {
    /// the messages the label holds now (the foot's first figure)
    var now: Int
    /// picked by the entry rules after Save: now + picks by rule - leaves; nil when the Entry card is off
    var entry: Int?
    /// accepted by the AI after Save: now + joins by the AI; nil unless the engine has spoken (`LabelProof.joinByAi`)
    var ai: Int?

    static func of(counts: FlowCounts?, proof: LabelProof, draft: LabelDraft, dirty: Bool, isNew: Bool, aiOn: Bool, exitOn: Bool) -> CountsAfter? {
        guard dirty, !isNew, proof.hasPreview, let c = counts else { return nil }
        let entry: Int? = draft.flow.entryOn ? max(0, c.entry + proof.joinByRule - proof.leaveCount) : nil
        return CountsAfter(now: c.held(aiOn: aiOn, exitOn: exitOn), entry: entry, ai: proof.joinByAi.map { c.ai + $0 })
    }
}
