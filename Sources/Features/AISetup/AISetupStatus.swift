import Foundation
import Observation
#if os(macOS)
import AppKit
#endif

/// The three things the AI features need.
enum AINeed: String, CaseIterable, Sendable { case writing, decision, meaning }

/// Where a need stands: `ready`, `working` (a download or a key check in flight), or `missing` (`failed` is a download that stopped).
enum AINeedState: String, Sendable { case ready, working, missing, failed }

/// The one control that fixes a need: a Settings pane (`SettingsPane` raw value) and the control's accessibility id.
struct AISetupDoor: Equatable, Sendable {
    let pane: String
    let control: String
    let title: String
}

extension SettingsPane {
    /// The needs a Settings pane's result depends on: its sidebar row carries the dot while any is unmet.
    var aiNeeds: [AINeed] {
        switch self { case .decisions: return [.decision]; case .ai: return [.writing, .decision, .meaning]; case .replies: return [.writing, .meaning]; default: return [] }
    }
}

/// One need, ready to draw or to read: the state, one plain sentence, the door, and progress in 0...1 when a download knows it.
struct AINeedStatus: Equatable, Sendable {
    let need: AINeed
    let state: AINeedState
    /// the machine word of the cause: provider-none, provider-checking, provider-refused, laya-missing, laya-downloading, laya-failed, jev-no-key, jev-no-consent, decisions-off, meaning-missing, ok
    let reason: String
    let words: String
    let door: AISetupDoor?
    let progress: Double?
    /// a ready need's one short fact ("Anthropic · 6 models"), empty otherwise
    var detail = ""
    var usable: Bool { state == .ready }
}

/// ONE read model for every AI surface (docs/lanes/firststart-replies.md "AI setup"): what is missing, in one sentence, and the door that fixes it.
/// It reads the core (`aiProviders`, `ai_provider/<id>/state`, `provider_credential/jev/stored`, `decisionModel`, `decisionConsent`), the Laya store's
/// own state and the meaning model's hand-over state (`BackgroundState`). Nothing here touches the network. It re-reads when the app comes to the
/// front, when one of its own actions ends, on a key check's landing (its own bounded re-read) and every second ONLY while a download runs (disk bytes).
@MainActor @Observable
final class AISetupStatus {
    static let shared = AISetupStatus()
    private static let providerIds = ["anthropic", "openai", "gemini", "openrouter"]

    private(set) var needs: [AINeedStatus] = []
    /// Fixture core only: a state the hidden rig sets so a drawing's state can be rendered (`click ai-setup.preview.<need>.<state>`); never on the real core.
    private(set) var preview: [AINeed: AINeedStatus] = [:]
    /// bumps on every change of any need, so a surface can `.onChange(of: revision)`
    private(set) var revision = 0
    @ObservationIgnored private var ticker: Task<Void, Never>?
    @ObservationIgnored private var observing = false
    @ObservationIgnored private var layaRunning = false
    @ObservationIgnored private var layaFailure: String?
    @ObservationIgnored private var layaTask: Task<Void, Never>?
    @ObservationIgnored private var layaCancelled = false

    private init() { refresh(); observe() }

    func status(_ n: AINeed) -> AINeedStatus { needs.first { $0.need == n } ?? Self.compute(n, core: CoreHost.shared.core, laya: .notDownloaded, layaRunning: false) }
    /// the needs that are not ready: what a warning mark lists
    var missing: [AINeedStatus] { needs.filter { !$0.usable } }
    var allReady: Bool { missing.isEmpty }

    private func observe() {
        guard !observing else { return }
        observing = true
        #if os(macOS)
        let center = NotificationCenter.default
        _ = center.addObserver(forName: NSApplication.didBecomeActiveNotification, object: nil, queue: .main) { _ in
            Task { @MainActor in AISetupStatus.shared.refresh() }   // nothing is taken from the Notification
        }
        #endif
    }

    /// Reads the sources again and publishes only a change.
    func refresh() {
        let laya = Self.layaNow(cancelled: layaCancelled && !layaRunning)
        if case .downloading = laya { layaRunning = true }
        let now = AINeed.allCases.map { preview[$0] ?? Self.compute($0, core: CoreHost.shared.core, laya: laya, layaRunning: layaRunning) }
        guard now != needs else { return }
        needs = now; revision += 1
        wantTicker(now.contains { $0.state == .working })
    }

    /// The Laya state without the store's `refresh()` while a download runs (that call re-reads the disk and would answer "not downloaded").
    private static func layaNow(cancelled: Bool) -> LayaModelState {
        let store = LayaOnDevice.shared.modelStore
        if cancelled { return store.refresh() }   // a cancelled download answers from the disk, never from a failure it was told to make
        switch store.state {
        case .downloading, .failed: return store.state
        default: return store.refresh()
        }
    }

