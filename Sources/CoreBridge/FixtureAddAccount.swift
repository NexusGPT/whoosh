import Foundation

/// Add account by IMAP in the fixture core (delta 8, group 1): recorded detections and validations per fixture domain.
/// Generated once from fixtures/addaccount/*.json (the J4 journey's expectations) and kept by hand after; no network is touched.
/// Differences from those files, each deliberate: fastmail.com is found by DNS SRV at 465 implicit TLS (docs/JOURNEYS.md J4.5 and the
/// recorded provider-database 404 in fixtures/autoconfig), and the progress lines are the autoconfig crate's own words.
final class FixtureAddAccountBook {
    struct Record {
        let domain: String
        let trace: [(DetectionStep, Bool)]
        let config: AccountConfig?
        let tenant: Bool
        let comingSoon: String?
        let validates: Bool
        /// the one password the recorded server accepts; nil = it refuses every password (the wrong-password case)
        let accepts: String?
        let folders: [(String, [String])]
        let roles: [String: String]
    }

    static let records: [Record] = [
        Record(domain: "blocked-m365.example.test", trace: [(.ispdb, false), (.autoconfigUrls, false), (.srv, false), (.autodiscover, true)], config: AccountConfig(imapHost: "outlook.office365.com", imapPort: 993, imapSecurity: .ssl, smtpHost: "smtp.office365.com", smtpPort: 587, smtpSecurity: .starttls, username: "assem.fixture@blocked-m365.example.test", smtpUsername: "assem.fixture@blocked-m365.example.test", authMethod: .oauth2, source: .autodiscover),
               tenant: true, comingSoon: "Sign in with Microsoft: coming soon", validates: false, accepts: nil, folders: [], roles: [:]),
        Record(domain: "fastmail.com", trace: [(.ispdb, false), (.autoconfigUrls, false), (.srv, true)], config: AccountConfig(imapHost: "imap.fastmail.com", imapPort: 993, imapSecurity: .ssl, smtpHost: "smtp.fastmail.com", smtpPort: 465, smtpSecurity: .ssl, username: "assem.fixture@fastmail.com", smtpUsername: "assem.fixture@fastmail.com", authMethod: .password, source: .srv),
               tenant: false, comingSoon: nil, validates: true, accepts: "fixture-password", folders: [("INBOX", []), ("Sent", ["\\Sent"]), ("Drafts", ["\\Drafts"]), ("Trash", ["\\Trash"]), ("Spam", ["\\Junk"])], roles: ["sent": "Sent", "drafts": "Drafts", "trash": "Trash", "junk": "Spam"]),
        Record(domain: "gmail.com", trace: [(.ispdb, true)], config: AccountConfig(imapHost: "imap.gmail.com", imapPort: 993, imapSecurity: .ssl, smtpHost: "smtp.gmail.com", smtpPort: 465, smtpSecurity: .ssl, username: "assem.fixture@gmail.com", smtpUsername: "assem.fixture@gmail.com", authMethod: .oauth2, source: .ispdb),
               tenant: false, comingSoon: "Sign in with Google: coming soon", validates: true, accepts: "fixture-password", folders: [("INBOX", []), ("Sent Mail", ["\\Sent"]), ("Drafts", ["\\Drafts"]), ("Trash", ["\\Trash"]), ("Spam", ["\\Junk"])], roles: ["sent": "Sent Mail", "drafts": "Drafts", "trash": "Trash", "junk": "Spam"]),
        Record(domain: "guess-only.example.test", trace: [(.ispdb, false), (.autoconfigUrls, false), (.srv, false), (.autodiscover, false), (.guess, true)], config: AccountConfig(imapHost: "imap.guess-only.example.test", imapPort: 993, imapSecurity: .ssl, smtpHost: "smtp.guess-only.example.test", smtpPort: 587, smtpSecurity: .starttls, username: "assem.fixture@guess-only.example.test", smtpUsername: "assem.fixture@guess-only.example.test", authMethod: .password, source: .guess),
               tenant: false, comingSoon: nil, validates: true, accepts: "fixture-password", folders: [("INBOX", []), ("Sent", []), ("Drafts", []), ("Trash", []), ("Junk", [])], roles: ["sent": "Sent", "drafts": "Drafts", "trash": "Trash", "junk": "Junk"]),
        Record(domain: "icloud.com", trace: [(.ispdb, true)], config: AccountConfig(imapHost: "imap.mail.me.com", imapPort: 993, imapSecurity: .ssl, smtpHost: "smtp.mail.me.com", smtpPort: 587, smtpSecurity: .starttls, username: "assem.fixture", smtpUsername: "assem.fixture@icloud.com", authMethod: .password, source: .ispdb),
               tenant: false, comingSoon: nil, validates: true, accepts: "fixture-password", folders: [("INBOX", []), ("Sent Messages", ["\\Sent"]), ("Drafts", ["\\Drafts"]), ("Deleted Messages", ["\\Trash"]), ("Junk", ["\\Junk"])], roles: ["sent": "Sent Messages", "drafts": "Drafts", "trash": "Deleted Messages", "junk": "Junk"]),
        Record(domain: "m365.example.test", trace: [(.ispdb, false), (.autoconfigUrls, false), (.srv, false), (.autodiscover, true)], config: AccountConfig(imapHost: "outlook.office365.com", imapPort: 993, imapSecurity: .ssl, smtpHost: "smtp.office365.com", smtpPort: 587, smtpSecurity: .starttls, username: "assem.fixture@m365.example.test", smtpUsername: "assem.fixture@m365.example.test", authMethod: .oauth2, source: .autodiscover),
               tenant: true, comingSoon: "Sign in with Microsoft: coming soon", validates: true, accepts: "fixture-password", folders: [("INBOX", []), ("Sent Items", ["\\Sent"]), ("Drafts", ["\\Drafts"]), ("Deleted Items", ["\\Trash"]), ("Junk Email", ["\\Junk"])], roles: ["sent": "Sent Items", "drafts": "Drafts", "trash": "Deleted Items", "junk": "Junk Email"]),
        Record(domain: "me.com", trace: [(.ispdb, true)], config: AccountConfig(imapHost: "imap.mail.me.com", imapPort: 993, imapSecurity: .ssl, smtpHost: "smtp.mail.me.com", smtpPort: 587, smtpSecurity: .starttls, username: "assem.fixture", smtpUsername: "assem.fixture@me.com", authMethod: .password, source: .ispdb),
               tenant: false, comingSoon: nil, validates: false, accepts: nil, folders: [], roles: [:]),
        Record(domain: "nothing.example.test", trace: [(.ispdb, false), (.autoconfigUrls, false), (.srv, false), (.autodiscover, false), (.guess, false)], config: nil,
               tenant: false, comingSoon: nil, validates: false, accepts: nil, folders: [], roles: [:]),
        Record(domain: "outlook.com", trace: [(.ispdb, true)], config: AccountConfig(imapHost: "outlook.office365.com", imapPort: 993, imapSecurity: .ssl, smtpHost: "smtp.office365.com", smtpPort: 587, smtpSecurity: .starttls, username: "assem.fixture@outlook.com", smtpUsername: "assem.fixture@outlook.com", authMethod: .oauth2, source: .ispdb),
               tenant: false, comingSoon: "Sign in with Microsoft: coming soon", validates: false, accepts: nil, folders: [], roles: [:]),
        Record(domain: "srv.example.test", trace: [(.ispdb, false), (.autoconfigUrls, false), (.srv, true)], config: AccountConfig(imapHost: "imap.srv.example.test", imapPort: 993, imapSecurity: .ssl, smtpHost: "smtp.srv.example.test", smtpPort: 587, smtpSecurity: .starttls, username: "assem.fixture@srv.example.test", smtpUsername: "assem.fixture@srv.example.test", authMethod: .password, source: .srv),
               tenant: false, comingSoon: nil, validates: true, accepts: "fixture-password", folders: [("INBOX", []), ("Verzonden", ["\\Sent"]), ("Concepten", ["\\Drafts"]), ("Prullenbak", ["\\Trash"]), ("Ongewenst", ["\\Junk"])], roles: ["sent": "Verzonden", "drafts": "Concepten", "trash": "Prullenbak", "junk": "Ongewenst"]),
    ]

