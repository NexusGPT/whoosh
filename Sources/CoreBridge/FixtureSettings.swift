import Foundation

/// The settings side of the fixture core (delta 7): notch and notification settings, account icon and colour, and
/// the one-shot refusals `fixture fail-next <kind> <reason>` arms. Called with FixtureCore's lock held.
final class FixtureSettingsBook {
    static let notchButtons: Set<String> = ["mark-read", "archive", "reply", "delete"]
    static let failKinds: Set<String> = ["notch-setting-save", "notification-setting-save", "speech-download", "send"]
    /// Keys the core can hold: the providers that need one (the on-device one needs none). Gemini and OpenRouter as the real core's
    /// provider table (core/crates/ai-providers/src/table.rs) holds them: the pane shows four provider rows (hid-ai).
    static let credentialIds = ["anthropic", "openai", "gemini", "openrouter", "jev"]

    var notch = NotchSetting(show: true, buttons: ["mark-read", "archive", "reply"], hiddenInScreenShare: true, followFocus: true, listShows: .latest)
    /// The notch's alert settings (owner's picks of 14 Nov 2026): the contract's start values until a save.
    private(set) var alerts = notchAlertsDefault()
    private var notifications: [String: NotificationSetting] = [:]
    private(set) var icons: [String: AccountIconSource] = [:]
    private(set) var colours: [String: String] = [:]
    private var failures: [String: String] = [:]
    private var failureCodes: [String: String] = [:]   // kind -> the armed server code (nil removes it)

    /// Arms the next save of this kind to be refused with `reason`. False for a kind the core does not know.
    func failNext(_ kind: String, reason: String, code: String? = nil) -> Bool {
        guard Self.failKinds.contains(kind) else { return false }
        failures[kind] = reason
        failureCodes[kind] = code
        return true
    }

    /// Takes the armed refusal of `kind`, once: its words and its server code (delta 12).
    func takeFailure(_ kind: String) -> (reason: String, code: String?)? {
        guard let reason = failures.removeValue(forKey: kind) else { return nil }
        return (reason, failureCodes.removeValue(forKey: kind))
    }

    private func refuseIfArmed(_ kind: String, field: String) throws {
        if let reason = failures.removeValue(forKey: kind) { throw CoreError.Invalid(field: field, reasonWords: reason) }
    }

    /// Saves the alert settings whole; the armed refusal of the notch settings (`notch-setting-save`) refuses this save too.
    func setAlerts(_ a: NotchAlerts) throws {
        try refuseIfArmed("notch-setting-save", field: "notch_alerts")
        alerts = a
    }

    func setNotch(_ s: NotchSetting) throws {
        try refuseIfArmed("notch-setting-save", field: "notch_setting")
        if let bad = s.buttons.first(where: { !Self.notchButtons.contains($0) }) {
            throw CoreError.Invalid(field: "buttons", reasonWords: "“\(bad)” is not a notch button; the buttons are mark-read, archive, reply and delete")
        }
        var seen = Set<String>()
        notch = NotchSetting(show: s.show, buttons: s.buttons.filter { seen.insert($0).inserted }, hiddenInScreenShare: s.hiddenInScreenShare, followFocus: s.followFocus, listShows: s.listShows)
    }

    func notification(_ id: String) -> NotificationSetting { notifications[id] ?? NotificationSetting(accountId: id, enabled: true, delivery: .both) }

    func setNotification(_ s: NotificationSetting, known: [String]) throws {
        try refuseIfArmed("notification-setting-save", field: "notification_setting")
        guard known.contains(s.accountId) else { throw CoreError.NotFound(kind: "account", id: s.accountId) }
        notifications[s.accountId] = s
    }

    /// The delivery every account agrees on; "both" when they differ.
    func commonDelivery(_ ids: [String]) -> NotificationDelivery {
        let all = Set(ids.map { notification($0).delivery ?? .both })
        return all.count == 1 ? all.first! : .both
    }

    private var lastLibrary: [String: String] = [:]   // account id -> the last library glyph picked (kept under an upload)

    func setIcon(_ id: String, _ source: AccountIconSource, known: [String]) throws {
        guard known.contains(id) else { throw CoreError.NotFound(kind: "account", id: id) }
        switch source {
        case .library(let iconId):
            if iconId.trimmingCharacters(in: .whitespaces).isEmpty { throw CoreError.Invalid(field: "icon", reasonWords: "pick an icon from the library") }
        case .uploaded(let path):
            guard let size = (try? FileManager.default.attributesOfItem(atPath: path))?[.size] as? Int else {
                throw CoreError.Invalid(field: "icon", reasonWords: "that image could not be read")
            }
            if size > WhooshConstants.ACCOUNT_ICON_MAX_BYTES {
                throw CoreError.Invalid(field: "icon", reasonWords: "that image is too big: the limit is \(WhooshConstants.ACCOUNT_ICON_MAX_BYTES / 1_048_576) MB")
            }
        }
        icons[id] = source
        if case .library(let i) = source { lastLibrary[id] = i }
    }

    func setColour(_ id: String, _ colour: String, known: [String]) throws {
        guard known.contains(id) else { throw CoreError.NotFound(kind: "account", id: id) }
        let c = colour.trimmingCharacters(in: .whitespaces)
        let hex = c.dropFirst()
        guard c.hasPrefix("#"), hex.count == 6, hex.allSatisfy({ $0.isHexDigit }) else {
            throw CoreError.Invalid(field: "colour", reasonWords: "a colour is written #RRGGBB, for example #2B8A3E")
        }
        colours[id] = c.uppercased()
    }

