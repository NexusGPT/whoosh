import Foundation
import Observation

/// The label inspector's state: which label (or none yet) is being defined in pane 3, the draft, and the live proof the list
/// draws. UI state only: the saved label is the core's (`labels()`, `state(path:)`), every write goes through the core's
/// `createLabelFull`, `updateLabel` and `rerunLabel`, and every count is the core's `ruleSetCount` and `labelRerunCount`.
@MainActor @Observable
final class LabelInspectorModel {
    static let shared = LabelInspectorModel()

    enum Mode: Equatable { case new, modify(String) }

    /// The question after an applied change: re-file the mail the label already holds? `baseline` is what the rules were.
    struct Offer: Equatable {
        var filed: Int
        var baseline: LabelDraft
        var join: Int, leave: Int, stay: Int
        /// Older inbound messages the label's meaning has not judged yet: a step looks at 500, and the Re-run offer says how many wait (core state `rerun/beyond`).
        var beyond: Int = 0
    }

    /// What a re-run did, read back from the core: the counts and, per message that joined, who decided it, and the card and chain the core's why read names.
    struct RerunResult: Equatable {
        var filed: Int, refiled: Int, unfiled: Int
        var decided: [(messageId: String, by: String, card: String?, chain: Int?)]
        /// the engine's own `rerun/done` (the mails its judge answered about) and `rerun/total` of the run; 0 when it said nothing
        var judged = 0, total = 0
        static func == (a: Self, b: Self) -> Bool {
            a.filed == b.filed && a.refiled == b.refiled && a.unfiled == b.unfiled && a.decided.map(\.messageId) == b.decided.map(\.messageId)
        }
    }

    var mode: Mode?
    var draft = LabelDraft()
    var saved = LabelDraft()
    var proof = LabelProof.empty
    /// a count is being read: the last one stays on screen until the new one lands
    var counting = false
    /// a test door: keeps `counting` true so the loading state can be seen and shot
    var holdCounting = false
    /// pane 2 shows the matches (true) or the mailbox again (false) without closing the inspector
    var showMatches = true
    var refusal: (field: String, words: String)?
    var failure: String?
    var statusLine: String?
    var offer: Offer?
    var result: RerunResult?
    /// a text field holds the keys, so a bare letter is typed, not a command
    var editing = false
    /// `click mac-labels.inspector.blur`: the field that holds the keys lets go (the socket cannot click away); the view reads the tick.
    var blurTick = 0
    func blur() -> Bool { guard isOpen else { return false }; blurTick += 1; editing = false; return true }
    var corpus: [ProofCorpusItem] = []
    var generation = 0
    /// the foot's four figures for a label that exists (state `label/<id>/flow/counts`); nil for a new label
    var counts: FlowCounts?
    /// why the AI cannot sort now, in the core's own words (`label/<id>/flow/ai_refusal`); nil = it can, or the card is off, or the label is new
    var aiRefusal: String?
    /// the core's code behind `aiRefusal` (`ai_needs_apple` changes the card's door); and the URLs the Apple Intelligence door opened (a test copy only records them)
    var aiRefusalCode: String?
    var appleSettingsOpened: [String] = []
    /// the on-device decision model (Laya): are its weights on this Mac, is a download running (read with the counts, never per frame)
    var layaState: LayaModelState = .notDownloaded
    /// a re-run is running on the core (it answers at once and judges in the background): its progress words
    var rerunning = false
    /// « Pourquoi ici ? » is open over the foot, and the lines it says about the message in focus
    var whyOpen = false
    var whyLines: [String] = []
    /// the rule row whose value field holds the keys (set by the view), and a door's request to give one the keys (taken by the view)
    var focusedRule: UUID?
    var focusRequest: LabelFocus?

    /// The listener of `LayaModelStore.didChange`, kept for the model's life.
    @ObservationIgnored private var layaListener: NSObjectProtocol?