    private(set) var detectionCount = 0
    private(set) var last: (address: String, outcome: String, step: String)?
    private(set) var lastValidation: ValidationReport?
    /// SPEC A87: `fixture refuse <cause>` arms the next check to fail with that cause's recorded server words, once.
    var armedRefusal: RefusalCause?
    /// `fixture refuse <code>` with an engine code: the next add or sign-in start fails with that code and the engine's words, once.
    var armedEngineCode: String?
    static let engineWords: [String: String] = [
        "engine.not_built_yet": "The local engine does not do this yet.", "oauth.no_client_id": "no client id is set for this sign-in",
        "oauth.sign_in_not_done": "the sign-in in the browser was not finished", "oauth.refused": "the provider refused the sign-in",
        "oauth.no_refresh_token": "the provider gave no refresh token", "oauth.browser": "the browser could not be opened",
        "oauth.consent_required": "the provider refused the sign-in: AADSTS65001: The user or administrator has not consented to use the application."]
    static func engineFailure(_ code: String) -> CoreError {
        let words = engineWords[code] ?? "engine words for an unknown code"
        return CoreError.Action(failure: ActionFailed(accountName: "", reasonWords: words, retryable: false, code: code))
    }
    private(set) var cache: [String: String] = [:]               // domain -> source word (detection_cache)
    private(set) var configs: [String: AccountConfig] = [:]      // account id -> the saved server config

