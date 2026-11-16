import Foundation

/// Picks the core at launch. `--fixture-core` (or WHOOSH_FIXTURE_CORE=1) serves the design fixture; otherwise the real
/// Rust core (slice 1, 2026-11-09: detection, the sign-in check, Add account) through UniFFI. `UnavailableCore` serves
/// only when the Rust core refuses to open, and then says why. The views read `CoreHost.shared.core` either way.
public final class CoreHost: @unchecked Sendable {
    public enum Kind: String { case fixture, rust, unavailable }
    public static let shared = CoreHost()
    public let core: WhooshCore
    public let kind: Kind
    public let fixture: FixtureCore?
    public let clock = FixtureClock()

    private init() {
        LaunchClock.trace("corehost-begin")
        defer { LaunchClock.trace("corehost-end") }
        let env = ProcessInfo.processInfo.environment
        let wantsFixture = CommandLine.arguments.contains("--fixture-core") || env["WHOOSH_FIXTURE_CORE"] == "1"
        if wantsFixture {
            if let url = FixtureFile.locate(), let f = try? FixtureCore(url: url, clock: clock) {
                core = f; fixture = f; kind = .fixture
            } else {
                core = UnavailableCore(reason: "the design fixture could not be read"); fixture = nil; kind = .unavailable
            }
            return
        }
        let place = CorePlace.current(env: env)
        do {
            // the on-device decision model (Laya) goes in with the core; nothing loads until the label AI asks its first question
            core = try openRustCoreWithDecision(dataDir: place.dataDir.path, rig: place.rig, decision: LayaOnDevice.shared.port); fixture = nil; kind = .rust
            DecisionModels.shared.choose(core.decisionModel()?.model)   // the owner's saved model answers from the first question (SPEC A133)
        } catch let CoreError.Action(failure) {
            core = UnavailableCore(reason: failure.reasonWords); fixture = nil; kind = .unavailable
        } catch {
            core = UnavailableCore(reason: "the local engine did not open: \(error)"); fixture = nil; kind = .unavailable
        }
    }
}

/// Where the real core keeps its database, and whether it runs as the hidden test copy. The test copy (bundle id ending
/// ".rig", or a rig launch naming WHOOSH_SOCKET_DIR / WHOOSH_RIG_HIDDEN / --rig-hidden) keeps its data in its own run
/// folder and its secrets in the rig Keychain namespace, so a test never reads or writes the owner's accounts.
public struct CorePlace: Equatable {
    public let dataDir: URL
    public let rig: Bool
    public static func current(env: [String: String], bundleId: String? = Bundle.main.bundleIdentifier, arguments: [String] = CommandLine.arguments) -> CorePlace {
        let runDir = env["WHOOSH_SOCKET_DIR"].flatMap { $0.isEmpty ? nil : URL(fileURLWithPath: $0, isDirectory: true) }
        let rig = isRig(env: env, bundleId: bundleId, arguments: arguments)
        let support = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        if rig {
            let base = runDir ?? support.appendingPathComponent("Whoosh-rig", isDirectory: true)
            return CorePlace(dataDir: base.appendingPathComponent("core", isDirectory: true), rig: true)
        }
        return CorePlace(dataDir: support.appendingPathComponent("Whoosh/Core", isDirectory: true), rig: false)
    }

    /// The test-copy decision alone (no folder looked up): a bundle id ending ".rig", a non-empty WHOOSH_SOCKET_DIR, WHOOSH_RIG_HIDDEN=1 or --rig-hidden.
    public static func isRig(env: [String: String], bundleId: String? = Bundle.main.bundleIdentifier, arguments: [String] = CommandLine.arguments) -> Bool {
        (bundleId?.hasSuffix(".rig") ?? false) || !(env["WHOOSH_SOCKET_DIR"] ?? "").isEmpty || env["WHOOSH_RIG_HIDDEN"] == "1" || arguments.contains("--rig-hidden")
    }
}

