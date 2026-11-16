import Foundation

/// Contracts delta 18 under --fixture-core, part 1: the mail import. No folder is read and nothing is stored: the fixture core serves the
/// contract's own rules (`checkImportStart`, `checkImportCancel`, `importProgressFraction`, `importResultAddsUp`) over a scripted run, so
/// the first-start step and Settings > Accounts can be driven and read exactly as the real core will serve them.
/// What a scan finds is set with `fixture import-sources <spec>...` (ControlSocket), how a run goes with `fixture import-progress <run> <step>`.
/// Source spec: `<kind>[:<location>]=<count|fda|missing|unreadable|?>`, kind apple-mail | olm | mbox (`apple-mail=18423`,
/// `apple-mail=fda`, `olm:~/Documents/Outlook.olm=2310`, `mbox:~/Mail/in.mbox=?`). Step: `<done>` (messages looked at), `finish`, `refuse`
/// (the source was refused mid-run: Full Disk Access taken back), `fail` (the local database refused a write).
final class FixtureImportBook: @unchecked Sendable {
    struct Run { var progress: ImportRunProgress; var into: ImportInto; var skippedNames: [ImportSkipped]; var result: ImportResult? }
    /// The default scan: Apple Mail, readable. (Fixture figure; the real core counts the files.)
    static let defaultSources: [ImportSource] = [
        ImportSource(id: importSourceId(kind: .appleMail, location: "~/Library/Mail"), kind: .appleMail, name: "Apple Mail", location: "~/Library/Mail",
                     messageCount: 1_240, importedCount: 0, lastRunMs: nil, access: .readable)]
    /// The "found on this Mac" list a scan starts from; `fixture import-sources` replaces it.
    var found: [ImportSource] = FixtureImportBook.defaultSources
    /// The sources of the last scan, in the order state `import/sources` serves them.
    var scanned: [ImportSource] = []
    var runs: [String: Run] = [:]
    var runCounter = 0

    /// A source spec of the verb, in the contract's words. Throws a plain sentence for a spec it cannot read.
    static func parse(_ spec: String) throws -> ImportSource {
        let halves = spec.split(separator: "=", maxSplits: 1).map(String.init)
        guard halves.count == 2 else { throw CoreError.Invalid(field: "spec", reasonWords: "write each source as kind=count, like apple-mail=1240") }
        let head = halves[0].split(separator: ":", maxSplits: 1).map(String.init)
        let kind: ImportSourceKind
        switch head[0] { case "apple-mail": kind = .appleMail; case "olm": kind = .olm; case "mbox": kind = .mbox
        default: throw CoreError.Invalid(field: "spec", reasonWords: "the kind is apple-mail, olm or mbox") }
        let location = head.count == 2 ? head[1] : (kind == .appleMail ? "~/Library/Mail" : "~/Documents/archive")
        var count: UInt64? = nil
        var access = ImportAccess.readable
        switch halves[1] {
        case "fda": access = .needsFullDiskAccess
        case "missing": access = .notFound
        case "unreadable": access = .unreadable
        case "?": count = nil
        default:
            guard let n = UInt64(halves[1]) else { throw CoreError.Invalid(field: "spec", reasonWords: "the count is a number, fda, missing, unreadable or ?") }
            count = n
        }
        let name = kind == .appleMail ? "Apple Mail" : (location as NSString).lastPathComponent
        return ImportSource(id: importSourceId(kind: kind, location: location), kind: kind, name: name, location: location,
                            messageCount: access == .readable ? count : nil, importedCount: 0, lastRunMs: nil, access: access)
    }
}

extension FixtureCore {
    // MARK: commands (called inside the lock by `run`)

