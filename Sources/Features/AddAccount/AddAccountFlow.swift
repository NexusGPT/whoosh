import Foundation

/// The two things the core does for this screen, each off the main thread so a slow server never freezes the window:
/// detectProvider (one progress event per step while it runs) and validateAccount then addAccountConfigured.
/// Under the fixture core both answer at once and run inline, so a check reads a finished state after one click.
extension AddAccountModel {
    @discardableResult
    public func continueTapped() -> Bool {
        guard stage != .detecting, stage != .validating else { return false }
        commitFocused()
        guard addressLooksValid else { notice = AddAccountWords.badAddress; return false }
        let email = address.trimmingCharacters(in: .whitespaces).lowercased()
        generation += 1; let mine = generation
        notice = ""; errorText = ""; errorLink = ""; errorHost = ""; errorHeader = ""; errorCause = nil; errorEngineCode = nil; errorDetails = ""; detailsOpen = false; lines = []; steps = []; result = nil; mode = .detected
        manualOpen = false; edited = false
        stage = .detecting
        let core = AppModel.shared.core
        let sink = DetectionSink { [weak self] number, progress in self?.reached(number: number, progress: progress, generation: mine) }
        let token = core.subscribe(stream: .syncStatus, sink: sink)
        run(hold: holdDetection, { core.detectProvider(email: email) }) { [weak self] r in
            core.unsubscribe(subscription: token)
            self?.detected(r, generation: mine)
        }
        return true
    }

    func reached(number: UInt32, progress: DetectionProgress, generation g: Int) {
        guard g == generation, stage == .detecting, !steps.contains(where: { $0.number == number }) else { return }
        steps.append(Step(id: Self.stepId(progress.step), found: progress.found, text: AddAccountWords.stepWords(Self.stepId(progress.step)), number: number))
    }

    func detected(_ r: DetectionResult, generation g: Int) {
        guard g == generation else { return }
        result = r
        steps = r.progress.enumerated().map {
            let id = Self.stepId($0.element.step)
            return Step(id: id, found: $0.element.found, text: AddAccountWords.stepWords(id), number: UInt32($0.offset + 1))
        }
        switch r.outcome {
        case .found(let config): draft = AddAccountDraft(config); mode = .detected; manualOpen = false
        case .imapOnly(let host, let port, let security, let user, let source):
            draft = AddAccountDraft(imapOnly: host, port: port, security: security, username: user, source: source); mode = .manual; manualOpen = true; draft.fillUsual()
        case .refused(let words): draft = AddAccountDraft(); draft.fillUsual(); mode = .manual; manualOpen = true; notice = words
        }
        stage = .summary
        refreshOAuth()
    }

    // MARK: add
    @discardableResult
    public func addTapped() -> Bool {
        guard canAdd else { return false }
        commitFocused()
        let addr = address.trimmingCharacters(in: .whitespaces).lowercased()
        if password.isEmpty { return fail(AddAccountWords.needPassword) }
        let config: AccountConfig
        switch draft.config(address: addr) {
        case .success(let c): config = c
        case .failure(let p): mode = .manual; manualOpen = true; return fail(p == .incoming ? AddAccountWords.incomingMissing : AddAccountWords.outgoingMissing)
        }
        generation += 1; let mine = generation
        errorText = ""; errorLink = ""; errorHost = ""; errorHeader = ""; errorCause = nil; errorEngineCode = nil; errorDetails = ""; detailsOpen = false; notice = ""; confirmingStop = false; stage = .validating
        lines = [Line(text: AddAccountWords.checkingServer, state: .running, details: []), Line(text: AddAccountWords.checkingSending, state: .pending, details: [])]
        let core = AppModel.shared.core, secret = SecretText(value: password), stop = stopFlag
        run(hold: holdValidation, { Self.validateThenAdd(core, config, addr, secret, stop) }) { [weak self] outcome in self?.finished(outcome, config: config, address: addr, generation: mine) }
        return true
    }