/// The one network rule of a test copy (owner: "nothing leaves this Mac except the relay"). A test copy (a rig, as `CorePlace` decides)
/// may ask only this Mac: a host 127.0.0.1, ::1 or localhost. The owner's copy is never changed by it: with no explicit environment the
/// answer is one read of a flag decided once per process. Every Swift call site is listed in docs/lanes/rig-net-swift.md.
public enum RigNet {
    /// The hosts that are this Mac, exactly as `URL.host` gives them.
    public static let thisMacHosts: Set<String> = ["127.0.0.1", "::1", "localhost"]
    /// An address that refuses at once (port 1 on this Mac): what a test copy asks instead of a host outside this Mac.
    public static let refusing = URL(string: "http://127.0.0.1:1/")!
    /// This process is a test copy: decided once, from the process's own environment, bundle id and arguments.
    public static let onRig: Bool = CorePlace.isRig(env: ProcessInfo.processInfo.environment)

    /// True when the host is this Mac.
    public static func isThisMac(_ host: String?) -> Bool { host.map { thisMacHosts.contains($0) } ?? false }

    /// Whether `env` (nil: this process) is a test copy.
    public static func rig(env: [String: String]?) -> Bool { env.map { CorePlace.isRig(env: $0) } ?? onRig }

    /// True when the address may be asked: always on the owner's copy; on a test copy only when its host is this Mac.
    public static func allows(_ url: URL?, env: [String: String]? = nil) -> Bool {
        guard rig(env: env) else { return true }
        return isThisMac(url?.host)
    }

    /// The address a request may use: the owner's copy, the address as given; a test copy, the address when its host is this Mac,
    /// else `refusing`.
    public static func safe(_ url: URL, env: [String: String]? = nil) -> URL { allows(url, env: env) ? url : refusing }

    /// A web address (http or https) a test copy refuses to open in another app: its host is not this Mac. False on the owner's copy.
    public static func refusesWebOpen(_ url: URL?, env: [String: String]? = nil) -> Bool {
        guard let url, ["http", "https"].contains(url.scheme?.lowercased() ?? "") else { return false }
        return !allows(url, env: env)
    }

    /// The words a test copy shows where it refused to reach a host outside this Mac (sign-in, a provider key check).
    public static var refusedWords: String { Words.text("This test copy reaches only addresses on this Mac, so nothing was sent.") }
}

/// The core when the Rust core did not open: every read is empty, every command and lookup fails in the reason's words.
public final class UnavailableCore: WhooshCore, @unchecked Sendable {
    let reason: String
    init(reason: String) { self.reason = reason }
    public func run(command: Command) throws -> CommandOutcome {
        throw CoreError.Action(failure: ActionFailed(accountName: "Whoosh", reasonWords: reason, retryable: false))
    }
    public func inbox(query: InboxQuery) -> [InboxRow] { [] }
    public func inboxRows(query: InboxQuery, rows: UInt32) -> [InboxRow] { [] }
    public func rowsForMessages(messageIds: [String]) -> [InboxRow] { [] }
    public func senderDomains(prefix: String) -> [SenderDomain] { [] }
    public func counts() -> Counts { Counts(tabs: [], mailboxes: [:]) }
    public func thread(threadId: String) -> MailThread? { nil }
    public func state(path: String) -> String? { nil }
    public func detectProvider(email: String) -> DetectionResult {
        DetectionResult(outcome: .refused(words: "Looking up mail settings is not available: \(reason). Use Manual setup."), stepReached: nil, progress: [], microsoft365Tenant: false, signInComingSoon: nil)
    }
    public func validateAccount(config: AccountConfig, password: SecretText) -> ValidationReport {
        ValidationReport(ok: false, steps: [], roles: [], folderCount: 0, refusedBy: nil, summaryWords: "Checking an account is not available: \(reason).")
    }
    public func subscribe(stream: StreamKind, sink: EventSink) -> UInt64 { 0 }
    public func unsubscribe(subscription: UInt64) {}
}

public extension CorePlace {
    /// True on a rig copy, whatever its core (bundle id `.rig`, or a launch naming WHOOSH_SOCKET_DIR / WHOOSH_RIG_HIDDEN): the one test the
    /// permission seams ask (Calendar, microphone), so a rig on the real core never reaches a system prompt.
    static var onRig: Bool { current(env: ProcessInfo.processInfo.environment).rig }
}
