import Foundation
import Testing
@testable import WhooshUI

// Welcome's real path against a core that behaves like the real engine: it REFUSES the address-only `.addAccount` (naming
// AddAccountConfigured) and accepts `.addAccountConfigured`. No server, no socket, no app window. The password in these tests is a stub
// string; it is only compared as a Bool, never printed. Every test carries a time limit.

/// A core that answers like the real engine's account commands, and records every command it is given.
final class RealishCore: WhooshCore, @unchecked Sendable {
    private let lock = NSLock()
    private var sent: [Command] = []
    var detection: DetectionOutcome
    var report: ValidationReport
    var configuredRefusal: ActionFailed?
    var detectCalls = 0
    init(detection: DetectionOutcome = RealishCore.found(), report: ValidationReport = RealishCore.passing(), configuredRefusal: ActionFailed? = nil) {
        self.detection = detection; self.report = report; self.configuredRefusal = configuredRefusal
    }
    var commands: [Command] { lock.withLock { sent } }
    static func found() -> DetectionOutcome {
        .found(config: AccountConfig(imapHost: "imap.example.test", imapPort: 993, imapSecurity: .ssl, smtpHost: "smtp.example.test", smtpPort: 587,
                                     smtpSecurity: .starttls, username: "ana@example.test", smtpUsername: "ana@example.test", authMethod: .password, source: .ispdb))
    }
    static func passing() -> ValidationReport { ValidationReport(ok: true, steps: [], roles: [], folderCount: 3, refusedBy: nil, summaryWords: "ok") }

    func run(command: Command) throws -> CommandOutcome {
        lock.withLock { sent.append(command) }
        switch command {
        case .addAccount:
            throw CoreError.Action(failure: ActionFailed(accountName: "Whoosh", reasonWords: "This engine adds an account with AddAccountConfigured.", retryable: false, code: "account.use_configured"))
        case .addAccountConfigured:
            if let f = configuredRefusal { throw CoreError.Action(failure: f) }
            return CommandOutcome(undoEntryId: nil, createdId: "acct-1")
        default: return CommandOutcome(undoEntryId: nil, createdId: nil)
        }
    }
    func inbox(query: InboxQuery) -> [InboxRow] { [] }
    func inboxRows(query: InboxQuery, rows: UInt32) -> [InboxRow] { [] }   // the paged list read the engine gained on 14 Nov
    func rowsForMessages(messageIds: [String]) -> [InboxRow] { [] }
    func senderDomains(prefix: String) -> [SenderDomain] { [] }
    func counts() -> Counts { Counts(tabs: [], mailboxes: [:]) }
    func thread(threadId: String) -> MailThread? { nil }
    func state(path: String) -> String? { nil }
    func subscribe(stream: StreamKind, sink: EventSink) -> UInt64 { 0 }
    func unsubscribe(subscription: UInt64) {}
    func detectProvider(email: String) -> DetectionResult {
        lock.withLock { detectCalls += 1 }
        return DetectionResult(outcome: detection, stepReached: nil, progress: [], microsoft365Tenant: false, signInComingSoon: nil)
    }
    func validateAccount(config: AccountConfig, password: SecretText) -> ValidationReport { report }
}

/// The configured command's parts, without a password in any message.
struct ConfiguredSent { let address: String; let config: AccountConfig; let passwordIs: (String) -> Bool }
func configuredSent(_ core: RealishCore) -> ConfiguredSent? {
    for c in core.commands { if case .addAccountConfigured(let address, _, let config, let password) = c { return ConfiguredSent(address: address, config: config, passwordIs: { $0 == password.value }) } }
    return nil
}
func sentAddAccountCount(_ core: RealishCore) -> Int { core.commands.filter { if case .addAccount = $0 { return true } else { return false } }.count }

@MainActor private func connect(_ core: RealishCore, _ address: String, secret: String?) async -> (WelcomeOutcome, [WelcomeRow.Phase]) {
    var phases: [WelcomeRow.Phase] = []
    let outcome = await WelcomeCoreConnector(core: core).connect(address: address, route: WelcomeRoutes.detect(address), secret: secret) { phases.append($0) }
    return (outcome, phases)
}

@MainActor @Suite(.timeLimit(.minutes(1))) struct WelcomeRealAddTests {
    @Test func sendsAddAccountConfiguredWithAddressPasswordAndDetectedServers() async throws {
        let core = RealishCore()
        let (outcome, phases) = await connect(core, "Ana@Example.test", secret: "stub-secret-1")
        #expect(outcome == .healthy)
        #expect(phases == [.connecting, .fetching])
        #expect(sentAddAccountCount(core) == 0)                        // never the address-only command the real engine refuses
        let sent = try #require(configuredSent(core))
        #expect(sent.address == "ana@example.test")                    // trimmed, lower-cased as the sheet does
        #expect(sent.config.imapHost == "imap.example.test")           // the detected servers
        #expect(sent.config.smtpHost == "smtp.example.test")
        #expect(sent.config.authMethod == .password)
        #expect(sent.passwordIs("stub-secret-1"))                      // the password route
    }

    @Test func detectionRefusalShowsItsSentenceInPlaceAndSendsNothing() async {
        let core = RealishCore(detection: .refused(words: "No mail server could be found for example.test."))
        let (outcome, _) = await connect(core, "ana@example.test", secret: "stub-secret-1")
        #expect(outcome == .refused(words: "No mail server could be found for example.test.", link: ""))
        #expect(core.commands.isEmpty)
    }

    @Test func aServerWithNoSendingHalfSaysTheSheetsOutgoingSentence() async {
        let core = RealishCore(detection: .imapOnly(imapHost: "imap.example.test", imapPort: 993, imapSecurity: .ssl, username: "ana", source: .guess))
        let (outcome, _) = await connect(core, "ana@example.test", secret: "stub-secret-1")
        #expect(outcome == .refused(words: AddAccountWords.outgoingMissing, link: ""))
        #expect(core.commands.isEmpty)
    }

    @Test func noPasswordIsRefusedInTheSheetsSentenceBeforeAnythingIsAsked() async {
        let core = RealishCore()
        let (outcome, _) = await connect(core, "ana@example.test", secret: nil)
        #expect(outcome == .refused(words: AddAccountWords.needPassword, link: ""))
        #expect(core.detectCalls == 0 && core.commands.isEmpty)
    }

    @Test func aRefusedPasswordShowsTheKnownCauseSentenceAndAddsNothing() async {
        let bad = ValidationReport(ok: false, steps: [], roles: [], folderCount: 0, refusedBy: "imap.example.test", summaryWords: "NO [AUTHENTICATIONFAILED] raw server text", cause: .badPassword)
        let core = RealishCore(report: bad)
        let (outcome, _) = await connect(core, "ana@example.test", secret: "stub-secret-1")
        #expect(outcome == .refused(words: AddAccountWords.causeSentence(.badPassword), link: ""))
        #expect(configuredSent(core) == nil)
    }

    @Test func theEnginesRefusalOfTheConfiguredCommandReadsAsTheCatalogueSentence() async {
        let core = RealishCore(configuredRefusal: ActionFailed(accountName: "Whoosh", reasonWords: "raw engine words", retryable: false, code: "engine.not_built_yet"))
        let (outcome, _) = await connect(core, "ana@example.test", secret: "stub-secret-1")
        #expect(outcome == .refused(words: EngineRefusal.sentence("engine.not_built_yet"), link: ""))
    }
}
