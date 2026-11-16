import Foundation
import Observation

/// The Add account flow's state: the two fields, what detection found, the eight settings, the validation lines and the
/// accounts added so far. One plain method per action, so the view, the socket's click and the tests walk one path.
/// The password is held here only while the screen is open and is handed to the core's addAccount command; it is in no
/// row, no log and no file.
@MainActor @Observable
public final class AddAccountModel {
    public static let shared = AddAccountModel()

    public enum Stage: String { case entry, detecting, summary, validating, added }
    public enum Mode: String { case detected, manual }
    /// Where the card is drawn: the sheet over a window, or inline in Welcome's quick start.
    public enum Style: String { case sheet, inline }
    /// A text the person can type into: the address, the password, or one of the eight settings (by field id).
    public struct Field: Hashable, Sendable {
        public let id: String
        public init(_ id: String) { self.id = id }
        public static let address = Field("address")
        public static let password = Field("password")
        public static let appId = Field("appId")
    }

    struct Step: Equatable { let id: String; let found: Bool; let text: String; let number: UInt32 }
    struct Line: Equatable { enum State: String { case pending, running, ok, failed }; let text: String; var state: State; var details: [String] }
    struct Added: Equatable, Identifiable { let id: String; let address: String; let name: String; let colour: String; let atMs: Int64; let kind: ProviderKind }

    public internal(set) var stage: Stage = .entry
    public internal(set) var mode: Mode = .detected
    var style: Style = .sheet
    var steps: [Step] = []
    var result: DetectionResult?
    var draft = AddAccountDraft()
    var notice = ""
    var errorText = ""
    var errorLink = ""
    /// the server that refused (empty for a problem found here, before anything was sent)
    var errorHost = ""
    /// the line above the server's words: who refused, in words ("imap.mail.me.com refused the password")
    var errorHeader = ""
    /// SPEC A87: the refusal's known cause (its plain sentence is `errorText`) and the server's raw text, shown only under Details
    var errorCause: RefusalCause?
    /// Round 5: the engine's refusal code when the sentence is the engine's (its words wait behind Details)
    var errorEngineCode: String?
    var errorDetails = ""
    var detailsOpen = false
    var stepDetailsOpen = false   // Details under a failed added-account step: the engine's own words, closed until asked for
    var lines: [Line] = []
    var added: [Added] = []
    var focus: Field?
    /// Manual setup is unfolded below the fields; a refusal leaves it folded until the person asks for the server settings.
    var manualOpen = false
    /// Esc during the check asked "Stop checking?" and waits for Keep checking or Stop.
    var confirmingStop = false
    var revealed = false
    var edited = false
    var openedLinks: [String] = []
    /// A98: the links the Copy button put on the pasteboard (under the fixture core the pasteboard is left alone and the link is recorded)
    var copiedLinks: [String] = []
    /// A98: the client ID the last Sign in ran with (set or pasted); the core never gives it back
    var signInClientId: String?
    /// counts refusals: the Password field takes the focus and selects its text, so the next typing replaces it
    var refusalTick = 0
    /// the rig can hold a finished detection or validation back, so a shot shows the running state (fixture core only)
    var holdDetection = false
    var holdValidation = false
    /// bring-your-own sign-in: the core's state for this address (read, never stored here), and Change having brought the field back
    var oauthState: OAuthSignInState = .needsClientId
    var changingAppId = false
    /// SPEC A88: the person chose "Use your organisation’s own ID" over Whoosh's default Microsoft registration
    var ownIdOpen = false
    @ObservationIgnored var held: (() -> Void)?
    @ObservationIgnored var generation = 0
    @ObservationIgnored var addressStore = ""
    @ObservationIgnored var passwordStore = ""
    @ObservationIgnored var oauthIdStore = ""
    @ObservationIgnored var pollTask: Task<Void, Never>?
    @ObservationIgnored var stopFlag = StopFlag()
    /// the host's: ends the flow (Settings closes its sheet); nil on the main window, where the surface returns to mac-main
    @ObservationIgnored var onClose: (() -> Void)?
    @ObservationIgnored var openURL: ((URL) -> Void)?

    private init() {}

    // MARK: the texts. The socket's `type` appends to AppModel.typed, the app's one focused input.
    public func text(_ f: Field) -> String { focus == f ? AppModel.shared.typed : stored(f) }
    func stored(_ f: Field) -> String {
        switch f { case .address: return addressStore; case .password: return passwordStore; case .appId: return oauthIdStore; default: return draft[f.id] }
    }
    public func set(_ f: Field, _ v: String) {
        switch f { case .address: addressStore = v; case .password: passwordStore = v; case .appId: oauthIdStore = v; default: edit(f.id, v) }
        if focus == f { AppModel.shared.typed = v }
    }
    public var address: String {
        get { text(.address) }
        set { set(.address, newValue) }
    }
    var password: String {
        get { text(.password) }
        set { set(.password, newValue) }
    }

    /// Moves the typing target: what was typed stays with its field, and the new field's text becomes the typed text.
    public func setFocus(_ f: Field?) {
        guard f != focus else { return }
        commitFocused()
        focus = f
        if let f { AppModel.shared.typed = stored(f) }
    }

