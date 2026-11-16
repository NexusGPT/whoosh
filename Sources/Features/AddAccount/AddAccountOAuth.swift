import Foundation
#if os(macOS)
import AppKit
#endif

/// Bring your own sign-in (PRD 2026-11-09 "Adding an account must be as easy as Thunderbird"): Whoosh ships no Microsoft or Google client
/// ID and submits nothing to either, ever. For a Microsoft 365 or Google Workspace address the person pastes THEIR organisation's app ID
/// once; the core keeps it in the Keychain and Sign in runs its PKCE loopback flow. The doors are contract delta 14's:
///   · command `SetOAuthClient { account_draft_id, provider, client_id }`: the ID checked by the provider's shape, a wrong paste refused in words
///   · command `StartOAuthSignIn { account_draft_id }`
///   · read `oauth_sign_in_state(account_draft_id)`: NeedsClientId · Ready · Waiting{authorize_url} · Done · Refused{words}, POLLED here
///   · `AddAccountConfigured` (auth_method oauth2, empty password) saves the account once the state is Done.
/// The draft id is the address, trimmed and lower-cased (`oauthAccountDraftId`). A personal @gmail.com or @googlemail.com address takes an
/// app password only, so it never shows this field.
enum OAuthKind: String, CaseIterable, Sendable {
    case microsoft, google
    var provider: OAuthProvider { self == .microsoft ? .microsoft : .google }
}

enum OAuthPhase: Equatable {
    case idle, waiting, done, refused(String)
    var name: String {
        switch self { case .idle: return "idle"; case .waiting: return "waiting"; case .done: return "done"; case .refused: return "refused" }
    }
    var words: String {
        switch self {
        case .idle: return ""
        case .waiting: return AddAccountWords.waitingBrowser
        case .done: return AddAccountWords.signedIn
        case .refused(let w): return EngineRefusal.parse(w).map { EngineRefusal.sentence($0.code) } ?? w
        }
    }
}

struct OAuthCall: Sendable { let error: String?; let state: OAuthSignInState; var code: String? = nil }

extension AddAccountModel {
    /// The provider whose own sign-in the sheet offers: Microsoft 365, or Google Workspace (not a personal Gmail address).
    var oauthKind: OAuthKind? {
        if AddAccountModel.isPersonalGoogle(domain) { return nil }
        switch headKind { case .microsoft?: return .microsoft; case .google?: return .google; default: return nil }
    }
    nonisolated static func isPersonalGoogle(_ d: String) -> Bool { d == "gmail.com" || d == "googlemail.com" }

    var oauthDraftId: String? { try? oauthAccountDraftId(address: address) }
    var oauthPhase: OAuthPhase {
        switch oauthState {
        case .waiting: return .waiting
        case .done: return .done
        case .refused(let w): return .refused(w)
        default: return .idle
        }
    }
    /// The app ID is set for this address (the core holds it; it is never read back).
    func oauthSaved(_ k: OAuthKind) -> Bool { oauthState != .needsClientId && !changingAppId }
    var canSignIn: Bool { stage == .summary && oauthKind != nil && oauthPhase != .waiting && oauthPhase != .done }

    /// SPEC A88: a Microsoft 365 address with no own ID saved signs in with Whoosh's default registration (when it ships
    /// one) unless the person opened "Use your organisation’s own ID". nil default = the field shows, as before A88.
    var defaultClientId: String? {
        guard oauthKind == .microsoft, !oauthSaved(.microsoft), !ownIdOpen else { return nil }
        return DefaultSignInClients.microsoftInUse
    }
    var usesDefaultClient: Bool { defaultClientId != nil }

    /// "Use your organisation’s own ID": the app-ID field comes back over the default.
    @discardableResult
    func openOwnId() -> Bool {
        guard usesDefaultClient else { return false }
        ownIdOpen = true; return true
    }

    /// Reads the sign-in's state for this address from the core.
    func refreshOAuth() {
        guard let id = oauthDraftId else { oauthState = .needsClientId; return }
        oauthState = AppModel.shared.core.oauthSignInState(accountDraftId: id)
    }

    /// Change: the field comes back so a new ID can be pasted (SetOAuthClient replaces the stored one).
    @discardableResult
    func changeAppId() -> Bool {
        guard let k = oauthKind, oauthSaved(k) else { return false }
        changingAppId = true; oauthIdStore = ""; return true
    }

    /// Sign in: set the pasted ID when there is one to set, start the flow, then watch its state.
    @discardableResult
    func signInTapped() -> Bool {
        guard canSignIn, let kind = oauthKind, let draftId = oauthDraftId else { return false }
        commitFocused()
        let id = defaultClientId ?? oauthIdStore.trimmingCharacters(in: .whitespaces)
        let setting = !oauthSaved(kind) || !id.isEmpty
        if !id.isEmpty { signInClientId = id }
        if setting && id.isEmpty { errorText = AddAccountWords.needAppId; errorLink = ""; errorHost = ""; errorHeader = ""; return false }
        errorText = ""; errorLink = ""; errorHost = ""; errorHeader = ""; errorCause = nil; errorEngineCode = nil; errorDetails = ""; detailsOpen = false
        generation += 1; let mine = generation
        let core = AppModel.shared.core, provider = kind.provider
        run(hold: false, { Self.startOAuth(core, draftId, provider, setting ? id : nil) }) { [weak self] call in self?.oauthStarted(call, generation: mine) }
        return true
    }