    private(set) var folderMaps: [String: [String: String]] = [:]   // account id -> role -> folder, as the recorded server mapped them

    /// The recorded case of an address: its own domain's record when that record carries these servers (two recorded domains share outlook.office365.com
    /// and imap.mail.me.com, so the host alone picks the wrong one), else the first record with these servers.
    func record(host: String, address: String?) -> Record? {
        if let a = address, let d = Self.domain(of: a), let r = Self.records.first(where: { $0.domain == d && $0.config?.imapHost == host }) { return r }
        return Self.records.first { $0.config?.imapHost == host }
    }

    func save(_ id: String, _ config: AccountConfig, address: String? = nil) {
        configs[id] = config
        if let r = record(host: config.imapHost, address: address) { folderMaps[id] = r.roles }
    }

    /// `account/<id>/<field>` once an account was added with a configuration (the password is not a field).
    func configValue(_ id: String, _ field: String) -> Any? {
        guard let c = configs[id] else { return nil }
        if field == "folders" { return folderMaps[id] ?? [:] }   // role -> folder, the mapping the validation recorded
        func sec(_ s: ConnectionSecurity) -> String { s == .ssl ? "ssl" : "starttls" }
        switch field {
        case "imap_host": return c.imapHost
        case "imap_port": return Int(c.imapPort)
        case "imap_security": return sec(c.imapSecurity)
        case "smtp_host": return c.smtpHost
        case "smtp_port": return Int(c.smtpPort)
        case "smtp_security": return sec(c.smtpSecurity)
        case "username": return c.username
        case "smtp_username": return c.smtpUsername
        case "auth_method": return c.authMethod == .password ? "password" : "oauth2"
        case "detection_source": return Self.sourceWord(c.source)
        default: return nil
        }
    }

    func lastValue(_ field: String) -> Any? {
        guard let l = last else { return nil }
        switch field { case "address": return l.address; case "outcome": return l.outcome; case "step": return l.step; default: return nil }
    }

    func validationValue(_ field: String) -> Any? {
        guard let v = lastValidation else { return nil }
        switch field {
        case "ok": return v.ok
        case "refused_by": return v.refusedBy
        case "summary": return v.summaryWords
        case "steps": return v.steps.count
        default: return nil
        }
    }

    func reset() { detectionCount = 0; last = nil; lastValidation = nil; cache = [:]; configs = [:] }

    static func domain(of address: String) -> String? {
        let parts = address.trimmingCharacters(in: .whitespaces).lowercased().split(separator: "@", omittingEmptySubsequences: false)
        guard parts.count == 2, !parts[0].isEmpty, parts[1].contains("."), !parts[1].hasPrefix("."), !parts[1].hasSuffix("."), !parts[1].contains(" ") else { return nil }
        return String(parts[1])
    }

    static func sourceWord(_ s: DetectionSource) -> String {
        switch s { case .ispdb: return "ispdb"; case .autoconfig: return "autoconfig"; case .srv: return "srv"; case .autodiscover: return "autodiscover"; case .mailRecord: return "mx"; case .guess: return "guess"; case .manual: return "manual" }
    }

