import Foundation
import Observation

/// What the sign-in routes show and do for the app. Owns no layout: the welcome and settings lanes draw `steps`,
/// `providerRows` and `chatgptRow` and call the actions.
@MainActor @Observable
public final class SignInController {
    public static let shared = SignInController()

    public enum Phase: Equatable { case idle, connecting(SignInProvider), failed(String), done(SignInProvider) }
    public private(set) var phase: Phase = .idle
    public private(set) var address = ""
    public private(set) var chatgptConnected = false
    /// The live service: Keychain vault, the system sign-in sheet, client ids from Info.plist (empty until the app
    /// registrations exist, which makes each route say so in words). Tests and the proof replace it.
    @ObservationIgnored public lazy var service: SignInService? = SignInController.liveService()
    public var flags = SignInFlags()

    public static func provider(forAddress a: String) -> MailProviderGuess {
        let d = a.split(separator: "@").last.map { String($0).lowercased() } ?? ""
        if ["gmail.com", "googlemail.com"].contains(d) { return .google }
        if ["outlook.com", "hotmail.com", "live.com", "office365.com"].contains(d) || d.hasSuffix(".onmicrosoft.com") { return .microsoft }
        if ["fastmail.com", "fastmail.fm", "messagingengine.com"].contains(d) { return .fastmail }
        if ["icloud.com", "me.com", "mac.com"].contains(d) { return .icloud }
        return .imap
    }
    public enum MailProviderGuess: String, Sendable { case google, microsoft, fastmail, icloud, imap }

    /// A test copy (`RigNet`, `env` nil: this process) gets no sign-in sheet at all (the sheet is a window and opens a web page) and the
    /// rule that refuses every address off this Mac, so a sign-in there ends at once in `.failed` with the catalogue's words.
    nonisolated static func liveService(env: [String: String]? = nil) -> SignInService {
        let rig = usesRigSheet(env: env)
        #if canImport(AuthenticationServices)
        let ui: AuthorizationUI = rig ? RigNoSheet() : WebAuthSheet()
        #else
        let ui: AuthorizationUI = rig ? RigNoSheet() : NoSheet()
        #endif
        return SignInService(vault: KeychainVault(), ui: ui, endpoints: { p in
            switch p {
            case .microsoft: return OAuthPresets.microsoft(clientId: plistId("WhooshMicrosoftClientId"))
            case .fastmail: return OAuthPresets.fastmail(clientId: plistId("WhooshFastmailClientId"))
            case .google: return OAuthPresets.google(clientId: plistId("WhooshGoogleClientId"))
            case .chatgpt: return OAuthPresets.chatgpt(clientId: plistId("WhooshChatGPTClientId"))
            }
        }, reaches: { RigNet.allows($0, env: env) }, refusal: { RigNet.refusedWords })
    }

    /// The one choice of the sheet `liveService` makes: true on a test copy (`RigNet`, `env` nil: this process), which gets `RigNoSheet`
    /// and never `WebAuthSheet`. A pure function, so a unit test asks it without building the live service (the app alone builds that).
    nonisolated static func usesRigSheet(env: [String: String]?) -> Bool { RigNet.rig(env: env) }

    public func setAddress(_ a: String) { address = a }
    public func reset() { phase = .idle; address = "" }

    /// The routes the welcome screen offers for the typed address.
    public func providerRows() -> [ControlRow] {
        let guess = Self.provider(forAddress: address)
        func row(_ id: String, _ title: String, _ route: String) -> ControlRow {
            ["ax": "mac-welcome-accounts.provider.\(id)", "id": id, "title": title, "route": route, "suggested": guess.rawValue == id]
        }
        var rows = [row("microsoft", "Sign in with Microsoft", "oauth"), row("fastmail", "Sign in with Fastmail", "oauth"),
                    row("google", "Google: app password", flags.gmailAPIVerified ? "oauth" : "app-password")]
        rows.append(row("imap", "Other mail host", "password"))
        return rows
    }

    /// The step lines under the provider choice: what happens next, in words, including a refusal.
    public func stepRows() -> [ControlRow] {
        func step(_ n: Int, _ s: String, _ state: String) -> ControlRow { ["ax": "mac-welcome-accounts.steps.\(n)", "step": n, "text": s, "state": state] }
        switch phase {
        case .idle: return [step(1, "Type your address", address.isEmpty ? "current" : "done"), step(2, "Pick how to sign in", address.isEmpty ? "next" : "current")]
        case .connecting(let p): return [step(1, "Sign in with \(p.rawValue.capitalized) in the sheet", "current"), step(2, "Whoosh saves the sign-in in your Keychain", "next")]
        case .failed(let words): return [step(1, words, "failed")]
        case .done(let p): return [step(1, "Signed in with \(p.rawValue.capitalized)", "done")]
        }
    }

    public func chatgptRow() -> ControlRow {
        ["id": "chatgpt", "title": "Sign in with ChatGPT", "state": chatgptConnected ? "connected" : "not connected",
         "note": SignInWords.chatgptNote]
    }