    /// The library glyph the lists show: the owner's library pick, else the fixture's own. An upload leaves it as it was
    /// (the image is `iconFile`), so the glyph is there again when the owner goes back to the library.
    func iconId(_ id: String, fallback: String?) -> String? {
        if case .library(let i)? = icons[id] { return i }
        if let i = lastLibrary[id] { return i }
        return fallback
    }

    /// `AccountSummary.icon_file`: the uploaded image's path, None when the icon is a library glyph (delta 11, item 2b).
    func iconFile(_ id: String) -> String? {
        if case .uploaded(let p)? = icons[id] { return p }
        return nil
    }

    /// `account_icon/<id>`: the library icon id, or "uploaded".
    func iconWord(_ id: String, fallback: String?) -> String? {
        if case .uploaded? = icons[id] { return "uploaded" }
        return iconId(id, fallback: fallback)
    }

    // MARK: decisions, speech, MCP (group 3)

    var decision: DecisionModel?
    private(set) var preview: [DecisionVerdict] = []
    private(set) var previewApplied = false
    /// message id -> who placed it after an applied preview ("Laya", "Jev", "Rules")
    private(set) var placedBy: [String: String] = [:]
    private(set) var speech = SpeechEngine(kind: .local, ready: false)
    private(set) var speechState: String?
    let mcpApps = ["Claude Desktop", "Cursor", "Raycast"]
    private var mcpAllowed: [String: Int64] = [:]
    private(set) var mcpOn = true

    func chooseSpeech(_ kind: SpeechEngineKind, hasKey: (String) -> Bool) throws {
        speech = SpeechEngine(kind: kind, ready: false)
        if let reason = failures.removeValue(forKey: "speech-download") {
            speechState = "failed"
            throw CoreError.Invalid(field: "speech_engine", reasonWords: reason.isEmpty ? "The download stopped: the network dropped." : reason)
        }
        if case .cloud(let providerId) = kind, !hasKey(providerId) {
            speechState = "needs key"
            throw CoreError.Invalid(field: "speech_engine", reasonWords: "Add your \(providerId) key in Settings: AI providers first.")
        }
        speech = SpeechEngine(kind: kind, ready: true)
        speechState = "ready"
    }

    func saveDecisionModel(_ m: DecisionModel, hasKey: (String) -> Bool) throws {
        if m.provider == .jev && !hasKey("jev") { throw CoreError.Invalid(field: "provider", reasonWords: "Add your Jev key first; Jev is not used without one.") }
        var out = m
        if out.provider == .local, out.model == nil { out.model = "laya" }
        decision = out
        preview = []; previewApplied = false
    }

    /// Verdicts over `recent` (message id, its current tab name, the name of the next tab): every third one differs.
    func runPreview(recent: [(id: String, tab: String, other: String)], nowMs: Int64) {
        let n = min(WhooshConstants.DECISION_PREVIEW_N, recent.count)
        preview = recent.prefix(n).enumerated().map { i, r in
            DecisionVerdict(messageId: r.id, entry: i % 3 == 2 ? nil : UInt32(1 + i % 4), outcome: i % 3 == 0 ? r.other : r.tab, differs: i % 3 == 0)
        }
        previewApplied = false
        var m = decision ?? DecisionModel(provider: .rules, model: nil, keychain: nil, decisionsEnabled: ["which-tab", "needs-reply", "vips"], questionWords: [:], lastRunMs: nil)
        m.lastRunMs = nowMs
        decision = m
    }

    /// The rows of a `previewDecisionList` run and its sentence (FixtureDecisionPreview.swift): kept for `decisionPreview()`, applied by no one until Apply.
    private(set) var previewWords = ""
    func keepPreview(_ rows: [DecisionVerdict], words: String, nowMs: Int64) {
        preview = rows
        previewWords = words
        previewApplied = false
        if var m = decision { m.lastRunMs = nowMs; decision = m }   // the saved choice is never invented by a preview
    }

    func applyPreview() throws {
        guard !preview.isEmpty else { throw CoreError.Invalid(field: "preview", reasonWords: "Run the decision model on recent mail first.") }
        let who: String
        switch decision?.provider ?? .rules { case .local: who = "Laya"; case .jev: who = "Jev"; case .rules: who = "Rules"; case .apple: who = "Apple Intelligence" }
        for v in preview where v.differs { placedBy[v.messageId] = who }
        previewApplied = true
    }

    func setMcp(_ app: String, allowed: Bool, nowMs: Int64) throws {
        guard mcpApps.contains(app) else { throw CoreError.NotFound(kind: "app", id: app) }
        if allowed { mcpAllowed[app] = nowMs } else { mcpAllowed[app] = nil }
    }
    func setMcpServer(_ on: Bool) { mcpOn = on }
    func mcpGrants() -> [McpGrant] { mcpApps.map { McpGrant(appName: $0, allowed: mcpAllowed[$0] != nil, grantedAtMs: mcpAllowed[$0]) } }
    func mcpAllowedNow(_ app: String) -> Bool { mcpAllowed[app] != nil }

    func reset() { /* settings survive a corpus load; only the armed refusals go */ failures = [:] }
}