    func detect(_ email: String) -> DetectionResult {
        guard let domain = Self.domain(of: email) else {
            last = (email, "refused", "none"); detectionCount += 1
            return DetectionResult.make(outcome: .refused(words: "This address cannot be used: it needs a name, an @ and a domain such as example.com."), progress: [], tenant: false, comingSoon: nil)
        }
        let record = Self.records.first { $0.domain == domain }
        let trace: [(DetectionStep, Bool)] = record?.trace ?? [(.ispdb, false), (.autoconfigUrls, false), (.srv, false), (.autodiscover, false), (.guess, false)]
        let progress = trace.map { DetectionProgress(step: $0.0, found: $0.1, words: $0.0.words) }
        let outcome: DetectionOutcome
        if let c = record?.config { outcome = .found(config: c) }
        else { outcome = .refused(words: "No mail settings were found for \(domain): no provider database entry, no autoconfig file, no DNS mail record, no Autodiscover answer, and no server answered on the usual ports. Use Manual setup.") }
        let soon: String?
        if let s = record?.comingSoon { soon = s }
        else if record?.config?.authMethod == .oauth2 { soon = "Sign in is coming soon" }
        else { soon = nil }
        detectionCount += 1
        if let c = record?.config { cache[domain] = Self.sourceWord(c.source) } else { cache[domain] = nil }
        let result = DetectionResult.make(outcome: outcome, progress: progress, tenant: record?.tenant ?? false, comingSoon: soon)
        last = (email.lowercased(), { if case .found = outcome { return "found" } else { return "refused" } }(), result.stepReached.map { "\($0)" } ?? "none")
        return result
    }

    /// A real login is replaced by the recorded server: the host that refuses and the words are the recorded case's.
    func validate(_ config: AccountConfig, password: String, address: String? = nil) -> ValidationReport {
        let report: ValidationReport
        if let cause = armedRefusal { armedRefusal = nil; report = Self.refusal(cause, config) } else { report = check(config, password: password, address: address ?? last?.address) }   // the contract's validateAccount carries no address: the one just detected stands
        lastValidation = report
        return report
    }

    /// The server's raw words for each cause, as real servers word them (the owner's 14:38 Microsoft text first). The sheet
    /// must show the cause's plain sentence and keep these behind Details.
    static func rawWords(_ cause: RefusalCause, _ c: AccountConfig) -> (ValidationStepKind, String) {
        switch cause {
        case .basicAuthDisabled: return (.imapLogin, "The server refused the sign-in: code: None, info: Some(\"Basic authentication is disabled.\")")
        case .badPassword: return (.imapLogin, "The server refused the sign-in: [AUTHENTICATIONFAILED] Invalid credentials (Failure)")
        case .hostUnreachable: return (.imapConnect, "Cannot reach \(c.imapHost) on port \(c.imapPort): Connection refused (os error 61). Check the server name and the port.")
        case .tlsFailed: return (.imapTls, "The secure connection to \(c.imapHost) failed: invalid peer certificate: UnknownIssuer.")
        case .smtpRefused: return (.smtpAuth, "535 5.7.8 Authentication credentials invalid")
        case .unknown: return (.imapList, "The server refused the folder list: NO [SERVERBUG] internal error")
        }
    }

    static func refusal(_ cause: RefusalCause, _ c: AccountConfig) -> ValidationReport {
        let (kind, words) = rawWords(cause, c)
        let order: [ValidationStepKind] = [.imapConnect, .imapTls, .imapLogin, .imapList, .smtpEhlo, .smtpAuth]
        var steps = order.prefix { $0 != kind }.map { ValidationStep(kind: $0, ok: true, words: "fine") }
        steps.append(ValidationStep(kind: kind, ok: false, words: words))
        let host = kind == .smtpAuth ? c.smtpHost : c.imapHost
        return ValidationReport.fromSteps(steps: steps, roles: [], folderCount: 0, failingHost: host)
    }

