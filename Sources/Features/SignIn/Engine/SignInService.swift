import Foundation

/// The sheet that asks the person to sign in and hands back the redirect. Production: ASWebAuthenticationSession.
/// Tests: a closure-backed stand-in that answers like the stubbed provider.
public protocol AuthorizationUI: Sendable {
    func authorize(_ url: URL, callbackScheme: String) async throws -> URL
}

public final class SignInService: Sendable {
    public let client: OAuthClient
    public let vault: TokenVault
    public let ui: AuthorizationUI
    public let flags: SignInFlags
    public let endpoints: @Sendable (SignInProvider) -> OAuthEndpoints?
    public let nowMs: @Sendable () -> Int64
    /// Whether an address may be asked at all. The app passes the test copy's rule (`RigNet.allows`: on a test copy only this Mac);
    /// the default allows every address. `refusal` gives the words of a refused route.
    public let reaches: @Sendable (URL) -> Bool
    public let refusal: @Sendable () -> String

    public init(vault: TokenVault, ui: AuthorizationUI, flags: SignInFlags = SignInFlags(),
                client: OAuthClient = OAuthClient(),
                endpoints: @escaping @Sendable (SignInProvider) -> OAuthEndpoints?,
                nowMs: @escaping @Sendable () -> Int64 = { Int64(Date().timeIntervalSince1970 * 1000) },
                reaches: @escaping @Sendable (URL) -> Bool = { _ in true },
                refusal: @escaping @Sendable () -> String = { SignInWords.offThisMac }) {
        self.vault = vault; self.ui = ui; self.flags = flags; self.client = client; self.endpoints = endpoints; self.nowMs = nowMs
        self.reaches = reaches; self.refusal = refusal
    }

    /// True when the route's sign-in page or token address may not be asked (`reaches`): the route ends refused before any sheet
    /// or request. False when the route has no endpoints (that refusal is `noRegistration`'s).
    public func refusesAddress(_ provider: SignInProvider) -> Bool {
        guard let e = endpoints(provider) else { return false }
        return !(reaches(e.authorize) && reaches(e.token))
    }

    /// Runs one route end to end. The token lands in the vault; the result carries only its reference.
    /// `name` is the account address for a mail route and the provider slug for ChatGPT.
    public func signIn(_ provider: SignInProvider, name: String) async throws -> SignInResult {
        if provider == .google && !flags.gmailAPIVerified { throw SignInError.routeClosed(SignInWords.googleClosed) }
        if refusesAddress(provider) { throw SignInError.words(refusal()) }   // no sheet, no request
        guard let e = endpoints(provider), !e.clientId.isEmpty else { throw SignInError.words(SignInWords.noRegistration) }
        let pkce = PKCE(), state = PKCE.randomState()
        let callback = try await ui.authorize(client.authorizeURL(e, pkce: pkce, state: state), callbackScheme: e.redirectScheme)
        let code = try client.code(from: callback, expectedState: state)
        let tokens = try await client.exchange(code: code, endpoints: e, pkce: pkce, nowMs: nowMs())
        let kind: VaultKind = provider == .chatgpt ? .providerCredential : .accountCredential
        let ref = try vault.store(kind: kind, name: name, secret: try JSONEncoder().encode(tokens))
        return SignInResult(provider: provider, reference: ref, scopes: e.scopes, expiresAtMs: tokens.expiresAtMs)
    }

    /// Sign in again on a broken account: same route, same name, the vault item is replaced.
    public func signInAgain(_ provider: SignInProvider, name: String) async throws -> SignInResult {
        try await signIn(provider, name: name)
    }
}
