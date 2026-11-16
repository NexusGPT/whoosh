import CryptoKit
import Foundation
import Network
import Testing
@testable import WhooshUI

// The sign-in engine against STUB authorisation servers, ported from the side package of commit 6f2d77a0
// (_captures/signin/proof/Tests/SignInEngineTests) so the app's own test run executes them.
// Every server here listens on 127.0.0.1 only; every endpoint the service is given is a 127.0.0.1 address, and the service is
// built with a `reaches` rule that refuses any other host, so no test can ask a real provider. A token in these tests is a stub
// string; it is kept only in a `MemoryVault` (the real app keeps it in the Keychain through `KeychainVault`) and no line of
// output prints one. Every test carries a time limit.

/// A loopback HTTP stub on an ephemeral port. Records every request it receives, answers with `status` and `body`.
final class SignInStub: @unchecked Sendable {
    let listener: NWListener
    private let lock = NSLock()
    private var seen: [String] = []
    private(set) var port: UInt16 = 0
    let status: Int, body: String
    init(status: Int, body: String) throws {
        self.status = status; self.body = body
        let p = NWParameters.tcp
        p.requiredLocalEndpoint = NWEndpoint.hostPort(host: "127.0.0.1", port: .any)
        listener = try NWListener(using: p)
        listener.newConnectionHandler = { [self] conn in
            conn.start(queue: .global())
            self.read(conn, Data())
        }
    }
    /// Reads until the headers and the whole body (Content-Length) have arrived, then answers.
    private func read(_ conn: NWConnection, _ acc: Data) {
        conn.receive(minimumIncompleteLength: 1, maximumLength: 16384) { data, _, done, _ in
            var all = acc; all.append(data ?? Data())
            let text = String(decoding: all, as: UTF8.self)
            let parts = text.components(separatedBy: "\r\n\r\n")
            let want = text.range(of: "Content-Length: ").flatMap { Int(text[$0.upperBound...].prefix { $0.isNumber }) } ?? 0
            if parts.count < 2 || parts[1].utf8.count < want, !done { self.read(conn, all); return }
            self.lock.withLock { self.seen.append(text) }
            let head = "HTTP/1.1 \(self.status) X\r\nContent-Type: application/json\r\nContent-Length: \(self.body.utf8.count)\r\nConnection: close\r\n\r\n"
            conn.send(content: Data((head + self.body).utf8), completion: .contentProcessed { _ in conn.cancel() })
        }
    }
    func start() async throws {
        let ready = AsyncStream<UInt16> { c in
            listener.stateUpdateHandler = { st in
                if case .ready = st, let p = self.listener.port?.rawValue { c.yield(p); c.finish() }
                if case .failed = st { c.yield(0); c.finish() }
            }
        }
        listener.start(queue: .global())
        for await p in ready { port = p }
        #expect(port != 0, "the loopback stub did not start")
    }
    func stop() { listener.cancel() }
    var tokenURL: URL { URL(string: "http://127.0.0.1:\(port)/token")! }
    var requests: [String] { lock.withLock { seen } }
}

/// Answers the sign-in sheet like a provider would: a code on the redirect, or a refusal. It opens nothing.
private struct SignInFakeSheet: AuthorizationUI {
    var respond: @Sendable (URL) -> URL
    func authorize(_ url: URL, callbackScheme: String) async throws -> URL { respond(url) }
    static func grants(code: String = "stub-code") -> SignInFakeSheet {
        SignInFakeSheet { u in URL(string: "whoosh:/oauth/callback?code=\(code)&state=\(signInParam(u, "state") ?? "")")! }
    }
    static func refuses(error: String, description: String) -> SignInFakeSheet {
        SignInFakeSheet { u in
            let d = description.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? ""
            return URL(string: "whoosh:/oauth/callback?error=\(error)&error_description=\(d)&state=\(signInParam(u, "state") ?? "")")!
        }
    }
}

private final class SignInBox<T: Sendable>: @unchecked Sendable {
    private let lock = NSLock(); private var v: T?
    func set(_ x: T) { lock.withLock { v = x } }
    func get() -> T? { lock.withLock { v } }
}

private func signInParam(_ u: URL, _ n: String) -> String? {
    URLComponents(url: u, resolvingAgainstBaseURL: false)?.queryItems?.first { $0.name == n }?.value
}