    /// Writes the focused field's typed text to its own store (before Add reads the settings).
    func commitFocused() {
        guard let f = focus else { return }
        let t = AppModel.shared.typed
        switch f { case .address: addressStore = t; case .password: passwordStore = t; case .appId: oauthIdStore = t; default: if mode == .manual, draft[f.id] != t { edit(f.id, t) } }
    }

    var maskedPassword: String { String(repeating: "•", count: password.count) }
    var addressLooksValid: Bool { AddAccountModel.looksLikeAddress(address) }
    var hasProblem: Bool { !errorText.isEmpty }
    var domain: String { address.split(separator: "@").last.map { String($0).lowercased() } ?? "" }

    nonisolated static func looksLikeAddress(_ s: String) -> Bool {
        let p = s.trimmingCharacters(in: .whitespaces).split(separator: "@", omittingEmptySubsequences: false)
        return p.count == 2 && !p[0].isEmpty && p[1].contains(".") && !p[1].hasPrefix(".") && !p[1].hasSuffix(".") && !p[1].contains(" ")
    }

    // MARK: what detection and the settings tell the screen
    var provider: AddAccountProvider? { AddAccountProvider.of(host: draft.imapHost, address: address) }
    var isTenant: Bool { result?.microsoft365Tenant ?? false }
    var showsSummary: Bool { stage == .summary || stage == .validating || stage == .added }
    var canAdd: Bool { stage == .summary }
    var canAddAnother: Bool { !added.isEmpty }
    /// The settings as they read now: the focused settings field's typed text counts before the field is left.
    var effectiveDraft: AddAccountDraft {
        var d = draft
        if let f = focus, f != .address, f != .password, mode == .manual { d[f.id] = AppModel.shared.typed }
        return d
    }
    var draftComplete: Bool { if case .success = effectiveDraft.config(address: address) { return true } else { return false } }

    /// The mark in the Email field: known once the address names a provider; a globe for any other domain; nothing before an address.
    var fieldKind: ProviderKind? { ProviderKind.of(address: address) }
    /// The mark in the head and the found line: the domain's provider, else the one the detected server names.
    var headKind: ProviderKind? {
        guard stage == .summary || stage == .validating, let k = fieldKind else { return nil }
        if k != .unknown { return k }
        let byHost = ProviderKind.of(host: draft.imapHost)
        // the detection names the provider too (delta 14): a Microsoft 365 tenant found by autodiscover on a domain and a
        // server the tables do not know (the hidden check's loopback server among them) still gets its sign-in
        if byHost == .unknown, let p = result?.provider { return ProviderKind.of(detected: p) }
        return byHost
    }

    // MARK: moving between states
    /// Back to the empty first screen; the accounts already added stay listed.
    public func startOver() {
        generation += 1; held = nil; stopFlag.cancel(); stopFlag = StopFlag()
        addressStore = ""; passwordStore = ""
        if let f = focus, f == .address || f == .password || f == .appId { AppModel.shared.typed = "" }
        focus = nil
        stage = .entry; mode = .detected; steps = []; result = nil; draft = AddAccountDraft()
        notice = ""; errorText = ""; errorLink = ""; errorHost = ""; errorHeader = ""; errorCause = nil; errorEngineCode = nil; errorDetails = ""; detailsOpen = false; stepDetailsOpen = false; lines = []
        manualOpen = false; confirmingStop = false; revealed = false; edited = false; openedLinks = []; copiedLinks = []; signInClientId = nil
        pollTask?.cancel(); oauthState = .needsClientId; changingAppId = false; ownIdOpen = false; oauthIdStore = ""
    }

    public func addAnother() -> Bool {
        guard canAddAnother else { return false }
        startOver(); return true
    }

    /// The edits of a settings field (Manual setup only); a typed field clears the refusal that stood on the old value.
    public func edit(_ field: String, _ value: String) {
        guard mode == .manual else { return }
        draft[field] = value; edited = true; errorText = ""; errorLink = ""; errorHost = ""; errorHeader = ""; errorCause = nil; errorEngineCode = nil; errorDetails = ""; detailsOpen = false
    }

    /// Editing the address after a detection throws the detection away: it was for another domain.
    public func addressEdited() {
        guard stage == .summary, result != nil else { return }
        stage = .entry; mode = .detected; steps = []; result = nil; draft = AddAccountDraft(); lines = []
        errorText = ""; errorLink = ""; errorHost = ""; errorHeader = ""; errorCause = nil; errorEngineCode = nil; errorDetails = ""; detailsOpen = false; manualOpen = false; edited = false
    }

    /// Lets a held detection or validation finish (the rig's `release` door).
    public func release() -> Bool {
        guard let h = held else { return false }
        held = nil; h(); return true
    }
}

/// Set when the person stops a check; the background work reads it before it saves anything.
final class StopFlag: @unchecked Sendable {
    private let lock = NSLock()
    private var stopped = false
    var isStopped: Bool { lock.lock(); defer { lock.unlock() }; return stopped }
    func cancel() { lock.lock(); stopped = true; lock.unlock() }
}
