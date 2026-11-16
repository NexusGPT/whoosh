import Foundation

/// Contracts delta 14 under --fixture-core: the bring-your-own OAuth2 sign-in, answered by a FAKE authority inside the
/// fixture core. No browser opens, no network is dialled and no Keychain item is written (the hidden rig shows nothing):
/// StartOAuthSignIn moves the draft to Waiting with an authorize URL on the fake authority's address, and the provider's
/// answer arrives with `fixture oauth approve|refuse <address> [words]` (ControlSocket), as a person finishing or
/// cancelling the sign-in in the browser would. The client ID is checked by the contract's own rule
/// (`checkOauthClientId`), so a wrong paste is refused in the real words.
final class FixtureOAuthBook: @unchecked Sendable {
    struct Draft { var provider: OAuthProvider; var clientId: String; var state: OAuthSignInState }
    private(set) var drafts: [String: Draft] = [:]
    /// The fake authority's address; never dialled (the answer comes by the fixture verb).
    static let authority = "http://127.0.0.1/fixture-authority"

    func setClient(_ draft: String, provider: OAuthProvider, clientId: String) throws {
        let d = try oauthAccountDraftId(address: draft)
        let id = try checkOauthClientId(provider: provider, clientId: clientId)
        drafts[d] = Draft(provider: provider, clientId: id, state: .ready)
    }

    func start(_ draft: String) throws {
        let d = try oauthAccountDraftId(address: draft)
        guard var e = drafts[d] else {
            throw CoreError.Invalid(field: "client_id", reasonWords: "paste the client ID of the app you registered first")
        }
        let scope = oauthEndpoints(provider: e.provider).scopes.joined(separator: " ")
        var q = URLComponents(string: Self.authority + "/authorize")!
        q.queryItems = [URLQueryItem(name: "client_id", value: e.clientId), URLQueryItem(name: "response_type", value: "code"),
                        URLQueryItem(name: "scope", value: scope), URLQueryItem(name: "code_challenge_method", value: "S256"),
                        URLQueryItem(name: "login_hint", value: d)]
        e.state = .waiting(authorizeUrl: q.url!.absoluteString)
        drafts[d] = e
    }

    /// The fake authority's answer: approve makes the sign-in Done; refuse carries the provider's words.
    func answer(_ draft: String, approve: Bool, words: String) throws {
        let d = try oauthAccountDraftId(address: draft)
        guard var e = drafts[d], case .waiting = e.state else {
            throw CoreError.Invalid(field: "sign_in", reasonWords: "no sign-in is waiting for \(d)")
        }
        e.state = approve ? .done : .refused(words: words.isEmpty ? "the provider refused the sign-in" : "the provider refused the sign-in: \(words)")
        drafts[d] = e
    }

    func state(_ draft: String) -> OAuthSignInState {
        guard let d = try? oauthAccountDraftId(address: draft) else { return .needsClientId }
        return drafts[d]?.state ?? .needsClientId
    }
}