    nonisolated static func startOAuth(_ core: WhooshCore, _ draftId: String, _ provider: OAuthProvider, _ clientId: String?) -> OAuthCall {
        do {
            if let clientId { _ = try core.run(command: .setOAuthClient(accountDraftId: draftId, provider: provider, clientId: clientId)) }
            _ = try core.run(command: .startOAuthSignIn(accountDraftId: draftId))
        } catch CoreError.Action(let f) { return OAuthCall(error: f.reasonWords, state: core.oauthSignInState(accountDraftId: draftId), code: EngineRefusal.owns(f.code) ? f.code : nil) }
        catch CoreError.Invalid(_, let w) { return OAuthCall(error: w, state: core.oauthSignInState(accountDraftId: draftId)) }
        catch { return OAuthCall(error: "\(error)", state: core.oauthSignInState(accountDraftId: draftId)) }
        return OAuthCall(error: nil, state: core.oauthSignInState(accountDraftId: draftId))
    }

    func oauthStarted(_ call: OAuthCall, generation g: Int) {
        guard g == generation else { return }
        oauthState = call.state
        if let e = call.error {
            if let code = call.code { showEngine(code: code, words: e); return }
            errorText = e; errorLink = ""; errorHost = ""; errorHeader = ""; return
        }
        showEngineIfRefused(call.state)
        changingAppId = false; oauthIdStore = ""
        if case .waiting(let url) = call.state { openAuthorize(url); watchOAuth(generation: g) }
    }

    /// The browser page of the provider's own sign-in (never a web view). Under the fixture core it is recorded, not opened.
    func openAuthorize(_ url: String) {
        openedLinks.append(url)
        #if os(macOS)
        // the Rust core opens the browser itself (StartOAuthSignIn: "opens the provider's sign-in in the browser"); opening here too showed the page twice
        if AppModel.shared.host.fixture == nil, AppModel.shared.host.kind != .rust, let u = URL(string: url), u.scheme == "https" { NSWorkspace.shared.open(u) }
        #endif
    }

    /// Polls the state while the browser holds the person: there is no event, the read is the contract.
    func watchOAuth(generation g: Int) {
        pollTask?.cancel()
        guard let id = oauthDraftId else { return }
        let core = AppModel.shared.core
        pollTask = Task { @MainActor [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 500_000_000)
                let s = await Task.detached { core.oauthSignInState(accountDraftId: id) }.value
                guard let self, g == self.generation else { return }
                self.oauthState = s
                switch s {
                case .done: self.oauthDone(generation: g); return
                case .waiting: continue
                case .refused: self.showEngineIfRefused(s); return
                default: return
                }
            }
        }
    }

    /// Done: the account is saved by AddAccountConfigured on that sign-in (oauth2, no password).
    func oauthDone(generation g: Int) {
        guard g == generation else { return }
        let addr = address.trimmingCharacters(in: .whitespaces).lowercased()
        guard case .success(var config) = draft.config(address: addr) else { oauthState = .refused(words: AddAccountWords.incomingMissing); return }
        config.authMethod = .oauth2
        let saved = config   // a constant copy for the background closure (Swift 6: a captured var is refused there)
        let core = AppModel.shared.core, fixture = AppModel.shared.host.fixture != nil
        let kind = oauthKind ?? .microsoft
        run(hold: false, { Self.saveOAuthAccount(core, addr, saved, kind, fixture) }) { [weak self] r in
            guard let self, g == self.generation else { return }
            if let id = r.id { self.recordAdded(id: id, address: addr, imapHost: saved.imapHost) }
            else { self.oauthState = .refused(words: EngineRefusal.tag(r.code, r.words)); self.showEngineIfRefused(self.oauthState) }
        }
    }

    /// SEAM(fixture): the fixture core's AddAccountConfigured still checks a password (and answers a tenant by IMAP host alone), so under it the
    /// account is added with the older `addAccount`; the real core takes AddAccountConfigured with an empty password.
    nonisolated static func saveOAuthAccount(_ core: WhooshCore, _ address: String, _ config: AccountConfig, _ kind: OAuthKind, _ fixture: Bool) -> (id: String?, words: String, code: String?) {
        do {
            let command: Command = fixture ? .addAccount(address: address, provider: kind == .microsoft ? .microsoft : .google)
                : .addAccountConfigured(address: address, displayName: nil, config: config, password: SecretText(value: ""))
            let out = try core.run(command: command)
            return (out.createdId, out.createdId == nil ? "The core did not say which account it made." : "", nil)
        } catch CoreError.Action(let f) { return (nil, f.reasonWords, EngineRefusal.owns(f.code) ? f.code : nil) }
        catch CoreError.Invalid(_, let w) { return (nil, w, nil) }
        catch { return (nil, "\(error)", nil) }
    }

    /// The guide: the bundled copy opens in the person's own viewer; under the fixture core the open is recorded, not performed.
    @discardableResult
    func openGuide() -> Bool {
        let name = AddAccountWords.guideFile
        openedLinks.append("guide:" + name)   // par-addaccount reads the link; the in-app guide (A90) replaces the markdown file
        EntraGuideModel.shared.open { [weak self] id in self?.guideSaved(id) }
        return true
    }

    /// The guide's "Save and sign in": the pasted ID goes in the app-ID field, and Sign in runs.
    func guideSaved(_ id: String) {
        setFocus(nil); set(.appId, id)
        _ = signInTapped()
    }
}
