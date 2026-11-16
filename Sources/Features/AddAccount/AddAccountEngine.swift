import Foundation

/// Round 5: the engine's refusals shown in the app's language. The engine sends a code beside its English words; the sheet shows the
/// catalogue sentence for the code and keeps the engine's words behind Details. The English words never show alone.
enum EngineRefusal {
    /// A97: Microsoft wants the organisation's admin to approve the app once (AADSTS65001, `consent_required`).
    static let consent = "oauth.consent_required"
    /// The codes shared with the engine (`ActionFailed.code`): `engine.*` and `oauth.*`.
    static let codes = ["engine.not_built_yet", "oauth.no_client_id", "oauth.sign_in_not_done", "oauth.refused", "oauth.no_refresh_token", "oauth.browser", "oauth.keychain", EngineRefusal.consent]

    /// A code this file owns: any `engine.` or `oauth.` code, known or not (a server's SMTP code such as "451 4.7.1" is never one).
    static func owns(_ code: String?) -> Bool {
        guard let code else { return false }
        return code.hasPrefix("engine.") || code.hasPrefix("oauth.")
    }

    /// A98: Microsoft's one-click admin-consent page for an organisation's tenant (its domain) and the sign-in's client ID.
    static func adminConsentLink(domain: String, clientId: String) -> String {
        let tenant = domain.addingPercentEncoding(withAllowedCharacters: .urlHostAllowed) ?? domain
        return "https://login.microsoftonline.com/\(tenant)/adminconsent?client_id=\(clientId)"
    }

    /// The one plain sentence for a code. A code with no sentence reads as the A87 "unknown" sentence, never as the engine's words.
    /// A98: the consent sentence names the mailbox's organisation (the address's domain) when it is known.
    static func sentence(_ code: String, domain: String = "") -> String {
        if code == EngineRefusal.consent, !domain.isEmpty { return AddAccountWords.consentRequired(domain) }
        switch code {
        case "engine.not_built_yet": return Words.text("add-account.engine.not_built_yet")
        case "oauth.no_client_id": return Words.text("add-account.engine.oauth_no_client_id")
        case "oauth.sign_in_not_done": return Words.text("add-account.engine.oauth_sign_in_not_done")
        case "oauth.refused": return Words.text("add-account.engine.oauth_refused")
        case "oauth.no_refresh_token": return Words.text("add-account.engine.oauth_no_refresh_token")
        case "oauth.browser": return Words.text("add-account.engine.oauth_browser")
        case "oauth.keychain": return Words.text("add-account.engine.oauth_keychain")
        case EngineRefusal.consent: return Words.text("add-account.engine.oauth_consent_required")
        default: return AddAccountWords.causeSentence(.unknown)
        }
    }

    /// A phase's words carry the code in front, `code: words`, when the refusal came from the engine (a state read has no code field).
    static func tag(_ code: String?, _ words: String) -> String { owns(code) ? "\(code ?? ""): \(words)" : words }

    /// Splits `code: words` (or the bare code) back into its parts; nil for words that carry no engine code.
    static func parse(_ words: String) -> (code: String, words: String)? {
        let head = words.prefix { !$0.isWhitespace && $0 != ":" }
        guard owns(String(head)) else { return nil }
        let rest = words.dropFirst(head.count).drop { $0 == ":" || $0.isWhitespace }
        return (String(head), String(rest))
    }
}

extension AddAccountModel {
    /// Shows an engine refusal in the error panel: the catalogue sentence, the engine's words behind Details (closed).
    func showEngine(code: String, words: String) {
        errorEngineCode = code; errorCause = nil
        errorText = EngineRefusal.sentence(code, domain: domain); errorDetails = words
        errorLink = ""; errorHost = ""; errorHeader = AddAccountWords.signInFailed
        detailsOpen = false; refusalTick += 1
    }
}

extension AddAccountModel {
    /// The two admin paths, one line each, under the sentence; none for any other refusal.
    var consentPaths: [String] {
        errorEngineCode == EngineRefusal.consent ? [AddAccountWords.consentPathPage, AddAccountWords.consentPathEntra] : []
    }
}

extension AddAccountModel {
    /// A98: the client ID the sign-in ran with: the one set when Sign in was tapped, else the default registration in use.
    var consentClientId: String? { signInClientId ?? DefaultSignInClients.microsoftInUse }

    /// A98: the one-click admin-consent link for the mailbox's organisation; empty for any other refusal or when the domain or ID is unknown.
    var consentLink: String {
        guard errorEngineCode == EngineRefusal.consent, !domain.isEmpty, let id = consentClientId, !id.isEmpty else { return "" }
        return EngineRefusal.adminConsentLink(domain: domain, clientId: id)
    }

    /// The link line as shown, `Approval link: <https link>`; empty when there is no link.
    var consentLinkLine: String { consentLink.isEmpty ? "" : AddAccountWords.consentLinkLine(consentLink) }
}

extension AddAccountModel {
    /// A refused sign-in state whose words start with an engine code: the error panel gets the sentence and Details too.
    func showEngineIfRefused(_ state: OAuthSignInState) {
        if case .refused(let w) = state, let p = EngineRefusal.parse(w) { showEngine(code: p.code, words: p.words) }
    }
}