    /// Validates, then adds. A check the person stopped saves nothing: the flag is read after the validation and before the add.
    nonisolated static func validateThenAdd(_ core: WhooshCore, _ config: AccountConfig, _ address: String, _ secret: SecretText, _ stop: StopFlag) -> AddOutcome {
        let report = core.validateAccount(config: config, password: secret)
        guard report.ok, !stop.isStopped else { return AddOutcome(report: report, createdId: nil, failure: nil) }
        do {
            let out = try core.run(command: .addAccountConfigured(address: address, displayName: nil, config: config, password: secret))
            return AddOutcome(report: report, createdId: out.createdId, failure: out.createdId == nil ? "The core did not say which account it made." : nil)
        } catch CoreError.Action(let f) { return AddOutcome(report: report, createdId: nil, failure: f.reasonWords, failureCode: EngineRefusal.owns(f.code) ? f.code : nil) }
        catch CoreError.Invalid(_, let words) { return AddOutcome(report: report, createdId: nil, failure: words) }
        catch { return AddOutcome(report: report, createdId: nil, failure: "\(error)") }
    }

    func finished(_ o: AddOutcome, config: AccountConfig, address: String, generation g: Int) {
        guard g == generation else { return }
        lines = Self.lines(from: o.report)
        if let id = o.createdId { recordAdded(id: id, address: address, imapHost: config.imapHost); return }
        mode = .manual; stage = .summary; draft.source = .manual; confirmingStop = false
        let words = o.failure ?? o.report.summaryWords
        // round 5: the engine's refusal reads as the catalogue sentence for its code; its own words wait behind Details
        if let code = o.failureCode { showEngine(code: code, words: words); return }
        // SPEC A87: a known cause reads as its one plain sentence; the server's raw text waits behind Details (closed)
        let cause = o.failure == nil ? o.report.cause : nil
        errorCause = cause; detailsOpen = false
        errorDetails = cause == nil ? "" : words
        errorText = cause.map(AddAccountWords.causeSentence) ?? Self.withSignInHint(words, oauthKind)
        errorLink = Self.firstLink(in: words)
        refusalTick += 1
        errorHost = o.report.refusedBy ?? ""
        errorHeader = errorHost.isEmpty ? AddAccountWords.signInFailed : Self.header(o.report, host: errorHost)
    }

    func fail(_ words: String) -> Bool { errorText = words; errorLink = ""; errorHost = ""; errorHeader = ""; errorCause = nil; errorEngineCode = nil; errorDetails = ""; detailsOpen = false; return false }

    /// SPEC A87: Details opens and closes the server's own words under the plain sentence.
    @discardableResult
    func toggleDetails() -> Bool {
        guard hasProblem, !errorDetails.isEmpty else { return false }
        detailsOpen.toggle(); return true
    }

    /// Who refused and how, from the step that failed: a sign-in is "refused the password", a silent server "did not answer".
    static func header(_ r: ValidationReport, host: String) -> String {
        switch r.steps.first(where: { !$0.ok })?.kind {
        case .imapLogin, .smtpAuth: return AddAccountWords.refusedBy(host)
        default: return AddAccountWords.didNotAnswer(host)
        }
    }

    // MARK: running
    /// Off the main thread for a real core; inline for the fixture core, whose answers are immediate. A held run keeps its
    /// answer until `release()`.
    func run<T: Sendable>(hold: Bool, _ work: @escaping @Sendable () -> T, then: @escaping @MainActor (T) -> Void) {
        if AppModel.shared.host.fixture != nil {
            // a held run does not start until it is released, so a check that stops it leaves nothing behind
            if hold { held = { then(work()) } } else { then(work()) }
            return
        }
        Task.detached { let r = work(); await MainActor.run { then(r) } }
    }
}

extension AddAccountModel {
    /// The account the core made: listed, the password dropped, the sheet on its added state (Welcome's card starts over).
    func recordAdded(id: String, address: String, imapHost: String) {
        AppModel.shared.refresh()
        let entry = AppModel.shared.core.accounts().first { $0.id == id }
        let kind = ProviderKind.of(address: address).flatMap { $0 != .unknown ? $0 : nil } ?? ProviderKind.of(host: imapHost)
        added.append(Added(id: id, address: address, name: entry.map { AccountNames.display($0) } ?? address, colour: entry?.colour ?? "", atMs: CoreHost.shared.clock.nowMs(), kind: kind))
        passwordStore = ""; if focus == .password { AppModel.shared.typed = "" }
        if style == .inline { startOver(); return }   // Welcome's card goes back to the empty fields; its Added list grows
        stage = .added
    }
}
