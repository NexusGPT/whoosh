import Foundation

/// The one way the Welcome rows and the sign-in route add an account on the REAL engine. The real engine refuses the address-only
/// `AddAccount` by design (its refusal names `AddAccountConfigured`), so both paths do what the Add account sheet does:
/// detect the servers (`detectProvider`), then send `AddAccountConfigured` with the address, the detected servers and the password.
/// The sheet's own functions do the work (`AddAccountDraft`, `AddAccountModel.validateThenAdd`) and its own sentences speak
/// (`AddAccountWords`, `EngineRefusal`, `RefusalCause`), so a refusal reads the same here as there. Nothing here keeps a password.
/// Under the fixture core the older `AddAccount` is still sent (SEAM(fixture): its AddAccountConfigured checks a password list).
enum RealAddAccount {
    /// How an add ended: the account's id, or the sentence to show in place and the first link in the server's words.
    struct Result: Sendable, Equatable {
        var id: String?
        var words = ""
        var link = ""
        static func refused(_ words: String, link: String = "") -> Result { Result(id: nil, words: words, link: link) }
    }

    /// What the off-main part ends in: a finished answer, or a failed check whose sentence is chosen on the main actor (the sheet's helpers live there).
    enum Attempt: Sendable { case done(Result), failed(AddOutcome, address: String) }

    /// `secret` nil with `oauth` false means no password was given: refused in the sheet's sentence. `oauth` true is the sign-in route: the
    /// provider's sign-in is done and the account is saved with auth method oauth2 and an empty password, as the sheet does after its sign-in.
    /// Detection and the check are network reads, so a caller runs this off the main thread (`add` does).
    nonisolated static func attempt(core: WhooshCore, address raw: String, provider: MailProvider, secret: String?, oauth: Bool, fixture: Bool) -> Attempt {
        let address = raw.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if fixture { return .done(run(core, .addAccount(address: address, provider: provider))) }
        guard AddAccountModel.looksLikeAddress(address) else { return .done(.refused(AddAccountWords.badAddress)) }
        if !oauth, (secret ?? "").isEmpty { return .done(.refused(AddAccountWords.needPassword)) }
        var draft: AddAccountDraft
        switch core.detectProvider(email: address).outcome {
        case .found(let config): draft = AddAccountDraft(config)
        case .imapOnly(let host, let port, let security, let user, let source):
            draft = AddAccountDraft(imapOnly: host, port: port, security: security, username: user, source: source); draft.fillUsual()
        case .refused(let words): return .done(.refused(words))
        }
        var config: AccountConfig
        switch draft.config(address: address) {
        case .success(let c): config = c
        case .failure(let p): return .done(.refused(p == .incoming ? AddAccountWords.incomingMissing : AddAccountWords.outgoingMissing))
        }
        if oauth {
            config.authMethod = .oauth2
            return .done(run(core, .addAccountConfigured(address: address, displayName: nil, config: config, password: SecretText(value: ""))))
        }
        let out = AddAccountModel.validateThenAdd(core, config, address, SecretText(value: secret ?? ""), StopFlag())
        if let id = out.createdId { return .done(Result(id: id)) }
        return .failed(out, address: address)
    }

    /// The sentence for a failed check: the same choice as the sheet's `finished` (the engine's code, else the known cause, else the server's words).
    @MainActor static func settle(_ a: Attempt) -> Result {
        switch a {
        case .done(let r): return r
        case .failed(let out, let address):
            let words = out.failure ?? out.report.summaryWords
            if let code = out.failureCode { return .refused(EngineRefusal.sentence(code, domain: domainOf(address))) }
            let sentence = out.failure == nil ? out.report.cause.map(AddAccountWords.causeSentence) : nil
            return .refused(sentence ?? AddAccountModel.withSignInHint(words, oauthKind(address)), link: AddAccountModel.firstLink(in: words))
        }
    }

    /// Off the main thread, then the sentence on it.
    @MainActor static func add(core: WhooshCore, address: String, provider: MailProvider, secret: String?, oauth: Bool, fixture: Bool) async -> Result {
        let a = await Task.detached { attempt(core: core, address: address, provider: provider, secret: secret, oauth: oauth, fixture: fixture) }.value
        return settle(a)
    }

    nonisolated static func run(_ core: WhooshCore, _ command: Command) -> Result {
        do {
            let out = try core.run(command: command)
            return Result(id: out.createdId ?? "", words: "", link: "")
        } catch CoreError.Action(let f) {
            return .refused(EngineRefusal.owns(f.code) ? EngineRefusal.sentence(f.code ?? "") : f.reasonWords)
        } catch CoreError.Invalid(_, let words) { return .refused(words) }
        catch { return .refused("\(error)") }
    }

    nonisolated static func domainOf(_ address: String) -> String { address.split(separator: "@").last.map { String($0).lowercased() } ?? "" }

    /// Microsoft 365 or Google Workspace addresses take the "sign in with your own app ID" hint after a refused password (not a personal Gmail).
    @MainActor static func oauthKind(_ address: String) -> OAuthKind? {
        if AddAccountModel.isPersonalGoogle(domainOf(address)) { return nil }
        switch ProviderKind.of(address: address) { case .microsoft?: return .microsoft; case .google?: return .google; default: return nil }
    }
}