    private func check(_ config: AccountConfig, password: String, address: String?) -> ValidationReport {
        let rec = record(host: config.imapHost, address: address)
        func step(_ k: ValidationStepKind, _ ok: Bool, _ w: String) -> ValidationStep { ValidationStep(kind: k, ok: ok, words: w) }
        let connect = step(.imapConnect, true, "Connected to \(config.imapHost).")
        let tls = step(.imapTls, true, config.imapSecurity == .ssl ? "Secure connection set up." : "Secure connection started with STARTTLS.")
        guard let rec else {
            return ValidationReport.fromSteps(steps: [step(.imapConnect, false, "\(config.imapHost) did not answer on port \(config.imapPort). Check the server name and the port.")], roles: [], folderCount: 0, failingHost: config.imapHost)
        }
        if password.isEmpty {
            return ValidationReport.fromSteps(steps: [connect, tls, step(.imapLogin, false, "\(config.imapHost) needs a password to sign in.")], roles: [], folderCount: 0, failingHost: config.imapHost)
        }
        if config.authMethod == .oauth2 && rec.comingSoon != nil && rec.config?.imapHost == "outlook.office365.com" && !rec.validates {
            return ValidationReport.fromSteps(steps: [connect, tls, step(.imapLogin, false, "\(config.imapHost) does not accept a password for this address (Basic authentication is disabled.)")], roles: [], folderCount: 0, failingHost: config.imapHost)
        }
        guard rec.validates, let ok = rec.accepts else {
            let words: String
            if rec.tenant { words = "\(config.imapHost): this organisation blocks password sign-in for IMAP (Basic authentication is disabled.)" }
            else if config.imapHost == "outlook.office365.com" { words = "\(config.imapHost) does not accept a password for this address (Basic authentication is disabled.)" }
            else { words = "\(config.imapHost) refused the sign-in: wrong password. If this account uses an app-specific password, make one at https://appleid.apple.com and paste it here." }
            return ValidationReport.fromSteps(steps: [connect, tls, step(.imapLogin, false, words)], roles: [], folderCount: 0, failingHost: config.imapHost)
        }
        guard password == ok else {
            // iCloud wants an app-specific password: the line carries Apple's page (J4.4)
            let link = config.imapHost.hasSuffix("me.com") ? " Make an app-specific password at https://appleid.apple.com and paste it here." : ""
            return ValidationReport.fromSteps(steps: [connect, tls, step(.imapLogin, false, "\(config.imapHost) refused the sign-in: wrong password.\(link)")], roles: [], folderCount: 0, failingHost: config.imapHost)
        }
        let roles = rec.roles.sorted { $0.key < $1.key }.map { role, folder in
            FolderRoleMapping(role: role, folder: folder, bySpecialUse: !(rec.folders.first { $0.0 == folder }?.1.isEmpty ?? true))
        }
        let steps = [connect, tls, step(.imapLogin, true, "Signed in to \(config.imapHost)."), step(.imapList, true, "Read \(rec.folders.count) folders."),
                     step(.smtpEhlo, true, "\(config.smtpHost) answered."), step(.smtpAuth, true, "Signed in to \(config.smtpHost) for sending.")]
        return ValidationReport.fromSteps(steps: steps, roles: roles, folderCount: UInt32(rec.folders.count), failingHost: nil)
    }
}

// The Swift twins of the contract crate's derivations (UniFFI exports no record constructors). The contract tests in
// core/crates/contracts/tests/delta8.rs prove the Rust side; Tests/Support/RootDelta8Tests.swift proves these equal.
extension DetectionResult {
    static func make(outcome: DetectionOutcome, progress: [DetectionProgress], tenant: Bool, comingSoon: String?) -> DetectionResult {
        DetectionResult(outcome: outcome, stepReached: progress.last?.step, progress: progress, microsoft365Tenant: tenant, signInComingSoon: comingSoon)
    }
}

extension ValidationReport {
    static func fromSteps(steps: [ValidationStep], roles: [FolderRoleMapping], folderCount: UInt32, failingHost: String?) -> ValidationReport {
        let ok = steps.allSatisfy(\.ok) && steps.contains { $0.kind == .imapList }
        let failure = steps.first { !$0.ok }
        let summary: String
        if ok { summary = "The mail server and the sending server both answered. \(folderCount) folders found." }
        else if let failure { summary = failure.words }
        else { summary = "The folder list was not read, so the account cannot be added yet." }
        // SPEC A87: the contract's own classifier (exported), so the twin cannot disagree with the Rust side
        let cause: RefusalCause? = ok ? nil : (refusalCause(steps: steps) ?? .unknown)
        return ValidationReport(ok: ok, steps: steps, roles: roles, folderCount: folderCount, refusedBy: ok ? nil : failingHost, summaryWords: summary, cause: cause)
    }
}

extension DetectionStep {
    /// The progress line naming the step (J4.1); the same words as `DetectionStep::words` in the contract crate (UniFFI exports no methods on enums).
    var words: String {
        switch self {
        case .ispdb: return "Looking in the provider database…"
        case .autoconfigUrls: return "Asking the domain for its mail settings…"
        case .srv: return "Reading the domain's DNS mail records…"
        case .autodiscover: return "Asking Microsoft Autodiscover…"
        case .mailRecord: return "Reading the domain's mail record…"   // delta 14
        case .guess: return "Trying the usual server names…"
        }
    }
}