    func importScanLocked(_ picked: [String]) throws -> CommandOutcome {
        var out: [ImportSource] = []
        for s in importBook.found {
            // what earlier runs stored stays on the row: a re-scan finds the same source with its history
            let prior = importBook.scanned.first { $0.id == s.id }
            out.append(ImportSource(id: s.id, kind: s.kind, name: s.name, location: s.location, messageCount: s.messageCount,
                                    importedCount: prior?.importedCount ?? 0, lastRunMs: prior?.lastRunMs, access: s.access))
        }
        for path in picked {
            let kind = try importKindOfPick(path: path)   // refuses anything but .olm / .mbox / .mbx in words
            let id = importSourceId(kind: kind, location: path)
            if out.contains(where: { $0.id == id }) { continue }
            let prior = importBook.scanned.first { $0.id == id }
            out.append(ImportSource(id: id, kind: kind, name: (path as NSString).lastPathComponent, location: path, messageCount: kind == .olm ? 2_310 : nil,
                                    importedCount: prior?.importedCount ?? 0, lastRunMs: prior?.lastRunMs, access: .readable))
        }
        importBook.scanned = out
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    func importStartLocked(_ sourceId: String, _ into: ImportInto) throws -> CommandOutcome {
        let source = importBook.scanned.first { $0.id == sourceId }
        let running = importBook.runs.values.first { $0.progress.state == .running }?.progress.runId
        try checkImportStart(source: source, into: into, accountIds: accountIdsLocked(), runningRunId: running)
        guard let source else { throw CoreError.NotFound(kind: "import source", id: sourceId) }
        importBook.runCounter += 1
        let runId = "run-\(importBook.runCounter)"
        let first = source.kind == .appleMail ? "INBOX" : source.name
        importBook.runs[runId] = FixtureImportBook.Run(
            progress: ImportRunProgress(runId: runId, sourceId: source.id, state: .running, done: 0, total: source.messageCount, current: first, stored: 0, skipped: 0, refusal: nil),
            into: into, skippedNames: [], result: nil)
        return CommandOutcome(undoEntryId: nil, createdId: runId)
    }

    func importCancelLocked(_ runId: String) throws -> CommandOutcome {
        try checkImportCancel(progress: importBook.runs[runId]?.progress)
        endRunLocked(runId, .cancelled, refusal: nil)
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    // MARK: the scripted run (`fixture import-progress`)

    /// One step of a scripted run. Returns the run's state word after it. Throws NotFound for an unknown run, and the contract's refusal for a
    /// run that is already over (`checkImportCancel`'s words: it is not running).
    @discardableResult
    public func fixtureImportProgress(runId: String, step: String) throws -> String {
        let word: String = try lock.withLock {
            guard let run = importBook.runs[runId] else { throw CoreError.NotFound(kind: "import run", id: runId) }
            try checkImportCancel(progress: run.progress)
            switch step {
            case "finish":
                applyCountsLocked(runId, done: run.progress.total ?? max(run.progress.done, 1))
                endRunLocked(runId, .finished, refusal: nil)
            case "refuse": endRunLocked(runId, .refused, refusal: importAccessRefusal(access: .needsFullDiskAccess))
            case "fail": endRunLocked(runId, .failed, refusal: importStoreFailedRefusal())
            default:
                guard let n = UInt64(step) else { throw CoreError.Invalid(field: "step", reasonWords: "a step is a number of messages, finish, refuse or fail") }
                applyCountsLocked(runId, done: n)
            }
            return importBook.runs[runId].map { importRunStateKey(state: $0.progress.state) } ?? ""
        }
        emit()
        return word
    }

    /// The scripted run skips every 25th message (too large), stores the rest: deterministic, and `stored + skipped <= done` always.
    private func applyCountsLocked(_ runId: String, done: UInt64) {
        guard var run = importBook.runs[runId] else { return }
        let skipped = done / 25
        run.progress.done = done; run.progress.skipped = skipped; run.progress.stored = done - skipped
        run.skippedNames = (0..<skipped).map { ImportSkipped(origin: "\(run.progress.current)/\($0 + 1).emlx", words: "The message is larger than Whoosh imports, so it was left out.") }
        importBook.runs[runId] = run
    }

    private func endRunLocked(_ runId: String, _ state: ImportRunState, refusal: ImportRefusal?) {
        guard var run = importBook.runs[runId] else { return }
        run.progress.state = state; run.progress.refusal = refusal
        let per: [ImportPerAccount]
        if run.progress.stored == 0 { per = [] }
        else if case .account(let id) = run.into { per = [ImportPerAccount(accountId: id, stored: run.progress.stored)] }
        else { per = [ImportPerAccount(accountId: "imported", stored: run.progress.stored)] }
        run.result = ImportResult(runId: runId, sourceId: run.progress.sourceId, state: state, seen: run.progress.done, stored: run.progress.stored, alreadyThere: 0,
                                  skipped: run.skippedNames, perAccount: per, refusal: refusal)
        importBook.runs[runId] = run
        // what the run stored joins the source's history: the next scan and Settings > Accounts read it
        if run.progress.stored > 0, let i = importBook.scanned.firstIndex(where: { $0.id == run.progress.sourceId }) {
            importBook.scanned[i].importedCount += run.progress.stored
            importBook.scanned[i].lastRunMs = clock.nowMs()
        }
    }

    /// `fixture import-sources <spec>...`: what the next scan finds on this Mac (empty = nothing found). Replaces the default (Apple Mail).
    public func fixtureImportSources(_ specs: [String]) throws {
        let parsed = try specs.map(FixtureImportBook.parse)
        lock.withLock { importBook.found = parsed }
        emit()
    }

    // MARK: state reads (`import/sources`, `import/<run>/progress`, `import/<run>/result`)

    func importStateLocked(_ p: [String]) -> D9Hit? {
        guard p.first == "import" else { return nil }
        switch p.count {
        case 2 where p[1] == "sources":
            return D9Hit(value: d18Json(importBook.scanned.map(sourceObject)))
        case 3 where p[2] == "progress":
            return D9Hit(value: importBook.runs[p[1]].flatMap { d18Json(progressObject($0.progress)) })
        case 3 where p[2] == "result":
            return D9Hit(value: importBook.runs[p[1]]?.result.flatMap { d18Json(resultObject($0)) })
        default: return nil
        }
    }

    private func refusalObject(_ r: ImportRefusal?) -> Any { r.map { ["code": $0.code, "words": $0.words] as [String: Any] } ?? NSNull() }

    private func sourceObject(_ s: ImportSource) -> [String: Any] {
        ["id": s.id, "kind": importSourceKindKey(kind: s.kind), "name": s.name, "location": s.location,
         "message_count": s.messageCount.map { $0 as Any } ?? NSNull(), "imported_count": s.importedCount,
         "last_run_ms": s.lastRunMs.map { $0 as Any } ?? NSNull(), "access": importAccessKey(access: s.access),
         "words": importAccessWords(access: s.access)]
    }

    private func progressObject(_ p: ImportRunProgress) -> [String: Any] {
        ["run_id": p.runId, "source_id": p.sourceId, "state": importRunStateKey(state: p.state), "done": p.done,
         "total": p.total.map { $0 as Any } ?? NSNull(), "fraction": importProgressFraction(done: p.done, total: p.total).map { $0 as Any } ?? NSNull(),
         "current": p.current, "stored": p.stored, "skipped": p.skipped, "refusal": refusalObject(p.refusal)]
    }

    private func resultObject(_ r: ImportResult) -> [String: Any] {
        ["run_id": r.runId, "source_id": r.sourceId, "state": importRunStateKey(state: r.state), "seen": r.seen, "stored": r.stored,
         "already_there": r.alreadyThere, "skipped": r.skipped.map { ["origin": $0.origin, "words": $0.words] },
         "per_account": r.perAccount.map { ["account_id": $0.accountId, "stored": $0.stored] as [String: Any] }, "refusal": refusalObject(r.refusal)]
    }
}

/// JSON for a state value: sorted keys, fragments allowed. nil when the value cannot be written.
func d18Json(_ v: Any) -> String? {
    guard JSONSerialization.isValidJSONObject(v) || v is String || v is Bool || v is Int else { return nil }
    guard let d = try? JSONSerialization.data(withJSONObject: v, options: [.fragmentsAllowed, .sortedKeys]) else { return nil }
    return String(data: d, encoding: .utf8)
}