    /// Starts a mail route. Returns after the sheet is up; `phase` tells how it ended.
    public func beginMailSignIn(_ provider: SignInProvider, model: AppModel) -> Bool {
        guard let service, !address.isEmpty else { phase = .failed("Type your address first."); return false }
        if service.refusesAddress(provider) { phase = .failed(service.refusal()); return true }   // a test copy: at once, no sheet, no request
        phase = .connecting(provider)
        let name = address
        Task { @MainActor in
            do {
                _ = try await service.signIn(provider, name: name)
                phase = .done(provider)
                await saveAccount(provider, address: name, core: model.core, fixture: model.host.fixture != nil, refresh: { model.refresh() })
            } catch let e as SignInError { phase = .failed(e.ownerWords) }
            catch { phase = .failed(error.localizedDescription) }
        }
        return true
    }

    /// After the provider's sign-in: the account goes to the core the way the Add account sheet saves a signed-in account (detected servers,
    /// auth method oauth2, no password: `AddAccountConfigured`, which the real engine takes; it refuses the address-only `AddAccount`).
    /// A refusal ends in `.failed` with the sheet's sentence. Under the fixture core the older `AddAccount` is sent.
    func saveAccount(_ provider: SignInProvider, address: String, core: WhooshCore, fixture: Bool, refresh: @MainActor () -> Void) async {
        let mp: MailProvider = provider == .microsoft ? .microsoft : provider == .fastmail ? .fastmail : .google
        let r = await RealAddAccount.add(core: core, address: address, provider: mp, secret: nil, oauth: true, fixture: fixture)
        if r.id != nil { refresh() } else { phase = .failed(r.words) }
    }

    public func beginChatGPTSignIn() -> Bool {
        guard let service else { return false }
        if service.refusesAddress(.chatgpt) { phase = .failed(service.refusal()); return true }   // a test copy: at once, no sheet, no request
        phase = .connecting(.chatgpt)
        Task { @MainActor in await signInWithChatGPT() }
        return true
    }

    /// `fixture chatgpt approve|refuse <loopback base>` (hid-ai round 9, item 348b2ea0): a TEST COPY signs in with ChatGPT through the same route,
    /// against a stand-in on this Mac: the sign-in page and the token address are `<base>/oauth/authorize` and `<base>/oauth/token`, the sheet is
    /// `RigChatGPTSheet` (it answers the redirect at once: a code, or the owner's refusal), and the token goes to process memory (`MemoryVault`),
    /// never the Keychain. The owner's copy refuses (nil = started; else the words of the refusal). A base off this Mac is refused (`RigNet`).
    public func rigChatGPT(approve: Bool, base: String) -> String? {
        guard RigNet.rig(env: nil) else { return RigNet.refusedWords }   // the owner's copy: the verb asks nothing
        guard let b = URL(string: base), RigNet.allows(b, env: nil) else { return RigNet.refusedWords }
        let ends = OAuthEndpoints(authorize: b.appendingPathComponent("oauth/authorize"), token: b.appendingPathComponent("oauth/token"),
                                  clientId: "whoosh-rig", scopes: ["openid", "offline_access"])
        service = SignInService(vault: MemoryVault(), ui: RigChatGPTSheet(approve: approve), endpoints: { $0 == .chatgpt ? ends : nil },
                                reaches: { RigNet.allows($0, env: nil) }, refusal: { RigNet.refusedWords })
        Task { @MainActor in await signInWithChatGPT() }
        return nil
    }

    /// `fixture reset`: ChatGPT is not connected and the live service is back (a test copy's own rule refuses every address off this Mac).
    public func resetChatGPTForFixture() {
        chatgptConnected = false; phase = .idle
        service = Self.liveService()
    }

    /// The ChatGPT route end to end. A success sets `chatgptConnected` (the Settings row reads it, `ChatGPTState`); a refusal or a
    /// failure leaves it as it was and puts the words in `phase`. The token goes to the vault only; nothing here keeps or shows it.
    public func signInWithChatGPT() async {
        guard let service else { return }
        phase = .connecting(.chatgpt)
        do { _ = try await service.signIn(.chatgpt, name: "chatgpt"); chatgptConnected = true; phase = .done(.chatgpt) }
        catch let e as SignInError { phase = .failed(e.ownerWords) }
        catch { phase = .failed(error.localizedDescription) }
    }
}

/// `fixture chatgpt`'s sheet on a test copy: no window, no web page; the redirect the provider would send, at once (hid-ai round 9).
struct RigChatGPTSheet: AuthorizationUI {
    let approve: Bool
    func authorize(_ url: URL, callbackScheme: String) async throws -> URL {
        let state = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems?.first { $0.name == "state" }?.value ?? ""
        let query = approve ? "code=rig-code&state=\(state)" : "error=access_denied&error_description=Refused%20in%20the%20sign-in%20page.&state=\(state)"
        guard let u = URL(string: "\(callbackScheme):/oauth/callback?\(query)") else { throw SignInError.words(RigNet.refusedWords) }
        return u
    }
}

/// A test copy's sheet: none. Nothing opens; the route is refused in the catalogue's words.
struct RigNoSheet: AuthorizationUI {
    func authorize(_ url: URL, callbackScheme: String) async throws -> URL { throw SignInError.words(RigNet.refusedWords) }
}

struct NoSheet: AuthorizationUI {
    func authorize(_ url: URL, callbackScheme: String) async throws -> URL { throw SignInError.words("No sign-in sheet on this platform.") }
}

private func plistId(_ key: String) -> String { (Bundle.main.object(forInfoDictionaryKey: key) as? String) ?? "" }
