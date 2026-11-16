import Foundation

/// The eight server fields as the screen edits them. Text, so a half-typed port is never lost; `config(address:)` turns
/// them into the contract's AccountConfig or says in words what is missing.
struct AddAccountDraft: Equatable {
    static let fieldIds = ["imapHost", "imapPort", "imapSecurity", "smtpHost", "smtpPort", "smtpSecurity", "username", "authMethod"]
    var imapHost = "", imapPort = "", imapSecurity = "", smtpHost = "", smtpPort = "", smtpSecurity = "", username = "", authMethod = ""
    var smtpUsername = ""
    var source: DetectionSource = .manual

    init() {}

    init(_ c: AccountConfig) {
        imapHost = c.imapHost; imapPort = String(c.imapPort); imapSecurity = Self.word(c.imapSecurity)
        smtpHost = c.smtpHost; smtpPort = String(c.smtpPort); smtpSecurity = Self.word(c.smtpSecurity)
        username = c.username; smtpUsername = c.smtpUsername; authMethod = c.authMethod == .password ? "password" : "oauth2"
        source = c.source
    }

    /// The guess found a mail server and no sending server: the IMAP half is filled, the rest is left to the owner.
    init(imapOnly host: String, port: UInt16, security: ConnectionSecurity, username: String, source: DetectionSource) {
        imapHost = host; imapPort = String(port); imapSecurity = Self.word(security); self.username = username; self.source = source
    }

    /// Manual setup starts from the usual ports and security (993 SSL in, 587 STARTTLS out), only where a field is empty.
    mutating func fillUsual() {
        if imapPort.isEmpty { imapPort = "993" }; if imapSecurity.isEmpty { imapSecurity = "ssl" }
        if smtpPort.isEmpty { smtpPort = "587" }; if smtpSecurity.isEmpty { smtpSecurity = "starttls" }
    }

    static func word(_ s: ConnectionSecurity) -> String { s == .ssl ? "ssl" : "starttls" }

    /// The value a row shows: a port as a number, the rest as text.
    func value(_ field: String) -> Any {
        switch field {
        case "imapPort": return Int(imapPort) ?? imapPort
        case "smtpPort": return Int(smtpPort) ?? smtpPort
        default: return self[field]
        }
    }

    subscript(field: String) -> String {
        get {
            switch field {
            case "imapHost": return imapHost; case "imapPort": return imapPort; case "imapSecurity": return imapSecurity
            case "smtpHost": return smtpHost; case "smtpPort": return smtpPort; case "smtpSecurity": return smtpSecurity
            case "username": return username; case "authMethod": return authMethod
            default: return ""
            }
        }
        set {
            switch field {
            case "imapHost": imapHost = newValue; case "imapPort": imapPort = newValue; case "imapSecurity": imapSecurity = newValue
            case "smtpHost": smtpHost = newValue; case "smtpPort": smtpPort = newValue; case "smtpSecurity": smtpSecurity = newValue
            case "username": username = newValue; case "authMethod": authMethod = newValue
            default: break
            }
        }
    }

    /// The configuration to check. The password route is the only one that works today, so a provider that prefers OAuth2
    /// is checked with the app password the owner pasted (sign-in with the provider is marked coming soon).
    func config(address: String) -> Result<AccountConfig, AddAccountProblem> {
        func port(_ s: String) -> UInt16? { UInt16(s.trimmingCharacters(in: .whitespaces)).flatMap { $0 > 0 ? $0 : nil } }
        func security(_ s: String) -> ConnectionSecurity? { s == "ssl" ? .ssl : s == "starttls" ? .starttls : nil }
        let ih = imapHost.trimmingCharacters(in: .whitespaces), sh = smtpHost.trimmingCharacters(in: .whitespaces)
        guard !ih.isEmpty, let ip = port(imapPort), let isec = security(imapSecurity) else { return .failure(.incoming) }
        guard !sh.isEmpty, let sp = port(smtpPort), let ssec = security(smtpSecurity) else { return .failure(.outgoing) }
        let user = username.trimmingCharacters(in: .whitespaces).isEmpty ? address : username.trimmingCharacters(in: .whitespaces)
        let smtpUser = smtpUsername.isEmpty || source == .manual ? user : smtpUsername
        return .success(AccountConfig(imapHost: ih, imapPort: ip, imapSecurity: isec, smtpHost: sh, smtpPort: sp, smtpSecurity: ssec,
                                      username: user, smtpUsername: smtpUser, authMethod: .password, source: source))
    }
}

enum AddAccountProblem: Error { case incoming, outgoing }