    private func wantTicker(_ on: Bool) {
        if !on { ticker?.cancel(); ticker = nil; return }
        guard ticker == nil else { return }
        ticker = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(1))
                guard let self else { return }
                self.refresh()
            }
        }
    }

    // MARK: actions (the existing doors; the step and any surface call these)
    /// A pasted provider key: SaveProviderKey through the AI pane's own door; the provider's answer lands on `ai_provider/<id>/state`.
    func saveKey(provider id: String, _ key: String) {
        AISettingsStore.shared.paste(key, into: id)
        refresh()   // the provider's answer lands later on `ai_provider/<id>/state`; the one-second re-read below runs while it says "checking"
    }

    /// The decision model's weights: `LayaOnDevice.download` with its progress; a second press while one runs does nothing.
    func downloadLaya() {
        guard !layaRunning else { return }
        layaRunning = true; layaFailure = nil; layaCancelled = false
        refresh()
        layaTask = Task { [weak self] in
            let end = await LayaOnDevice.shared.download { _ in Task { @MainActor in AISetupStatus.shared.refresh() } }
            guard let self else { return }
            self.layaRunning = false; self.layaTask = nil
            if case .failed(let r) = end, !self.layaCancelled { self.layaFailure = r }
            self.refresh()
        }
    }

    /// Cancel on a running download: the task is cancelled (the store's URL session follows) and the state reads the disk again.
    func cancelLaya() {
        guard layaRunning else { return }
        layaCancelled = true; layaTask?.cancel(); layaRunning = false; layaTask = nil
        refresh()
    }

    /// The meaning model: the door lane bg-hooks built (`MeaningModelStore.startInBackground`, one download at a time).
    func downloadMeaning() { _ = MeaningModelStore.shared.startInBackground(); refresh() }

    /// Jev: the key (SaveProviderKey for `jev`), then the consent (GiveDecisionConsent), then the choice of Jev as the decision model.
    func connectJev(key: String) { AISettingsStore.shared.connectJev(key); refresh() }
    func giveJevConsent(_ given: Bool) { AISettingsStore.shared.setConsent(given); refresh() }

    /// The fixture rig's states of the drawings: `missing`, `working` (312 MB of 843 MB, 37 %), `ready` ("Anthropic · 6 models"), or `clear`.
    func setPreview(_ n: AINeed, _ state: String) -> Bool {
        guard CoreHost.shared.kind == .fixture else { return false }
        let fresh = Self.compute(n, core: CoreHost.shared.core, laya: .notDownloaded, layaRunning: false)
        switch state {
        case "clear": preview[n] = nil
        case "missing":
            // A rig that holds the model, or a fixture with a provider connected, computes a READY status: the preview then carries the need's own missing reason and
            // sentence, never the ready sentence in the warn colour beside a Download button (main200 picture: "The meaning model is on this Mac." on the search list).
            let own: (reason: String, words: String) = switch n {
            case .writing: ("provider-none", AISetupWords.writingMissing)
            case .decision: ("laya-missing", AISetupWords.layaMissing)
            case .meaning: ("meaning-missing", AISetupWords.meaningMissing)
            }
            let missing = fresh.state == .missing
            preview[n] = AINeedStatus(need: n, state: .missing, reason: missing ? fresh.reason : own.reason, words: missing ? fresh.words : own.words, door: fresh.door, progress: nil)
        case "working": preview[n] = AINeedStatus(need: n, state: .working, reason: "preview-working", words: AISetupWords.downloading(Self.byteWords(312_000_000), Self.byteWords(AISetupWords.layaBytes)), door: nil, progress: 312.0 / 843.0)
        case "ready": preview[n] = AINeedStatus(need: n, state: .ready, reason: "ok", words: AISetupWords.writingReady, door: nil, progress: nil, detail: n == .writing ? AISetupWords.providerModels("Anthropic", 6) : "")
        default: return false
        }
        revision += 1; refresh(); return true
    }

    /// the fixture's `fixture reset`
    func reset() { preview = [:]; layaRunning = false; layaFailure = nil; AISettingsStore.shared.syncFromCore(); refresh() }

    // MARK: the pure part
    static func compute(_ need: AINeed, core: WhooshCore, laya: LayaModelState, layaRunning: Bool) -> AINeedStatus {
        switch need {
        case .writing: return writing(core)
        case .decision: return decision(core, laya)
        case .meaning: return meaning()
        }
    }

    private static func providerState(_ core: WhooshCore, _ id: String) -> AiProviderState? {
        core.state(path: "ai_provider/\(id)/state")
            .flatMap { try? JSONSerialization.jsonObject(with: Data($0.utf8), options: [.fragmentsAllowed]) as? String }
            .flatMap { aiProviderStateFromKey(key: $0) }
    }

    private static func writing(_ core: WhooshCore) -> AINeedStatus {
        let door = AISetupDoor(pane: "ai", control: "mac-settings-ai.providers.section", title: AISetupWords.doorKey)
        let states = providerIds.compactMap { providerState(core, $0) }
        let connected = core.aiProviders().contains { $0.id != "jev" && $0.connected } || states.contains { providerStateIsUsable(s: $0) }
        let who = core.aiProviders().first { $0.id != "jev" && $0.connected }
        if connected { return AINeedStatus(need: .writing, state: .ready, reason: "ok", words: AISetupWords.writingReady, door: nil, progress: nil, detail: who.map { AISetupWords.providerModels($0.name, $0.models.count) } ?? "") }
        if states.contains(.checking) { return AINeedStatus(need: .writing, state: .working, reason: "provider-checking", words: AISetupWords.writingChecking, door: nil, progress: nil) }
        if states.contains(.refused) { return AINeedStatus(need: .writing, state: .missing, reason: "provider-refused", words: AISetupWords.writingRefused, door: door, progress: nil) }
        return AINeedStatus(need: .writing, state: .missing, reason: "provider-none", words: AISetupWords.writingMissing, door: door, progress: nil)
    }

    private static func decision(_ core: WhooshCore, _ laya: LayaModelState) -> AINeedStatus {
        func s(_ st: AINeedState, _ r: String, _ w: String, _ d: AISetupDoor?, _ p: Double? = nil) -> AINeedStatus { AINeedStatus(need: .decision, state: st, reason: r, words: w, door: d, progress: p) }
        switch core.decisionModel()?.provider {
        case .rules?: return s(.missing, "decisions-off", AISetupWords.decisionOff, AISetupDoor(pane: "decisions", control: "mac-settings-ai.decisions.engine", title: AISetupWords.doorTurnOn))
        case .jev?:
            let stored = core.state(path: "provider_credential/jev/stored") == "true" || (providerState(core, "jev").map { providerStateIsUsable(s: $0) } ?? false)
            if !stored { return s(.missing, "jev-no-key", AISetupWords.jevNoKey, AISetupDoor(pane: "decisions", control: "mac-settings-ai.decisions.jev.key", title: AISetupWords.doorJevKey)) }
            if !core.decisionConsent(provider: .jev) { return s(.missing, "jev-no-consent", AISetupWords.jevNoConsent, AISetupDoor(pane: "decisions", control: "mac-settings-ai.decisions.jev.consent.allow", title: AISetupWords.doorConsent)) }
            return s(.ready, "ok", AISetupWords.jevReady, nil)
        default:
            let door = AISetupDoor(pane: "decisions", control: "mac-settings-ai.decisions.local.download", title: AISetupWords.doorDownload)
            switch laya {
            case .ready: return s(.ready, "ok", AISetupWords.layaReady, nil)
            case .notDownloaded: return s(.missing, "laya-missing", AISetupWords.layaMissing, door)
            case .downloading(let got, let total):
                let r = byteWords(got)
                let all = total > 0 ? total : AISetupWords.layaBytes
                return s(.working, "laya-downloading", AISetupWords.downloading(r, byteWords(all)), nil, min(1, Double(got) / Double(all)))
            case .failed: return s(.failed, "laya-failed", laya.words, door)   // the store's own sentence, already in the app's language
            }
        }
    }

    private static func meaning() -> AINeedStatus {
        let door = AISetupDoor(pane: "ai", control: MeaningModelDoor.downloadId, title: AISetupWords.doorMeaning)
        let snap = BackgroundState.shared.snapshot()
        func s(_ st: AINeedState, _ r: String, _ w: String, _ d: AISetupDoor?, _ p: Double? = nil) -> AINeedStatus { AINeedStatus(need: .meaning, state: st, reason: r, words: w, door: d, progress: p) }
        switch MeaningModelStore.shared.refresh() {
        case .downloading(let got, let total):
            let all = total > 0 ? total : 1_525_000_000
            return s(.working, "meaning-downloading", AISetupWords.downloading(byteWords(got), byteWords(all)), nil, min(1, Double(got) / Double(all)))
        case .failed(let why): return s(.failed, "meaning-failed", why, door)
        case .ready: return s(.ready, "ok", AISetupWords.meaningReady, nil)
        case .notDownloaded:
            if snap.embedderHanded { return s(.ready, "ok", AISetupWords.meaningReady, nil) }   // weights found elsewhere (cache, WHOOSH_EMBED_WEIGHTS)
            return s(.missing, "meaning-missing", AISetupWords.meaningMissing, door)
        }
    }

    nonisolated static func byteWords(_ n: Int64) -> String { AttachmentSize.words(n) }
}
