import Foundation

/// One account on the Welcome screen. Never holds a password: a secret goes to the connector and is dropped.
public struct WelcomeRow: Identifiable, Equatable, Codable, Sendable {
    public enum Phase: String, Codable, Sendable { case waiting, connecting, fetching, healthy, refused }
    public var id: String
    public var address: String
    public var name: String
    public var colour: String
    public var iconId: String?
    public var signature: String
    public var routeName: String
    public var phase: Phase
    /// The server's words when the row is refused or slowed; empty otherwise.
    public var status: String
    public var statusLink: String

    public var progressWords: String {
        switch phase {
        case .waiting: return "waiting for the password"
        case .connecting: return "connecting"
        case .fetching: return "fetching the inbox"
        case .healthy: return "ready"
        case .refused: return status
        }
    }
    public var inFlight: Bool { phase == .connecting || phase == .fetching }
}

/// The rows' colours are data, like the core's: spread round the hue wheel by the golden angle so 30 neighbours differ.
enum WelcomeColour {
    static func hex(forIndex i: Int) -> String {
        let h = (Double(i) * 0.61803398875).truncatingRemainder(dividingBy: 1)
        let (r, g, b) = rgb(h: h, s: 0.62, v: 0.86)
        return String(format: "#%02X%02X%02X", Int(r * 255), Int(g * 255), Int(b * 255))
    }
    static func rgb(h: Double, s: Double, v: Double) -> (Double, Double, Double) {
        let i = Int(h * 6), f = h * 6 - Double(i), p = v * (1 - s), q = v * (1 - f * s), t = v * (1 - (1 - f) * s)
        switch i % 6 { case 0: return (v, t, p); case 1: return (q, v, p); case 2: return (p, v, t)
        case 3: return (p, q, v); case 4: return (t, p, v); default: return (v, p, q) }
    }
    /// The picker's eight swatches.
    static let swatches: [String] = (0..<8).map { hex(forIndex: $0 + 3) }
}

/// What a connection attempt ends in. The secret is consumed here and never stored.
public enum WelcomeOutcome: Equatable, Sendable {
    case healthy
    case refused(words: String, link: String)
}

public protocol WelcomeConnector: Sendable {
    /// Reports progress through `step`; returns how it ended. One call per row, so a refusal stays in its row.
    @MainActor func connect(address: String, route: WelcomeRoute, secret: String?, step: @MainActor (WelcomeRow.Phase) -> Void) async -> WelcomeOutcome
}

/// Fakes only (nothing leaves this Mac): an address whose local part holds "refused" is refused with the route's words;
/// a Microsoft address whose local part holds "admin-blocked" is refused with the sign-in engine's admin line.
public struct WelcomeFakeConnector: WelcomeConnector {
    public init() {}
    /// The fake refusal rules in one place, so the fixture connector and the fake one answer alike.
    @MainActor static func refusal(address: String, route: WelcomeRoute, secret: String?) -> WelcomeOutcome? {
        let a = address.lowercased()
        if route.kind == .microsoft, a.split(separator: "@").first?.contains("admin-blocked") == true {
            return .refused(words: Words.text(SignInWords.adminBlocked), link: "")
        }
        if a.contains("refused") || secret == "wrong-password" { return .refused(words: route.refusedWords(), link: route.link ?? "") }
        return nil
    }
    @MainActor public func connect(address: String, route: WelcomeRoute, secret: String?, step: @MainActor (WelcomeRow.Phase) -> Void) async -> WelcomeOutcome {
        step(.connecting)
        await Task.yield()
        if let refused = Self.refusal(address: address, route: route, secret: secret) { return refused }
        step(.fetching)
        await Task.yield()
        return .healthy
    }
}

/// Under the fixture core: the fake refusal rule first, then the account goes through the core's own addAccount
/// (FixtureCore serves it), so the account really exists in the core. Fakes only: nothing leaves this Mac.
public struct WelcomeFixtureConnector: WelcomeConnector {
    public init() {}
    @MainActor public func connect(address: String, route: WelcomeRoute, secret: String?, step: @MainActor (WelcomeRow.Phase) -> Void) async -> WelcomeOutcome {
        if WelcomeFakeConnector.refusal(address: address, route: route, secret: secret) != nil {
            return await WelcomeFakeConnector().connect(address: address, route: route, secret: secret, step: step)
        }
        return await WelcomeCoreConnector().connect(address: address, route: route, secret: secret, step: step)
    }
}

/// The real path: the core adds the account and says how it went in the server's words (core-sync lanes). Detection, then
/// `AddAccountConfigured` with the address, the detected servers and the password, as the Add account sheet does (WelcomeRealAdd.swift);
/// a row with no password is refused in the sheet's sentence. `core` and `fixture` are the app's own unless a test names them.
public struct WelcomeCoreConnector: Sendable {
    let core: WhooshCore?
    let fixture: Bool?
    public init() { core = nil; fixture = nil }
    init(core: WhooshCore, fixture: Bool = false) { self.core = core; self.fixture = fixture }
}
extension WelcomeCoreConnector: WelcomeConnector {
    @MainActor public func connect(address: String, route: WelcomeRoute, secret: String?, step: @MainActor (WelcomeRow.Phase) -> Void) async -> WelcomeOutcome {
        step(.connecting)
        let model = AppModel.shared
        let theCore = core ?? model.core
        let isFixture = fixture ?? (model.host.fixture != nil)
        let provider: MailProvider = { switch route.kind { case .google: return .google; case .microsoft: return .microsoft
            case .icloud: return .icloud; case .fastmail: return .fastmail; default: return .imap } }()
        let r = await RealAddAccount.add(core: theCore, address: address, provider: provider, secret: secret, oauth: false, fixture: isFixture)
        guard let id = r.id else { return .refused(words: r.words, link: r.link.isEmpty ? (route.link ?? "") : r.link) }
        if core == nil { model.refresh() }
        step(.fetching)
        // the account's own sync state (the last one only under the fixture, which gives no id back)
        let states = theCore.syncStates()
        let sync = states.first { $0.accountId == id } ?? (id.isEmpty ? states.last : nil)
        if let e = sync?.lastError { return .refused(words: e.reasonWords, link: route.link ?? "") }
        return .healthy
    }
}