private func signInFormValue(_ request: String, _ name: String) -> String? {
    let body = request.components(separatedBy: "\r\n\r\n").last ?? ""
    for pair in body.split(separator: "&") {
        let kv = pair.split(separator: "=", maxSplits: 1).map(String.init)
        if kv.count == 2, kv[0] == name { return kv[1].removingPercentEncoding }
    }
    return nil
}

private let signInOkToken = "{\"access_token\":\"AT-stub\",\"refresh_token\":\"RT-stub\",\"expires_in\":3600,\"token_type\":\"Bearer\"}"

/// The real provider preset with BOTH addresses swapped for this Mac's: the token address is the stub, the authorise address is a
/// loopback address nothing listens on (the sheet is a stand-in that never fetches it).
private func signInStubbed(_ base: OAuthEndpoints, token: URL, client: String = "stub-client") -> OAuthEndpoints {
    var e = base
    e.authorize = URL(string: "http://127.0.0.1:9/authorize")!
    e.token = token; e.clientId = client
    return e
}

private func signInOnThisMacOnly(_ u: URL) -> Bool { u.host == "127.0.0.1" }

private func signInService(_ vault: MemoryVault, _ ui: some AuthorizationUI, token: URL, client: String = "stub-client",
                           flags: SignInFlags = SignInFlags()) -> SignInService {
    SignInService(vault: vault, ui: ui, flags: flags, endpoints: { p in
        switch p {
        case .microsoft: return signInStubbed(OAuthPresets.microsoft(clientId: ""), token: token, client: client)
        case .fastmail: return signInStubbed(OAuthPresets.fastmail(clientId: ""), token: token, client: client)
        case .google: return signInStubbed(OAuthPresets.google(clientId: ""), token: token, client: client)
        case .chatgpt: return signInStubbed(OAuthPresets.chatgpt(clientId: ""), token: token, client: client)
        }
    }, reaches: signInOnThisMacOnly)
}

@Test(.timeLimit(.minutes(1))) func everyStubbedSignInRouteAsksOnlyAddressesOnThisMac() async throws {
    let stub = try SignInStub(status: 200, body: signInOkToken); try await stub.start(); defer { stub.stop() }
    let s = signInService(MemoryVault(), SignInFakeSheet.grants(), token: stub.tokenURL, flags: SignInFlags(gmailAPIVerified: true))
    for p in SignInProvider.allCases {
        let e = try #require(s.endpoints(p))
        #expect(e.authorize.host == "127.0.0.1" && e.token.host == "127.0.0.1", "\(p) has an address off this Mac")
        #expect(!s.refusesAddress(p))
    }
    // the guard itself: a real provider address is refused before any sheet or request
    let real = SignInService(vault: MemoryVault(), ui: SignInFakeSheet.grants(), endpoints: { _ in OAuthPresets.microsoft(clientId: "x") },
                             reaches: signInOnThisMacOnly)
    #expect(real.refusesAddress(.microsoft))
}

@Test(.timeLimit(.minutes(1))) func microsoftSignInSendsPKCEAndStoresTheTokenOnlyInTheVault() async throws {
    let stub = try SignInStub(status: 200, body: signInOkToken); try await stub.start(); defer { stub.stop() }
    let vault = MemoryVault()
    let seenAuthorize = SignInBox<URL>()
    let sheet = SignInFakeSheet { u in
        seenAuthorize.set(u)
        return URL(string: "whoosh:/oauth/callback?code=stub-code&state=\(signInParam(u, "state") ?? "")")!
    }
    let r = try await signInService(vault, sheet, token: stub.tokenURL).signIn(.microsoft, name: "assem@contoso-example.test")
    let a = try #require(seenAuthorize.get())
    #expect(signInParam(a, "code_challenge_method") == "S256")
    let scopes = try #require(signInParam(a, "scope"))
    #expect(scopes.contains("IMAP.AccessAsUser.All")); #expect(scopes.contains("graph.microsoft.com/Mail.ReadWrite"))
    #expect(scopes.contains("offline_access"))
    let posted = try #require(stub.requests.first)
    let verifier = try #require(signInFormValue(posted, "code_verifier"))
    #expect(PKCE.challenge(for: verifier) == signInParam(a, "code_challenge"), "the posted verifier does not hash to the sent challenge")
    #expect(signInFormValue(posted, "code") == "stub-code")
    #expect(r.reference.service.hasSuffix("account-credential")); #expect(r.reference.account == "assem@contoso-example.test")
    let stored = try JSONDecoder().decode(TokenSet.self, from: vault.read(r.reference))
    let kept = stored.accessToken == "AT-stub" && stored.refreshToken == "RT-stub"
    #expect(kept, "the vault does not hold the token set the stub issued")      // never prints the value
    #expect(!String(describing: r).contains("AT-stub"), "the result must carry the reference, never the token")
}

