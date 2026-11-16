import Foundation
import Observation

/// The pane's state. Contract types where the contract has them (AiProvider, AgentConfig, AgentPermission,
/// DecisionProviderKind); keys, agents and speech go through the core's Commands; the doors it does not serve yet are named INTERIM (docs/lanes/settingsaimac.md).
@MainActor @Observable
final class AISettingsStore {
    static let shared = AISettingsStore()

    /// The decision model's store, asked each time (a rig copy swaps it for one that reads a loopback server). A test gives its own.
    private let modelStore: @Sendable () -> LayaModelStore
    /// The listener of `LayaModelStore.didChange`, installed here and not by the fixture: every change of the store reaches the line on the
    /// owner's copy as on a test copy. It never reads the model's disk itself; the store announces, the screen re-reads once.
    @ObservationIgnored private var layaListener: NSObjectProtocol?

    init(modelStore: @escaping @Sendable () -> LayaModelStore = { LayaOnDevice.shared.modelStore }) {
        self.modelStore = modelStore
        layaListener = NotificationCenter.default.addObserver(forName: LayaModelStore.didChange, object: nil, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.layaStateChanged() }
        }
    }

    enum Section: Hashable { case providers, agents, decisions, page(String) }
    enum Engine: String, CaseIterable { case local, jev, off }
    enum Speech: String { case none, downloading, ready, failed }

    var section: Section = .decisions
    /// The four rows the pane shows; the core serves the ones it holds a key for (anthropic, openai).
    private static let shownProviders = [("anthropic", "Anthropic"), ("openai", "OpenAI"), ("gemini", "Gemini"), ("openrouter", "OpenRouter")]
    var providers: [AiProvider] = AISettingsStore.shownProviders.map {
        AiProvider(id: $0.0, name: $0.1, models: [], connected: false, keychain: nil)
    }
    var refusals: [String: String] = [:]
    /// The key text typed in each provider's field (a secure field: never read back to a row, only its length).
    var keyDrafts: [String: String] = [:]
    /// The provider whose key field holds the keys (the socket's `type` verb goes there while it is set).
    var keyFocus: String?
    /// The model entry whose question the socket's `type` verb edits (set by the door `entry.<n>.question`, the text field's own click).
    var questionFocus: (page: String, entry: UUID)?
    /// Providers being asked about a pasted key: the answer comes off the main thread.
    private var checks: [String: KeyCheck] = [:]
    var speech: Speech = .none
    var speechWords: String?
    /// The engine dictation uses: "local", or the id of a connected provider.
    var speechKind = "local"
    /// The core's words when it refused a model change, by agent id (shown on the card).
    var agentRefusals: [String: String] = [:]
    var agents: [AgentConfig] = []
    var draftAgent: AgentConfig?
    /// The draft's id in the core once its name has let it be saved; later edits save the same agent.
    private var draftSaved = false
    var formMissing: String?
    var engine: Engine = .local
    /// What the core holds for Jev's key (`provider_credential/jev/stored`), re-read; never a local flag.
    var jevConnected = false
    /// Jev was chosen before its key was stored: the choice is saved the moment the key is accepted.
    var pendingJev = false
    /// The core's words when it refused the engine choice.
    var engineRefusal: String?
    /// The owner agreed to send mail text to the hosted model (the core's record `decision_consent/jev`, read back).
    var jevConsent = false
    /// The core's word for Jev's key (`ai_provider/jev/state`): "saved, not checked" until a provider check exists; nil when the core says nothing.
    var jevState: AiProviderState?
    /// What the Jev chip shows: the core's own word, never a green "connected" for a key that was only saved.
    var jevChip: String {
        if isChecking("jev") { return AIWords.state(.checking) }
        if let s = jevState { return AIWords.state(s) }
        return AIWords.state(jevConnected ? .connected : .notConnected)
    }
    /// The Laya weights on this Mac, as the label inspector reads them (LayaOnDevice.shared.state).
    var layaState: LayaModelState = .notDownloaded
    var pages = DecisionCatalog.shipped()
    var previews: [String: [PreviewRow]] = [:]
    /// The core's sentence for a preview that is empty or was cut short (`decision_preview/words`), by page; nil = it has none.
    var previewNotes: [String: String] = [:]
    var applied: Set<String> = []
    /// Where each page's preview stands: idle, running (the core is asked off the main actor), done, refused. The page's socket serves it (`page.<id>.preview.state`) so a check waits on it, never on time.
    enum PreviewState: String { case idle, running, done, refused }
    var previewStates: [String: PreviewState] = [:]
    @ObservationIgnored private var previewRuns: [String: Task<Void, Never>] = [:]
    /// Which run of a page may still land: a run that finds its number changed (the page closed, the list edited, a reset) is dropped.
    @ObservationIgnored private var previewGeneration: [String: Int] = [:]
    func previewState(_ id: String) -> PreviewState { previewStates[id] ?? .idle }
    /// How many runs were started (a second click while one runs starts none): a seam for the unit tests, read by nothing else.
    @ObservationIgnored private(set) var previewStarts = 0
    /// The page whose preview ran last: `decisions.result` reads that run's verdicts and why each message was decided as it was.
    var lastPreviewPage: String?
    /// The rule "Add a rule" opened last (page and entry): `decisions.rule-editor` reads it.
    var ruleDraft: (page: String, entry: UUID)?
    var proposals: [String: String] = [:]
    private var labelsSeeded = false
    /// hid-ai: the last message `fixture decide` ran through a page's list (the test engine's sample, FixtureDecide.swift): `decisions.result`
    /// reads it until the next preview. `decideRefusal` holds the test engine's words when it refused (an unknown message or page).
    struct DecidedMessage { let page: String; let message: String; let decision: FixtureDecision }
    var lastDecision: DecidedMessage?
    var decideRefusal: String?

    var sectionName: String {
        switch section { case .providers: "providers"; case .agents: "agents"; case .decisions: "decisions"; case .page(let id): "page.\(id)" }
    }

    var anyConnected: Bool { providers.contains { $0.connected } }

    private var core: WhooshCore { AppModel.shared.core }

    /// Providers and agents as the core holds them (a provider the core has no key door for stays "not connected").
    func syncFromCore() {
        let live = core.aiProviders()
        providers = Self.shownProviders.map { id, name in
            guard var p = live.first(where: { $0.id == id }) else { return AiProvider(id: id, name: name, models: [], connected: false, keychain: nil) }
            if !p.connected { p.models = [] }
            return p
        }
        if case .cloud(let pid) = core.speechEngine().kind { speechKind = pid } else { speechKind = "local" }
        syncDecision()
        agents = core.agentConfigs().filter { $0.id != draftAgent?.id || !draftSaved }
    }

    /// The decision model as the core holds it: the pane draws this and keeps nothing of its own (a relaunch shows the same).
    func syncDecision() {
        switch core.decisionModel()?.provider {
        case .jev?: engine = .jev
        case .rules?: engine = .off
        default: engine = .local
        }
        jevConnected = core.state(path: "provider_credential/jev/stored") == "true"
        jevConsent = core.decisionConsent(provider: .jev)
        jevState = core.state(path: "ai_provider/jev/state")
            .flatMap { try? JSONSerialization.jsonObject(with: Data($0.utf8), options: [.fragmentsAllowed]) as? String }
            .flatMap { aiProviderStateFromKey(key: $0) }
        if let s = jevState { jevConnected = providerStateIsUsable(s: s) }   // "not checked" is usable: a saved key Jev can be chosen with
        // the on-device model in use, as the core holds it; the port that answers follows the same value (DecisionModels)
        let model = DecisionModels.shared.line(id: core.decisionModel()?.model).id
        if model != chosenModel { chosenModel = model }
        DecisionModels.shared.choose(model)
        // The Laya line is NOT read here: this runs for socket reads of other rows, and a read must never change what the screen shows.
    }

    /// The engine the segmented control shows: Jev while its key is still being asked for.
    var shownEngine: Engine { pendingJev ? .jev : engine }

    /// Laya, Jev or rules only: `SaveDecisionModel`, then the pane re-reads. Jev without a stored key is refused by the core in words;
    /// the pane then waits for the key (`pendingJev`) and saves the choice when the key is accepted.
    func chooseEngine(_ e: Engine) {
        engineRefusal = nil
        var m = core.decisionModel() ?? DecisionModel(provider: .local, model: nil, keychain: nil, decisionsEnabled: [], questionWords: [:], lastRunMs: nil)
        switch e {
        case .local: m.provider = .local
        case .jev:
            m.provider = .jev
            m.keychain = core.aiProviders().first { $0.id == "jev" }?.keychain ?? KeychainRef(service: "whoosh.provider", account: "jev")
        case .off: m.provider = .rules
        }
        do { _ = try core.run(command: .saveDecisionModel(model: m)); pendingJev = false }
        catch CoreError.Invalid(_, let words) { engineRefusal = words; pendingJev = e == .jev }
        catch { engineRefusal = AIWords.refusedWords; pendingJev = e == .jev }
        dropAllPreviews(); previews = [:]; previewNotes = [:]
        syncDecision()
    }

    /// Round 12: Apple's on-device model judges the Labels question when the system says it is available (LabelJudgeRouter): the same test
    /// the router makes (AppleLabelJudge.unavailableReason), so the Engine row never names a model that does not sort.
    static var labelsByApple: Bool { AppleLabelJudge().unavailableReason() == nil }

    /// The weights' size the row names before the server has answered (decimal, as the file system shows it): proposed as a CONSTANTS row, LAYA_MODEL_BYTES.
    static let layaBytes: Int64 = Int64(DecisionModelCatalogue.laya.downloadMB) * 1_000_000   // the catalogue's line holds the size (ledger DECISION_MODEL_LAYA_MB)

    /// The Cancel button of the Laya row: stops the running download for every surface that shows it.
    func cancelLaya() { modelStore().cancel(); layaTask?.cancel(); layaTask = nil; layaStateChanged() }

    /// The download the Download button started, until it returns (round 6). Between the click and the store's first step the disk still says
    /// "not downloaded": a re-read then must not take the row back off "downloading" (main92 sweep-settingsai laya-cancel-stops-the-download: Cancel
    /// right after Download found the row not downloading and did nothing), and Cancel stops a download that has not begun yet.
    @ObservationIgnored private var layaTask: Task<Void, Never>?

    /// The Download door of the Laya row: the same one as the label inspector's. Both read `LayaOnDevice.shared.state`; the store joins a
    /// download that is already running, so a second click changes nothing but the progress.
    ///
    /// The click first reads what the disk says: a model already there shows "Ready on this Mac" at once and nothing is asked of the server.
    /// When the download call returns (ready, failed, cancelled, or joined to one already running) the screen reads again: the store announces
    /// only a CHANGE, and a call that returns at once changes nothing, so the line would stay on "downloading" for ever.
    func downloadLaya() {
        layaStateChanged()
        switch layaState {
        case .downloading, .ready: return
        case .notDownloaded, .failed: break
        }
        layaState = .downloading(received: 0, total: -1)   // the row changes on the click, not at the first byte
        let store = modelStore()
        layaTask = Task { [weak self] in
            _ = await store.download()
            self?.layaTask = nil
            self?.layaStateChanged()
        }
    }

    /// The store moved (any surface), or the page opened: re-read the one state, then the other lines of the list.
    func layaStateChanged() {
        let now = modelStore().refresh()
        var keep = false
        if layaTask != nil, case .notDownloaded = now, case .downloading = layaState { keep = true }   // the click's download has not reached the store yet
        if !keep { layaState = now }
        refreshModels()
    }

    // MARK: the list of on-device models (SPEC A133; the owner's pick M1, one line per model)

    /// The model in use, as the core holds it (`DecisionModel.model`): none saved, or one this build does not know, is the catalogue's first line.
    var chosenModel = DecisionModelCatalogue.laya.id   // the first line of `DecisionModels.shared.lines` until the core's choice is read
    /// The model the owner asked for whose files are not on this Mac yet: it becomes the model in use when they are. Until then the model in
    /// use keeps sorting and its line says "In use until … is ready." (pick M1).
    var wantedModel: String?
    /// Where each line other than Laya's stands, re-read when a store announces a change, never by a socket read.
    var modelStates: [String: LayaModelState] = [:]
    @ObservationIgnored private var modelTasks: [String: Task<Void, Never>] = [:]
    /// Moves when a line is added or removed (or a reset re-reads the added ones): the list reads `modelLines`, so the card redraws its lines.
    private var linesRevision = 0
    /// Every line of the list (`DecisionModels.shared.lines`), read through the store so a view that draws them follows an add or a Remove.
    var modelLines: [DecisionModelEntry] { _ = linesRevision; return DecisionModels.shared.lines }

    func modelState(_ entry: DecisionModelEntry) -> LayaModelState {
        entry.family == .laya ? layaState : (modelStates[entry.id] ?? .notDownloaded)
    }

    /// The size a line names before the server has answered (the catalogue's figure, decimal as the file system shows it).
    func modelBytes(_ entry: DecisionModelEntry) -> Int64 { Int64(entry.downloadMB) * 1_000_000 }

    private func refreshModels() {
        for entry in DecisionModels.shared.lines where entry.family != .laya {
            let now = DecisionModels.shared.state(of: entry)
            if modelTasks[entry.id] != nil, case .notDownloaded = now, case .downloading? = modelStates[entry.id] { continue }   // as Laya's click
            if modelStates[entry.id] != now { modelStates[entry.id] = now }
        }
        if let wanted = wantedModel, case .ready = modelState(DecisionModels.shared.line(id: wanted)) { useModel(wanted) }
    }

    /// The Download door of a line. A model that is not the one in use becomes the model in use when its files are here (`wantedModel`).
    func downloadModel(_ id: String) {
        let entry = DecisionModels.shared.line(id: id)
        guard entry.id == id else { return }
        layaStateChanged()
        if case .ready = modelState(entry) { if id != chosenModel { useModel(id) }; return }
        if id != chosenModel { wantedModel = id }
        if entry.family == .laya { downloadLaya(); return }
        if case .downloading = modelState(entry) { return }
        modelStates[id] = .downloading(received: 0, total: modelBytes(entry))   // the line changes on the click, not at the first byte
        modelTasks[id] = Task { [weak self] in
            _ = await DecisionModels.shared.download(entry)
            self?.modelTasks[id] = nil
            self?.layaStateChanged()
        }
    }

    /// The Cancel button of a line: the download stops, and a model that was only asked for is no longer waited for.
    func cancelModel(_ id: String) {
        let entry = DecisionModels.shared.line(id: id)
        guard entry.id == id else { return }
        if wantedModel == id { wantedModel = nil }
        if entry.family == .laya { cancelLaya(); return }
        DecisionModels.shared.cancelDownload(entry)
        modelTasks[id]?.cancel(); modelTasks[id] = nil
        modelStates[id] = .notDownloaded
        layaStateChanged()
    }

    /// The circle of a line: a model on this Mac becomes the model in use at once; one that is not is downloaded first.
    func chooseModel(_ id: String) {
        guard id != chosenModel else { return }
        downloadModel(id)
    }

    /// The button Remove of an added line (not in use, not downloading): its files and its entry go, and the list re-reads.
    func removeModel(_ id: String) {
        guard DecisionModels.shared.isAdded(id), id != chosenModel else { return }
        if case .downloading = modelState(DecisionModels.shared.line(id: id)) { return }
        if wantedModel == id { wantedModel = nil }
        modelTasks[id]?.cancel(); modelTasks[id] = nil
        DecisionModels.shared.remove(id)
        modelStates[id] = nil
        linesRevision += 1
        layaStateChanged()
    }

    // MARK: the line "Other model" (the owner's pick M1, its last line): a Hugging Face address, checked, becomes a line of the list

    /// The id the address field types into (`keyDrafts`, `keyFocus`: the socket's `type` verb fills it as it fills a key field).
    static let otherField = "other-model"
    /// Add was pressed: the line shows the field and Check.
    var otherOpen = false
    /// Why the last address was refused, in one sentence (the amber line under the field); nil = none.
    var otherRefusal: String?
    /// The hub is being asked.
    var otherChecking = false
    @ObservationIgnored private var otherTask: Task<Void, Never>?
    /// The address as typed.
    var otherAddress: String {
        get { keyDrafts[Self.otherField] ?? "" }
        set { keyDrafts[Self.otherField] = newValue }
    }

    /// The button Add: the line turns into the field, which takes the keys.
    func openOther() {
        otherOpen = true; otherRefusal = nil; keyFocus = Self.otherField
    }

    /// Escape: the field goes, with what was typed and any refusal; a check in flight lands nowhere.
    func closeOther() {
        otherTask?.cancel(); otherTask = nil; otherChecking = false
        otherOpen = false; otherRefusal = nil; keyDrafts[Self.otherField] = nil
        if keyFocus == Self.otherField { keyFocus = nil }
    }

    /// The button Check (Return): the address is read, the hub asked, and a model the app runs becomes the last line; any other is refused in one sentence.
    func checkOther() {
        guard otherOpen, !otherChecking else { return }
        otherRefusal = nil
        guard let address = ModelHub.address(otherAddress) else { otherRefusal = AIWords.hubRefusal(.notAnAddress); return }
        if DecisionModels.shared.holds(address) { otherRefusal = AIWords.hubRefusal(.already); return }
        otherChecking = true
        otherTask = Task { [weak self] in
            let answer = await ModelHub.check(address)
            guard let self, !Task.isCancelled else { return }
            self.otherChecking = false; self.otherTask = nil
            switch answer {
            case .failure(let why): self.otherRefusal = AIWords.hubRefusal(why)
            case .success(let found):
                // false: the same model became a line while the hub was asked, or added-models.json could not be written (the field stays, unchanged)
                guard DecisionModels.shared.add(found) else {
                    if DecisionModels.shared.holds(address) { self.otherRefusal = AIWords.hubRefusal(.already) }
                    return
                }
                self.linesRevision += 1
                self.closeOther()
                self.layaStateChanged()
            }
        }
    }

    /// `SaveDecisionModel` with the line's id: the core keeps the choice, the pane re-reads it, and the port answers with that model.
    private func useModel(_ id: String) {
        engineRefusal = nil
        if wantedModel == id { wantedModel = nil }
        var m = core.decisionModel() ?? DecisionModel(provider: .local, model: nil, keychain: nil, decisionsEnabled: [], questionWords: [:], lastRunMs: nil)
        m.provider = .local; m.model = id
        do { _ = try core.run(command: .saveDecisionModel(model: m)) }
        catch CoreError.Invalid(_, let words) { engineRefusal = words }
        catch { engineRefusal = AIWords.refusedWords }
        dropAllPreviews(); previews = [:]; previewNotes = [:]
        syncDecision()
    }

    /// Jev's key goes through SaveProviderKey like any provider's (the core stores it in the Keychain and asks nothing the pane sees).
    func connectJev(_ key: String) { paste(key, into: "jev") }

    /// Jev's Test button (DECISION-MODEL.md:63, "a Test button that sends one fixed sample"): `Command.testProviderKey` on either engine, asked off the
    /// main thread; the core answers at once and the sample's answer lands on `ai_provider/jev/state` ("checking", then "connected" or "refused"),
    /// read back by the same bounded wait as a pasted key (`landed`). The line under the key says the refusal in a catalogue sentence (`jevWords`).
    func testJev() {
        guard checks["jev"] == nil else { return }
        let check = KeyCheck()
        checks["jev"] = check
        refusals["jev"] = nil
        let core = self.core
        Task.detached(priority: .userInitiated) {
            do { _ = try core.run(command: .testProviderKey(providerId: "jev")); check.finish(AISettingsStore.landed(core, "jev")) }
            catch { check.finish(error) }
            await MainActor.run { AISettingsStore.shared.settle("jev") }
        }
    }

    /// "don't mix languages": Jev's refusal is shown in a catalogue sentence keyed on what failed, never the engine's or Jev's English words.
    /// field "key" = no key saved yet; anything else (Jev said no, no answer, an answer that cannot be read) = the key was not taken.
    static func jevWords(_ e: Error) -> String {
        if case CoreError.Invalid("key", let words)? = e as? CoreError, words == jevNoKeyWords { return AIWords.pasteJevKeyFirst }
        return AIWords.refusedWords
    }
    /// The engine's "no key" words (ai_settings.rs JEV_NO_KEY_WORDS, the test engine's FixtureDecide.swift): the one refusal told apart by its words,
    /// because a landed refusal also arrives as field "key" (`landed`).
    nonisolated static let jevNoKeyWords = "Paste the Jev key first."

    // MARK: teaching (DECISION-MODEL.md v2 §3): read from the core's `decision_teaching/...` paths; the offers follow from them
    /// The examples from which Laya may be refitted on this Mac (v2 §3: "after 35 examples"); proposed CONSTANTS row DECISION_REFIT_EXAMPLES.
    static let refitAt = 35
    struct Teaching: Equatable { let corrections: Int; let shared: String?; let offers: [String] }
    func teaching(_ page: String) -> Teaching {
        func number(_ p: String) -> Int { Int(core.state(path: p) ?? "") ?? 0 }
        let n = number("decision_teaching/\(page)/corrections")
        let shared = core.state(path: "decision_teaching/\(page)/shared")
            .flatMap { try? JSONSerialization.jsonObject(with: Data($0.utf8), options: [.fragmentsAllowed]) as? String }
            .flatMap { $0.isEmpty ? nil : $0 }
        var offers: [String] = []
        if n > 0, shared != nil { offers.append("make-rule") }                                    // the corrections share a sender, a domain or a list
        if engine == .local, number("decision_teaching/examples") >= Self.refitAt { offers.append("refit") }   // Laya only: Jev is never refitted
        return Teaching(corrections: n, shared: shared, offers: offers)
    }

    /// "Make a rule from these": a rule naming what the corrections share goes first in the list, its editor open on it.
    func ruleFromCorrections(page id: String) -> Bool {
        guard let p = pageIndex(id), let who = teaching(id).shared else { return false }
        pages[p].entries.insert(.init(.rule, AIWords.fromSender(who.contains("@") ? who : "@" + who), outcome: pages[p].entries.last?.outcome ?? "", match: "from:\(who)"), at: 0)
        ruleDraft = (id, pages[p].entries[0].id)
        forgetPreview(id)
        return true
    }

    /// The sentence under a page's teaching banner after "Refit the model": the engine's refusal in a catalogue sentence; nil = none.
    var refitNotes: [String: String] = [:]

    /// "Refit the model": `Command.refitDecisionModel` on either engine. No refit is built yet (fix-teachcore: no calibration fit exists), so the
    /// engine refuses every ask; the banner says why in a catalogue sentence keyed on the refusal (field or code), never that a refit happened.
    func refit(page id: String) -> Bool {
        guard teaching(id).offers.contains("refit") else { return false }
        do { _ = try core.run(command: .refitDecisionModel); refitNotes[id] = nil }
        catch { refitNotes[id] = refitWords(error) }
        return true
    }

    /// The engine's refit refusal as a catalogue sentence: the model in force (field "provider"), too few examples (field "examples"), or not built yet
    /// (code engine.not_built_yet, and any other failure).
    func refitWords(_ e: Error) -> String {
        switch e as? CoreError {
        case .Invalid("provider", _)?: return engine == .jev ? AIWords.jevNotRefitted : AIWords.modelOffNoRefit
        case .Invalid("examples", _)?:
            let have = Int(core.state(path: "decision_teaching/examples") ?? "") ?? 0
            return AIWords.refitNeeds(Self.refitAt, have)
        default: return AIWords.refitNotOnThisMac
        }
    }

    /// Allow / Withdraw: `GiveDecisionConsent` for Jev, then the pane re-reads the core's record. Jev waiting on its consent is saved the moment it is given.
    func setConsent(_ given: Bool) {
        do { _ = try core.run(command: .giveDecisionConsent(provider: .jev, given: given)); engineRefusal = nil }
        catch CoreError.Invalid(_, let words) { engineRefusal = words }
        catch { engineRefusal = AIWords.refusedWords }
        syncDecision()
        if given, pendingJev, jevConnected { chooseEngine(.jev) }
    }

    /// `fixture reset`: the keys, the agents the owner made and the open draft are forgotten, here and in the fixture core.
    func resetFromFixture() {
        CoreHost.shared.fixture?.resetAISettings()
        settleAll()
        pendingJev = false; engineRefusal = nil
        dropAllPreviews(); previews = [:]; previewNotes = [:]   // a preview in flight lands nowhere, and the page starts idle
        refusals = [:]; keyDrafts = [:]; keyFocus = nil; questionFocus = nil; draftAgent = nil; draftSaved = false; formMissing = nil
        speech = .none; speechWords = nil
        lastDecision = nil; decideRefusal = nil; refitNotes = [:]
        // round 6: the decision lists are the launch state again (main92 decision-check: a rule added, an entry moved or a rule made by an earlier case
        // stayed, so J5.8/unticking-vip-removes-it-from-the-preview found the VIP rule no longer entry 1 and jev-body read entry 4)
        pages = DecisionCatalog.shipped(); labelsSeeded = false; proposals = [:]; ruleDraft = nil; lastPreviewPage = nil; applied = []
        FixtureTeaching.reset()
        SignInController.shared.resetChatGPTForFixture()   // round 9: the ChatGPT row is not connected again
        modelStore().forgetFailure()   // hid-ai: a reset is the launch state: a failed download's sentence goes, the disk decides
        wantedModel = nil; modelTasks.values.forEach { $0.cancel() }; modelTasks = [:]; modelStates = [:]   // the list too: nothing asked for, the disk decides
        // the added lines are the disk's too (added-models.json, kept across launches): a reset re-reads the file and keeps them; the door is closed
        DecisionModels.shared.reloadAdded(); linesRevision += 1; closeOther()
        layaStateChanged()   // a fixture verb, not a read: the line follows the store again
        syncFromCore()
    }

    // MARK: providers: SaveProviderKey; the core asks the provider and keeps the key (state shows `stored` only)
    func isChecking(_ id: String) -> Bool { checks[id] != nil }

    /// The Connect button: the typed key goes to the provider, the field empties.
    func connect(_ id: String) {
        let key = keyDrafts[id] ?? ""
        keyDrafts[id] = nil; keyFocus = nil
        paste(key, into: id)
    }

    /// The provider is asked OFF the main thread: the row says "checking" at once and the answer lands when it comes.
    func paste(_ key: String, into id: String) {
        guard checks[id] == nil else { return }
        let check = KeyCheck()
        checks[id] = check
        refusals[id] = nil
        let core = self.core
        Task.detached(priority: .userInitiated) {
            do { _ = try core.run(command: .saveProviderKey(providerId: id, key: SecretText(value: key))); check.finish(AISettingsStore.landed(core, id)) }
            catch { check.finish(error) }
            await MainActor.run { AISettingsStore.shared.settle(id) }
        }
    }

    /// The real core answers SaveProviderKey at once; the provider's answer lands on `ai_provider/<id>/state`: "checking", then "connected", or
    /// "refused" with the provider's words at `ai_provider/<id>/refusal`. nil = accepted, or a core that answers after its own check and serves no
    /// such path (the fixture core). Runs in the detached task of `paste`, never on the main actor. The core emits no event for it, so this is a
    /// bounded re-read (every 50 ms, never longer than twice the core's own check bound AI_HTTP_TIMEOUT_MS), not a timer that lives on.
    nonisolated static func landed(_ core: WhooshCore, _ id: String) -> Error? {
        let path = "ai_provider/\(id)/state"
        let until = Date().addingTimeInterval(Double(2 * WhooshConstants.AI_HTTP_TIMEOUT_MS) / 1000)
        while core.state(path: path) == "\"checking\"", Date() < until { Thread.sleep(forTimeInterval: 0.05) }
        guard core.state(path: path) == "\"refused\"" else { return nil }
        let words = core.state(path: "ai_provider/\(id)/refusal")
            .flatMap { try? JSONSerialization.jsonObject(with: Data($0.utf8), options: [.fragmentsAllowed]) as? String }
        return CoreError.Invalid(field: "key", reasonWords: words ?? AIWords.refusedWords)
    }

    /// Takes a finished check's answer (once: a second call finds nothing).
    func settle(_ id: String) {
        guard let check = checks[id], check.isDone else { return }
        checks[id] = nil
        if let e = check.error {
            if id == "jev" { refusals[id] = Self.jevWords(e) }   // hid-ai round 4: Jev's refusal in a catalogue sentence ("don't mix languages")
            else if case CoreError.Invalid(_, let words)? = e as? CoreError { refusals[id] = words }
            else if case CoreError.NotFound? = e as? CoreError { refusals[id] = AIWords.noKeyDoorYet }
            else { refusals[id] = AIWords.refusedWords }
        } else {
            refusals[id] = nil
            if id == "jev", pendingJev { chooseEngine(.jev) }
        }
        syncFromCore()
    }

    /// For the control socket's row reads and `fixture` verbs only (a test reads the answer, not "checking"): waits for the checks
    /// in flight. No view and no click door calls this.
    func settleAll() {
        for (id, check) in checks { check.wait(); settle(id) }
    }

    func state(of id: String) -> String {
        if checks[id] != nil { return AIWords.state(.checking) }
        if refusals[id] != nil { return AIWords.state(.refused) }
        return AIWords.state(providers.first { $0.id == id }?.connected == true ? .connected : .notConnected)
    }

    // MARK: speech: ChooseSpeechEngine; ready, or failed in the core's words (Retry asks again)
    func chooseLocalSpeech() { chooseSpeech(provider: nil) }

    /// nil = on this Mac; else the connected provider's id. The core's refusal words come back in the line under the pill.
    func chooseSpeech(provider: String?) {
        do {
            _ = try core.run(command: .chooseSpeechEngine(kind: provider.map { SpeechEngineKind.cloud(providerId: $0) } ?? SpeechEngineKind.local))
            speech = .ready; speechWords = nil; speechKind = provider ?? "local"
        } catch CoreError.Invalid(_, let words) {
            speech = .failed; speechWords = words
        } catch {
            speech = .failed; speechWords = AIWords.speechFailed
        }
    }

    /// The key that starts dictation: the active preset's own chord, else the hold key the dictation engine listens for.
    var dictationKey: String? { ShortcutEngine.shared.chord(for: "dictate") ?? Self.holdToDictateKey }
    static let holdToDictateKey = "Fn"   // DictationFeature.keyBindings: "Fn:down" / "Fn:up"

    func openShortcuts() { _ = SettingsWindowModel.shared.select(.shortcuts) }

    // MARK: agents: SaveAgentConfig; a draft with a name is saved as it changes (no Save press is needed for the core to hold it)
    func newAgent() {
        syncFromCore()
        keyFocus = nil
        guard let p = providers.first(where: { $0.connected }) else { formMissing = AIWords.missingProvider; return }
        formMissing = nil
        draftSaved = false
        draftAgent = AgentConfig(id: UUID().uuidString, name: "", purpose: "", providerId: p.id, model: p.models.first ?? "",
                                 instructions: "", allowed: [], builtIn: false)
    }

    /// Text the socket's `type` verb sends while the pane holds the keys: the draft's name field takes it in the same turn, and the
    /// shared buffer is never read (a read of the rows or a closed window can no longer take what another surface was typed).
    /// False when no draft is open: the text goes on to the next field.
    func typeIntoDraft(_ text: String) -> Bool {
        if let id = keyFocus { keyDrafts[id, default: ""] += text; return true }   // a provider's key field was clicked last
        if let q = questionFocus {   // a model entry's question was clicked last: the words are added word for word (what is sent is what you read)
            update(page: q.page, entry: q.entry) { $0.text += text }
            return true
        }
        guard var d = draftAgent else { return false }
        d.name += text
        draftAgent = d
        persistDraft()
        return true
    }

    /// The draft first, then the agents the core holds: the cards in the order the pane draws them.
    var cards: [AgentConfig] { (draftAgent.map { [$0] } ?? []) + agents }

    /// What the model menu offers: each connected provider's models, then one greyed line per provider with no key.
    struct ModelChoice {
        let provider: AiProvider
        let model: String?
        var enabled: Bool { model != nil }
    }
    func modelChoices() -> [ModelChoice] {
        providers.flatMap { p -> [ModelChoice] in
            p.connected && !p.models.isEmpty ? p.models.map { ModelChoice(provider: p, model: $0) } : [ModelChoice(provider: p, model: nil)]
        }
    }

    /// A choice in the model menu: the agent is saved with the new provider and model at once (the core's words show on the card if it refuses).
    func setModel(_ id: String, to c: ModelChoice) {
        guard let model = c.model else { return }
        agentRefusals[id] = nil
        if var d = draftAgent, d.id == id {
            d.providerId = c.provider.id; d.model = model; draftAgent = d; persistDraft(); return
        }
        guard var a = agents.first(where: { $0.id == id }) else { return }
        a.providerId = c.provider.id; a.model = model
        do { _ = try core.run(command: .saveAgentConfig(config: a)) }
        catch CoreError.Invalid(_, let words) { agentRefusals[id] = words }
        catch { agentRefusals[id] = AIWords.refusedWords }
        syncFromCore()
    }

    /// The click on the draft's name field: the typed text goes there again.
    func focusDraftName() -> Bool { keyFocus = nil; questionFocus = nil; return draftAgent != nil }

    func toggle(_ p: AgentPermission, on id: String) {
        mutateAgent(id) { a in
            if let i = a.allowed.firstIndex(of: p) { a.allowed.remove(at: i) } else { a.allowed.append(p) }
        }
    }

    func mutateAgent(_ id: String, _ f: (inout AgentConfig) -> Void) {
        if var d = draftAgent, d.id == id { f(&d); draftAgent = d; persistDraft(); return }
        if var a = agents.first(where: { $0.id == id }) {
            f(&a)
            if (try? core.run(command: .saveAgentConfig(config: a))) != nil { syncFromCore() }
        }
    }

    /// Saves the open draft to the core when it has a name (the core refuses an unnamed agent); the first save gives it the core's id.
    private func persistDraft() {
        guard var d = draftAgent, !d.name.trimmingCharacters(in: .whitespaces).isEmpty else { return }
        if !draftSaved { d.id = "" }
        guard let out = try? core.run(command: .saveAgentConfig(config: d)), let id = out.createdId ?? (draftSaved ? d.id : nil) else { return }
        if !draftSaved { draftAgent?.id = id; draftSaved = true }
        syncFromCore()
    }

    func saveDraft() {
        guard var d = draftAgent else { return }
        if d.name.trimmingCharacters(in: .whitespaces).isEmpty { d.name = AIWords.untitledAgent; draftAgent = d }
        persistDraft()
        draftAgent = nil; draftSaved = false
        syncFromCore()
    }

    func duplicate(_ a: AgentConfig) {
        let c = AgentConfig(id: "", name: AIWords.copyOf(a.name), purpose: a.purpose, providerId: a.providerId,
                        model: a.model, instructions: a.instructions, allowed: a.allowed, builtIn: false)
        if (try? core.run(command: .saveAgentConfig(config: c))) != nil { syncFromCore() }
    }

    // MARK: decisions
    func pageIndex(_ id: String) -> Int? { pages.firstIndex { $0.id == id } }

    /// The Labels page's outcomes are nodes of the core's tree: the shipped sample rule is re-hung on the real node
    /// with the same name, and the model's proposed child sits under a real node. Runs once, on first read.
    func seedLabels() {
        guard !labelsSeeded, let p = pageIndex("labels") else { return }
        let tree = AILabelTree.nodes()
        guard !tree.isEmpty else { return }
        labelsSeeded = true
        for i in pages[p].entries.indices where pages[p].entries[i].kind == .rule {
            pages[p].entries[i].outcome = AILabelTree.resolve(pages[p].entries[i].outcome, in: tree)
        }
        proposals["labels"] = AILabelTree.proposal(in: tree)
    }

    /// Moves an entry; the default stays last whatever is dropped on it.
    func move(page id: String, from: Int, to: Int) {
        guard let p = pageIndex(id) else { return }
        var es = pages[p].entries
        let last = es.count - 1
        guard from != to, from < last, (0..<es.count).contains(to) else { return }
        let e = es.remove(at: from)
        es.insert(e, at: min(to, last - 1))
        pages[p].entries = es
        forgetPreview(id)
    }

    func update(page id: String, entry: UUID, _ f: (inout DecisionEntry) -> Void) {
        guard let p = pageIndex(id), let i = pages[p].entries.firstIndex(where: { $0.id == entry }) else { return }
        f(&pages[p].entries[i])
        forgetPreview(id)
    }

    func addRule(page id: String) {
        guard let p = pageIndex(id) else { return }
        let outcome = id == "labels" ? (AILabelTree.nodes().first ?? AIWords.pickLabel) : pages[p].entries.last?.outcome ?? ""
        pages[p].entries.insert(.init(.rule, "subject contains …", outcome: outcome, match: "subject:…"), at: 0)
        ruleDraft = (id, pages[p].entries[0].id)
    }

    /// hid-ai: `fixture add-rule <page> <words> <outcome>`: the rule goes first in the list, as "Create a rule" puts it (docs/DECISION-MODEL.md v2 §1).
    func addRule(page id: String, words: String, outcome: String) {
        guard let p = pageIndex(id) else { return }
        pages[p].entries.insert(.init(.rule, words, outcome: outcome, match: FixtureDecideSample.matchKey(words)), at: 0)
        forgetPreview(id)
    }

    /// hid-ai: `fixture decide`: one sample message through the page's list as it stands, with the engine and consent the core holds.
    func decideFromFixture(page id: String, message: String, label: String, confidence: Double) {
        settleAll()   // a key or a consent saved just before is in the core first
        lastDecision = nil; decideRefusal = nil
        guard let p = pageIndex(id), let f = CoreHost.shared.fixture else { return }   // no such page, or not the test engine: no answer, the result row stays empty
        do {
            let d = try f.fixtureDecide(page: id, message: message, entries: DecisionPreview.entries(of: pages[p]), label: label, confidence: confidence)
            lastDecision = DecidedMessage(page: id, message: message, decision: d)
        } catch CoreError.Invalid(_, let words) { decideRefusal = words } catch { decideRefusal = String(describing: error) }
    }

    func addModel(page id: String) {
        guard let p = pageIndex(id), !pages[p].entries.contains(where: { $0.kind == .model }) else { return }
        let q = id == "labels" ? "Which label fits this email? Pick any node of the tree, or propose a new child." : "Ask the model"
        pages[p].entries.insert(.init(.model, q, outcome: "the answer", trust: 80), at: pages[p].entries.count - 1)
    }

    /// The preview asks the core OFF the main actor: the page says it is working (state `running`, the pane's own "checking" word) and stays responsive; the answer lands
    /// on the main actor. A second click while one runs starts nothing. A run whose page was closed (the open section is no longer the page) or whose list was edited meanwhile is dropped when it lands.
    func runPreview(page id: String) {
        guard let p = pageIndex(id), previewState(id) != .running else { return }
        lastPreviewPage = id
        lastDecision = nil
        previewStates[id] = .running
        previewStarts += 1
        previews[id] = nil; previewNotes[id] = nil; applied.remove(id)
        // the core runs THIS list (as the page holds it now) on the recent mail and keeps the rows; it writes nothing (`decision_preview/applied` stays false until Apply)
        let entries = DecisionPreview.entries(of: pages[p])
        let core = self.core
        previewGeneration[id, default: 0] += 1
        let generation = previewGeneration[id] ?? 0
        previewRuns[id] = Task.detached(priority: .userInitiated) { [weak self] in
            let answer = PreviewAnswer.ask(core, page: id, entries: entries)
            await self?.landPreview(id, generation, answer)
        }
    }

    /// What the detached run brings back (everything it read, so the main actor reads nothing from the core).
    struct PreviewAnswer: Sendable {
        var rows: [DecisionVerdict] = []
        var words: String?
        var refused = false
        /// A seam for the unit tests: whether the last ask ran on the main thread (it must not).
        nonisolated(unsafe) static var lastAskRanOnMainThread = false
        nonisolated static func ask(_ core: WhooshCore, page id: String, entries: [PreviewEntry]) -> PreviewAnswer {
            lastAskRanOnMainThread = Thread.isMainThread
            do {
                _ = try core.run(command: .previewDecisionList(pageId: id, entries: entries))
                return PreviewAnswer(rows: core.decisionPreview(), words: core.state(path: "decision_preview/words").flatMap { $0.isEmpty ? nil : $0 })
            } catch {
                // refused in the core's own words (no number for the bound yet, an unknown page): no rows, and the words where the sentence goes
                if case CoreError.Invalid(_, let words) = error { return PreviewAnswer(words: words, refused: true) }
                return PreviewAnswer(refused: true)
            }
        }
    }

    private func landPreview(_ id: String, _ generation: Int, _ answer: PreviewAnswer) {
        guard previewGeneration[id] == generation, previewStates[id] == .running else { return }   // the list was edited or the engine changed: the result is dropped
        previewRuns[id] = nil
        guard section == .page(id) else { previewStates[id] = .idle; return }   // the page was closed (another section is open): the result is dropped
        previews[id] = DecisionPreview.rows(from: answer.rows)
        previewNotes[id] = answer.words
        previewStates[id] = answer.refused ? .refused : .done
    }

    /// A run in flight that is no longer wanted lands nowhere.
    func dropPreview(page id: String) {
        guard previewStates[id] == .running else { return }
        previewGeneration[id, default: 0] += 1
        previewRuns[id] = nil
        previewStates[id] = .idle
    }

    func dropAllPreviews() { for id in Array(previewStates.keys) { dropPreview(page: id) }; previewStates = [:] }

    /// The list changed: its rows are no longer this list's, and a run in flight is for the old list.
    private func forgetPreview(_ id: String) {
        dropPreview(page: id)
        previews[id] = nil
        applied.remove(id)
    }

    /// For tests only: waits for the run in flight on a page to land (no view and no door calls this; the socket's checks poll `preview.state`).
    func settlePreview(page id: String) async { await previewRuns[id]?.value }

    /// Apply asks the core to write the preview it ran (`ApplyDecisionPreview`): the placements the page showed, nothing when the preview is stale (the core says so in words,
    /// shown where the preview's sentence goes). Only an Apply the core accepted marks the page applied. The Labels page then writes its rules (`writeLabelRules`).
    func apply(page id: String) {
        if core.decisionPreview().isEmpty { _ = try? core.run(command: .previewDecisions) }
        do {
            _ = try core.run(command: .applyDecisionPreview)
        } catch {
            if case CoreError.Invalid(_, let words) = error { previewNotes[id] = words }
            return
        }
        if id == "labels" { writeLabelRules() }
        applied.insert(id)
        previewNotes[id] = nil
    }

    /// The Labels list's rules that file under a node of the tree go to the core as one decision list (`SaveDecisionList`): each rule with the node it names, so the
    /// core files what it catches under that node (also a node whose AI card is off). A rule with no condition the core can read, or whose node the tree lacks, is left out.
    private func writeLabelRules() {
        guard let p = pageIndex("labels") else { return }
        let labels = core.labels()
        var nodeIds: [String] = [], rules: [RuleSet] = []
        for e in pages[p].entries where e.kind == .rule && e.enabled {
            guard let node = labels.first(where: { AILabelTree.display($0) == e.outcome }), let c = Self.condition(e.match) else { continue }
            nodeIds.append(node.id); rules.append(RuleSet(conditions: [c], matchMode: .all))
        }
        guard !nodeIds.isEmpty else { return }
        _ = try? core.run(command: .saveDecisionList(pageId: "labels", nodeIds: nodeIds, rules: rules))
    }

    /// The core's condition for a rule's match key (`DecisionEntry.match`): "vip", "list", "from:<x>", "subject:<x>"; nil for a key the core has no condition for.
    static func condition(_ match: String?) -> RuleCondition? {
        guard let m = match else { return nil }
        if m == "vip" { return .fact(fact: .vipSender, wanted: true) }
        if m == "list" { return .fact(fact: .mailingList, wanted: true) }
        if m.hasPrefix("from:") { return .from(words: String(m.dropFirst(5))) }
        if m.hasPrefix("subject:") { return .subjectContains(words: String(m.dropFirst(8))) }
        return nil
    }

    /// The proposed child becomes a real label: the core's tree gains it under its parent node (a node already there is left alone).
    private func createNode(_ path: String) {
        let labels = core.labels()
        guard !labels.contains(where: { AILabelTree.display($0) == path }) else { return }
        let name = AILabelTree.leaf(path)
        var parentId: String?
        if AILabelTree.depth(path) > 1 {
            let parent = AILabelTree.parent(path)
            guard let node = labels.first(where: { AILabelTree.display($0) == parent }) else { return }
            parentId = node.id
        }
        _ = try? core.run(command: .createLabel(name: name, parentId: parentId))
    }

    func acceptProposal(page id: String) {
        guard let path = proposals[id], let p = pageIndex(id) else { return }
        if id == "labels" { createNode(path) }
        pages[p].entries.insert(.init(.rule, "the model proposed it", outcome: path, match: nil), at: 0)
        proposals[id] = nil
    }
}

/// One key check in flight: done once the core has answered (written before the signal, read after it).
private final class KeyCheck: @unchecked Sendable {
    private let sem = DispatchSemaphore(value: 0)
    private let state = NSLock()
    private var answered = false
    private var failure: Error?
    func finish(_ e: Error?) { state.withLock { failure = e; answered = true }; sem.signal() }
    var isDone: Bool { state.withLock { answered } }
    var error: Error? { state.withLock { failure } }
    func wait() { sem.wait(); sem.signal() }
}
