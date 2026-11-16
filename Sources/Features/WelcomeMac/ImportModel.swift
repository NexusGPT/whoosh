import Foundation
import Observation

/// Lets the core cross into the one background task of a scan (the core is thread-safe by contract; FixtureCore says the same with @unchecked Sendable).
private final class CoreBox: @unchecked Sendable { let core: WhooshCore; init(_ core: WhooshCore) { self.core = core } }

/// The optional import step of the first start, and the same panel behind Settings > Accounts "Import mail…".
/// Reads `import/sources`, `import/<run>/progress` and `import/<run>/result`; writes `ImportScan`, `ImportStart`, `ImportCancel` (docs/lanes/contracts.md
/// "Delta 18"). Skip is no command. A run is watched by one 250 ms read while it is running and by nothing else; no input waits on it.
@MainActor @Observable
public final class ImportModel {
    public static let shared = ImportModel()
    static let offerDoneKey = "whoosh.import.offerDone"

    var sources: [ImportSourceItem] = []
    var selected: String?
    /// nil = the matching account by receiving address, else Imported (the contract's `ImportInto::Imported`).
    var intoAccount: String?
    var accounts: [ImportAccountItem] = []
    var run: ImportRunItem?
    var result: ImportResultItem?
    /// A refusal of a scan, a start or a cancel, already in the app's language.
    var notice = ""
    var scanning = false
    var scanned = false
    /// Skip or a finished run ends the first-start offer for good (kept in UserDefaults, never a secret); Settings still offers it.
    var skipped = UserDefaults.standard.bool(forKey: ImportModel.offerDoneKey)
    /// The panel was opened on purpose (the Settings offer); the first start shows it by itself once an account is connected.
    var open = false
    var fromSettings = false
    /// Presses of "Open Full Disk Access" that a rig copy counted instead of opening the real System Settings window (a person's copy opens it).
    var fdaPresses = 0
    var connected = false
    var picked: [String] = []
    @ObservationIgnored var poll: Task<Void, Never>?

    var showing: Bool { open || (connected && !skipped) }
    var selectedSource: ImportSourceItem? { sources.first { $0.id == selected } }
    var running: Bool { run.map { !$0.isOver } ?? false }
    var canStart: Bool { (selectedSource?.readable ?? false) && !running && !scanning }

    /// The refusal in view, in words: a refused action, else the run's own refusal, else why the chosen source cannot be read yet.
    var refusalWords: String? {
        if !notice.isEmpty { return notice }
        if let r = run, let w = r.refusalWords { return ImportWords.refusal(code: r.refusalCode ?? "", words: w) }
        guard let s = selectedSource, let code = ImportWords.accessCode(s.access) else { return nil }
        return ImportWords.refusal(code: code, words: s.words)
    }
    var needsFullDiskAccess: Bool { selectedSource?.access == "needs_full_disk_access" || run?.refusalCode == "import_needs_full_disk_access" }
    var progressLine: String { run.map { ImportWords.progress(done: $0.done, total: $0.total, current: $0.current) } ?? "" }
    var finishLine: String {
        guard let r = run, r.isOver, r.state == "finished", let x = result else { return run?.state == "cancelled" ? ImportWords.cancelled : "" }
        return ImportWords.finished(new: x.stored, already: x.alreadyThere, left: x.skipped)
    }

    // MARK: scan
    func reconnect() {
        let list = CoreHost.shared.core.accounts()
        connected = !list.isEmpty
        accounts = list.map { ImportAccountItem(id: $0.id, name: AccountNames.display($0)) }   // the Imported account's name in the app's language (imported-name)
    }

    func scan(picking path: String? = nil) {
        guard !scanning else { return }
        stage(path)
        let box = CoreBox(CoreHost.shared.core), paths = picked
        Task { [self] in
            let refused: (String, String)? = await Task.detached { () -> (String, String)? in
                do { _ = try box.core.run(command: .importScan(pickedPaths: paths)); return nil } catch { return Self.refusalOf(error) }
            }.value
            finishScan(refused, path)
        }
    }

    /// The same scan on the calling thread: the rig's doors use it so a read right after a click sees the result.
    func scanNow(picking path: String? = nil) {
        stage(path)
        var refused: (String, String)?
        do { _ = try CoreHost.shared.core.run(command: .importScan(pickedPaths: picked)) } catch { refused = Self.refusalOf(error) }
        finishScan(refused, path)
    }

    private func stage(_ path: String?) {
        if let path, !picked.contains(path) { picked.append(path) }
        scanning = true; notice = ""
    }

    private func finishScan(_ refused: (String, String)?, _ path: String?) {
        scanning = false; scanned = true
        sources = ImportState.sources(CoreHost.shared.core)
        if let refused {
            picked.removeAll { $0 == path }
            notice = ImportWords.refusal(code: refused.0, words: refused.1)
        }
        if let path, let s = sources.first(where: { $0.location == path }) { selected = s.id }
        else if selectedSource == nil { selected = (sources.first { $0.readable } ?? sources.first)?.id }
    }

    nonisolated static func refusalOf(_ e: Error) -> (String, String) {
        if case let CoreError.Invalid(field, words) = e { return (field, words) }
        if case let CoreError.Action(failure) = e { return ("", failure.reasonWords) }
        return ("", "\(e)")
    }
}