@Test(.timeLimit(.minutes(1))) func anAdminBlockedRedirectShowsTheLineTellingTheOwnerWhatToAsk() async throws {
    let stub = try SignInStub(status: 200, body: signInOkToken); try await stub.start(); defer { stub.stop() }
    let vault = MemoryVault()
    let sheet = SignInFakeSheet.refuses(error: "access_denied",
        description: "AADSTS90094: The grant requires admin permission. Trace ID: 0000")
    do {
        _ = try await signInService(vault, sheet, token: stub.tokenURL).signIn(.microsoft, name: "a@contoso-example.test")
        Issue.record("an admin-blocked sign-in must not succeed")
    } catch let e as SignInError {
        #expect(e == .adminConsentBlocked)
        #expect(e.ownerWords.contains("Ask your admin")); #expect(e.ownerWords == SignInWords.adminBlocked)
    }
    #expect(vault.count == 0, "nothing may be saved after a blocked sign-in")
    #expect(stub.requests.isEmpty, "no token request after a refusal")
}

@Test(.timeLimit(.minutes(1))) func anAdminBlockedTokenAnswerShowsTheSameLine() async throws {
    let stub = try SignInStub(status: 400, body: "{\"error\":\"invalid_grant\",\"error_description\":\"AADSTS65001: The user or administrator has not consented.\\r\\nTrace ID: 1\"}")
    try await stub.start(); defer { stub.stop() }
    let vault = MemoryVault()
    do {
        _ = try await signInService(vault, SignInFakeSheet.grants(), token: stub.tokenURL).signIn(.microsoft, name: "a@contoso-example.test")
        Issue.record("must fail")
    } catch let e as SignInError { #expect(e.ownerWords == SignInWords.adminBlocked) }
    #expect(vault.count == 0)
}

@Test(.timeLimit(.minutes(1))) func otherRefusalsKeepTheProvidersOwnWords() async throws {
    let stub = try SignInStub(status: 400, body: "{\"error\":\"invalid_grant\",\"error_description\":\"The code has expired.\"}")
    try await stub.start(); defer { stub.stop() }
    do {
        _ = try await signInService(MemoryVault(), SignInFakeSheet.grants(), token: stub.tokenURL).signIn(.fastmail, name: "a@fastmail-example.test")
        Issue.record("must fail")
    } catch let e as SignInError { #expect(e.ownerWords == "The code has expired."); #expect(e != .adminConsentBlocked) }
}

@Test(.timeLimit(.minutes(1))) func fastmailUsesAuthorizationCodeWithPKCEAndJMAPScopes() async throws {
    let stub = try SignInStub(status: 200, body: signInOkToken); try await stub.start(); defer { stub.stop() }
    let vault = MemoryVault()
    let seen = SignInBox<URL>()
    let sheet = SignInFakeSheet { u in seen.set(u); return URL(string: "whoosh:/oauth/callback?code=c&state=\(signInParam(u, "state") ?? "")")! }
    let r = try await signInService(vault, sheet, token: stub.tokenURL).signIn(.fastmail, name: "a@fastmail-example.test")
    let a = try #require(seen.get())
    #expect(signInParam(a, "response_type") == "code"); #expect(signInParam(a, "code_challenge_method") == "S256")
    #expect(signInParam(a, "scope")?.contains("urn:ietf:params:jmap:mail") == true)
    #expect(signInFormValue(try #require(stub.requests.first), "grant_type") == "authorization_code")
    #expect(vault.count == 1); #expect(r.provider == .fastmail)
}

@Test(.timeLimit(.minutes(1))) func theGoogleRouteIsUnreachableWhileTheVerificationFlagIsFalse() async throws {
    let stub = try SignInStub(status: 200, body: signInOkToken); try await stub.start(); defer { stub.stop() }
    let vault = MemoryVault()
    let sheetOpened = SignInBox<Bool>()
    let sheet = SignInFakeSheet { u in sheetOpened.set(true); return URL(string: "whoosh:/oauth/callback?code=c&state=\(signInParam(u, "state") ?? "")")! }
    do {
        _ = try await signInService(vault, sheet, token: stub.tokenURL, flags: SignInFlags(gmailAPIVerified: false)).signIn(.google, name: "a@gmail-example.test")
        Issue.record("the Google route must be closed")
    } catch let e as SignInError { #expect(e == .routeClosed(SignInWords.googleClosed)) }
    #expect(sheetOpened.get() == nil, "the sign-in sheet must never open while the flag is false")
    #expect(stub.requests.isEmpty); #expect(vault.count == 0)
    // positive control: the same call opens once the flag is true
    let ok = try await signInService(vault, sheet, token: stub.tokenURL, flags: SignInFlags(gmailAPIVerified: true)).signIn(.google, name: "a@gmail-example.test")
    #expect(ok.provider == .google); #expect(vault.count == 1)
}

@Test(.timeLimit(.minutes(1))) func signInWithChatGPTStoresAProviderCredentialAndTheNoteIsFixed() async throws {
    let stub = try SignInStub(status: 200, body: signInOkToken); try await stub.start(); defer { stub.stop() }
    let vault = MemoryVault()
    let r = try await signInService(vault, SignInFakeSheet.grants(), token: stub.tokenURL).signIn(.chatgpt, name: "chatgpt")
    #expect(r.reference.service.hasSuffix("provider-credential"))
    let kept = try JSONDecoder().decode(TokenSet.self, from: vault.read(r.reference)).accessToken == "AT-stub"
    #expect(kept, "the vault does not hold the token the stub issued")
    #expect(SignInWords.chatgptNote.contains("can stop working"))
}

@Test(.timeLimit(.minutes(1))) func aRedirectWithAnotherStateIsIgnored() async throws {
    let stub = try SignInStub(status: 200, body: signInOkToken); try await stub.start(); defer { stub.stop() }
    let vault = MemoryVault()
    let sheet = SignInFakeSheet { _ in URL(string: "whoosh:/oauth/callback?code=c&state=forged")! }
    await #expect(throws: SignInError.self) { try await signInService(vault, sheet, token: stub.tokenURL).signIn(.fastmail, name: "a@x.test") }
    #expect(vault.count == 0); #expect(stub.requests.isEmpty)
}

@Test(.timeLimit(.minutes(1))) func aRouteWithoutAnAppRegistrationSaysSo() async throws {
    // an empty client id on a loopback address: the words are the registration's, not the off-this-Mac refusal
    let s = signInService(MemoryVault(), SignInFakeSheet.grants(), token: URL(string: "http://127.0.0.1:9/token")!, client: "")
    do { _ = try await s.signIn(.microsoft, name: "a@x.test"); Issue.record("must fail") }
    catch let e as SignInError { #expect(e.ownerWords == SignInWords.noRegistration) }
}

// A successful ChatGPT sign-in shows as connected on the Settings row (ChatGPTState, ProvidersView.swift), a failed one does not.

@MainActor @Test(.timeLimit(.minutes(1))) func aSuccessfulChatGPTSignInShowsTheSettingsRowAsConnected() async throws {
    let stub = try SignInStub(status: 200, body: signInOkToken); try await stub.start(); defer { stub.stop() }
    let vault = MemoryVault()
    let c = SignInController()
    c.service = signInService(vault, SignInFakeSheet.grants(), token: stub.tokenURL)
    #expect(ChatGPTState.words(for: c) == AIWords.notConnected, "before any sign-in the row must say not connected")
    await c.signInWithChatGPT()
    #expect(c.phase == .done(.chatgpt))
    #expect(ChatGPTState.words(for: c) == AIWords.connected)
    #expect(c.chatgptRow()["state"] as? String == "connected")
    #expect(vault.count == 1, "the token must be in the vault")
    #expect(!String(describing: c.chatgptRow()).contains("AT-stub") && !String(describing: c.phase).contains("AT-stub"),
            "the controller must show the state, never the token")
}

@MainActor @Test(.timeLimit(.minutes(1))) func aRefusedChatGPTSignInLeavesTheSettingsRowNotConnected() async throws {
    let stub = try SignInStub(status: 400, body: "{\"error\":\"invalid_grant\",\"error_description\":\"The code has expired.\"}")
    try await stub.start(); defer { stub.stop() }
    let vault = MemoryVault()
    let c = SignInController()
    c.service = signInService(vault, SignInFakeSheet.grants(), token: stub.tokenURL)
    await c.signInWithChatGPT()
    #expect(c.phase == .failed("The code has expired."))
    #expect(ChatGPTState.words(for: c) == AIWords.notConnected)
    #expect(vault.count == 0)
}