    init() {
        // a download started or moved elsewhere (Settings, Decisions) redraws the inspector's AI card too. The store announces on the next turn of the
        // main queue and only when its state really changed.
        layaListener = NotificationCenter.default.addObserver(forName: LayaModelStore.didChange, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.layaDidChange() }
        }
    }

    /// Takes the store's CURRENT state with `modelStore.state`, which only reads: `LayaOnDevice.state` goes through `refresh()`, which re-reads the disk and would
    /// overwrite a running download with "not downloaded" and announce that, each time (the loop of main74).
    private func layaDidChange() {
        let now = LayaOnDevice.shared.modelStore.state
        if now != layaState { layaState = now }
    }

    var app: AppModel { AppModel.shared }
    var core: WhooshCore { app.core }

    var isOpen: Bool { mode != nil }
    var isNew: Bool { mode == .new }
    var labelId: String? { if case .modify(let id) = mode { id } else { nil } }
    var changes: Int { isNew ? (draft.name.isEmpty ? 0 : max(1, draft.changes(from: saved))) : draft.changes(from: saved) }
    var dirty: Bool { changes > 0 }
    var parentLabel: MailLabel? { draft.parentId.flatMap { id in core.labels().first { $0.id == id } } }
    var scopeAccountName: String? {
        if case .account(let a) = draft.scope { return AccountBook.shared.account(a)?.name ?? a } else { return nil }
    }
    /// Create or Apply can run: named, changed, and nothing refused
    var canWrite: Bool { !draft.name.trimmingCharacters(in: .whitespaces).isEmpty && refusal == nil && (isNew || dirty) }

    // MARK: opening and closing

    @discardableResult
    func openNew(parent: String?, name: String = "") -> Bool {
        var d = LabelDraft()
        d.parentId = parent; d.name = name
        begin(.new, draft: d, saved: LabelDraft())
        return true
    }

    @discardableResult
    func openModify(_ id: String) -> Bool {
        guard let l = core.labels().first(where: { $0.id == id }) else { return false }
        let d = readDraft(l)
        begin(.modify(id), draft: d, saved: d)
        return true
    }

    /// The label as the core holds it: fields, entry and exit chains (a core that serves only delta 9's one rule set gives chain 0), cards.
    func readDraft(_ l: MailLabel) -> LabelDraft {
        var entry = StoredChains.decode(core.state(path: "label/\(l.id)/chains/entry"))
        if entry.isEmpty, let set = StoredRules.object(core.state(path: "label/\(l.id)/rules")) as? [String: Any] {
            entry = [DecodedChain(id: 0, mode: StoredRules.mode(set["match"]), conditions: StoredRules.conditions(set["conditions"]))]
        }
        var d = LabelDraft(saved: l, entry: entry, exit: StoredChains.decode(core.state(path: "label/\(l.id)/chains/exit")))
        // No starter rows here: a SAVED label shows the rows it holds. Seeding "From" and "Subject" whenever the saved chain had none (the old line) brought the two
        // empty rows back at every open and after every Save, so a row the owner removed (an empty row is never stored) could never stay deleted. The two starter
        // rows belong to a NEW label (`ChainDraft()`'s default).
        return d
    }

    func refreshCounts() {
        counts = labelId.flatMap { FlowCounts(core.state(path: "label/\($0)/flow/counts")) }
        aiRefusal = labelId.flatMap { AiRefusal.words(core.state(path: "label/\($0)/flow/ai_refusal")) }
        aiRefusalCode = labelId.flatMap { AiRefusal.code(core.state(path: "label/\($0)/flow/ai_refusal")) }
        if case .downloading = layaState {} else { layaState = LayaOnDevice.shared.state }
    }

    private func begin(_ m: Mode, draft d: LabelDraft, saved s: LabelDraft) {
        mode = m; draft = d; saved = s; offer = nil; result = nil; failure = nil; statusLine = nil; showMatches = true
        whyOpen = false; whyLines = []; refreshCounts()
        corpus = LabelProofCompute.corpus(app)
        recompute()
    }

    /// Esc, Close and Cancel: the draft is dropped, the reader comes back, pane 2 returns to the mailbox.
    @discardableResult
    func close() -> Bool {
        guard isOpen else { return false }
        mode = nil; offer = nil; failure = nil; refusal = nil; editing = false; generation += 1; counting = false; whyOpen = false
        return true
    }

    /// ⌘I: the inspector opens on the selected label (or a new one), or goes back to the reader.
    @discardableResult
    func toggle(selected: String?) -> Bool {
        if isOpen { return close() }
        if let id = selected, openModify(id) { return true }
        return openNew(parent: nil)
    }

    // MARK: editing

    /// Every edit lands here: the field changes on the keystroke's frame, the proof follows.
    func edit(_ change: (inout LabelDraft) -> Void) {
        change(&draft); failure = nil; statusLine = nil
        recompute()
    }

    func editRule(_ id: UUID, _ change: (inout RuleDraft) -> Void) { editRule(.entry, id, change) }

    /// The refusal the core would give, read before any write (the contract's own check), then the one rule this view adds:
    /// an account rule the label's scope can never reach.
    func preflight() { refusal = specRefusal() ?? flowRefusal() }

    private func specRefusal() -> (field: String, words: String)? {
        if draft.name.trimmingCharacters(in: .whitespaces).isEmpty { return nil }
        do { try validateLabelSpec(spec: draft.spec, parent: parentLabel) }
        catch CoreError.Invalid(let field, let words) { return (field, Words.text(words)) }
        catch { return ("", "\(error)") }
        if case .account(let scope) = draft.scope, let bad = draft.rules.first(where: { $0.kind == .account && $0.allValues.contains { $0 != scope } }),
           let other = bad.allValues.first(where: { $0 != scope }) {
            let a = scopeAccountName ?? scope, b = AccountBook.shared.account(other)?.name ?? other
            return ("rules", Words.text("The label applies to %@ only, so it cannot match %@ mail. Change Applies to, or remove the rule.", a, b))
        }
        return nil
    }

    /// Reads the proof of the draft: the count is the core's, the rows are the rules run over the corpus. The last proof stays
    /// until the new one lands (no flash); only the newest read is kept.
    func recompute() {
        preflight()
        generation += 1
        let mine = generation
        let was = proof.count
        counting = true
        let (c, items, d) = (core, corpus, draft)
        let baseline = offer?.baseline ?? saved
        let filed = Set(items.filter { $0.row.labelNames.contains(saved.name) }.map(\.row.messageId))
        let savedOrNil: LabelDraft? = isNew ? nil : baseline
        let written: String? = offer != nil ? labelId : nil   // after Save the rules are the core's, and so is the preview
        Task.detached(priority: .userInitiated) {
            let p = LabelProofCompute.compute(core: c, corpus: items, draft: d, saved: savedOrNil, filed: filed, written: written)
            await MainActor.run { [weak self] in
                guard let self, self.generation == mine else { return }
                var next = p; next.was = was
                self.proof = next
                self.settleOffer()
                self.counting = self.holdCounting
            }
        }
    }
}
