import Foundation

/// The run, and the ways out of the panel (see ImportModel).
extension ImportModel {
    // MARK: run
    func start() {
        guard let s = selectedSource, canStart else { return }
        notice = ""; result = nil
        let into: ImportInto = intoAccount.map { .account(accountId: $0) } ?? .imported
        do {
            let out = try CoreHost.shared.core.run(command: .importStart(sourceId: s.id, into: into))
            guard let id = out.createdId else { return }
            run = ImportState.progress(CoreHost.shared.core, run: id)
            watch(id)
        } catch { let r = Self.refusalOf(error); notice = ImportWords.refusal(code: r.0, words: r.1) }
    }

    func cancel() {
        guard let r = run, !r.isOver else { return }
        do { _ = try CoreHost.shared.core.run(command: .importCancel(runId: r.runId)) }
        catch { let e = Self.refusalOf(error); notice = ImportWords.refusal(code: e.0, words: e.1) }
        syncRun()
    }

    /// Reads the run again. True once it is over (the sources are read again too: their imported counts moved).
    @discardableResult
    func syncRun() -> Bool {
        guard let id = run?.runId, let now = ImportState.progress(CoreHost.shared.core, run: id) else { return false }
        run = now
        guard now.isOver else { return false }
        result = ImportState.result(CoreHost.shared.core, run: id)
        sources = ImportState.sources(CoreHost.shared.core)
        return true
    }

    private func watch(_ id: String) {
        poll?.cancel()
        poll = Task { [weak self] in
            while !Task.isCancelled {
                guard let self, !self.syncRun() else { return }
                try? await Task.sleep(for: .milliseconds(250))
            }
        }
    }

    // MARK: leaving
    func select(_ id: String) { if !running { selected = id; notice = "" } }
    func selectKind(_ kind: String) { if let s = sources.first(where: { $0.kind == kind }) { select(s.id) } }

    /// Skip, and Go to the inbox after a run: the first start goes on to the inbox; from Settings the panel only closes.
    @discardableResult
    func leave(_ m: AppModel) -> Bool {
        poll?.cancel()
        reconnect()   // the add-account card connects through the core, so ask it now
        let toInbox = !fromSettings && (connected || WelcomeModel.shared.canOpenInbox)
        open = false; skipped = true; fromSettings = false
        UserDefaults.standard.set(true, forKey: Self.offerDoneKey)
        if toInbox { m.surface = "mac-main" }
        return true
    }

    /// Settings > Accounts "Import mail…": a clean panel, then the Welcome window opens on it.
    func openFromSettings() {
        poll?.cancel()
        run = nil; result = nil; notice = ""; skipped = false; fromSettings = true; open = true
        reconnect(); scan()
    }

    /// A clean slate (the Welcome reset and the rig's `import.reset` door): nothing found, nothing chosen, no run, the panel closed.
    func reset() {
        poll?.cancel()
        // a run still going in the core would refuse the next Start ("already running"): a clean slate ends it first
        if let r = run, !r.isOver { _ = try? CoreHost.shared.core.run(command: .importCancel(runId: r.runId)) }
        sources = []; selected = nil; intoAccount = nil; run = nil; result = nil; notice = ""
        scanning = false; scanned = false; skipped = false; open = false; fromSettings = false; picked = []
        UserDefaults.standard.removeObject(forKey: Self.offerDoneKey)
    }
}
