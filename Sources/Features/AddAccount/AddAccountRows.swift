import Foundation

/// The rows the socket reads (`rows mac-add-account.<region>`). No row ever holds the password: its own row shows dots.
@MainActor enum AddAccountRows {
    static let A = AddAccountWords.surface
    static var m: AddAccountModel { AddAccountModel.shared }

    static func state() -> [ControlRow] {
        [["ax": "\(A).state", "stage": m.stage.rawValue, "mode": m.mode.rawValue, "held": m.held != nil, "added": m.added.count,
          "sheet": AppModel.shared.surface == A, "style": m.style.rawValue, "manual_open": m.manualOpen, "confirming_stop": m.confirmingStop,
          "revealed": m.revealed, "opened_links": m.openedLinks, "copied_links": m.copiedLinks]]
    }
    static func fields() -> [ControlRow] { ["address", "password"].map { ["ax": "\(A).\($0)", "id": $0] } }
    static func address() -> [ControlRow] { [["ax": "\(A).address", "text": m.address, "visible": true, "focused": m.focus == .address, "locked": m.isWorking]] }
    static func password() -> [ControlRow] {
        [["ax": "\(A).password", "text": m.maskedPassword, "length": m.password.count, "visible": true, "focused": m.focus == .password, "revealed": m.revealed, "locked": m.isWorking]]
    }

    static func steps() -> [ControlRow] {
        m.steps.map { ["ax": "\(A).detection.step.\($0.id)", "id": $0.id, "outcome": $0.found ? "found" : "miss", "text": $0.text] }
    }
    /// The one grey line of words under the fields, the way the sheet reads it: its words, "n of N", the hairline's segments.
    static func step() -> [ControlRow] {
        let W = AddAccountWords.self
        switch m.stage {
        case .detecting:
            let order = W.stepOrder
            let n = min(m.steps.count + 1, order.count)
            return [["ax": "\(A).step", "text": W.stepWords(order[n - 1]), "n": n, "of": order.count, "count": W.stepOf(n, order.count), "segments": order.count, "done": n - 1]]
        case .validating: return [["ax": "\(A).step", "text": W.checkingServer, "n": 1, "of": 2, "count": W.stepOf(1, 2), "segments": 2, "done": 0]]
        case .added:
            let s = addedStep(m.added.last)
            return [["ax": "\(A).step", "text": s.words, "state": s.state, "n": 0, "of": 0, "count": s.count, "details": s.details, "details_open": m.stepDetailsOpen, "segments": 0, "done": 0]]
        case .entry: return [["ax": "\(A).step", "text": m.notice.isEmpty ? W.keychain : m.notice, "n": 0, "of": 0, "count": "", "segments": 0, "done": 0]]
        case .summary:
            return [["ax": "\(A).step", "text": m.hasProblem ? "" : (m.showsFoundLine ? "found" : m.settingsWords), "n": 0, "of": 0, "count": "", "segments": 0, "done": 0]]
        }
    }
    /// The found line: the two servers, which step found them, the mark.
    static func found() -> [ControlRow] {
        guard m.showsFoundLine else { return [] }
        let d = m.draft
        return [["ax": "\(A).found", "imap": "\(d.imapHost):\(d.imapPort)", "smtp": "\(d.smtpHost):\(d.smtpPort)", "source": AddAccountWords.foundBy(d.source),
                 "mark": (m.headKind ?? .unknown).rawValue]]
    }
    /// Where the provider's mark shows now: the Email field, the head chip, the found line, each added account.
    static func marks() -> [ControlRow] {
        var out: [ControlRow] = []
        if let k = m.fieldKind, m.stage != .added { out.append(["ax": "\(A).marks.field", "placement": "field", "kind": k.rawValue]) }
        out.append(["ax": "\(A).marks.head", "placement": "head", "kind": (m.stage == .added ? m.added.last?.kind : m.headKind)?.rawValue ?? "mail"])
        if m.showsFoundLine { out.append(["ax": "\(A).marks.found", "placement": "found", "kind": (m.headKind ?? .unknown).rawValue]) }
        for (i, a) in m.added.enumerated() { out.append(["ax": "\(A).marks.added.\(i)", "placement": "added", "kind": a.kind.rawValue, "account": a.id]) }
        return out
    }
    /// Every button on screen, with the key it shows: the rows are the footer's own list.
    static func controls() -> [ControlRow] {
        AddAccountControls.visible(m).map { c in
            ["ax": "\(A).\(c.id)", "id": c.id, "title": c.title, "key": c.key ?? "", "enabled": c.enabled, "busy": c.busy,
             "look": "\(c.look)", "tooltip": c.key.map { AddAccountWords.tooltip(c.title, $0) } ?? c.title]
        }
    }
    static func mode() -> [ControlRow] { m.showsSummary ? [["ax": "\(A).summary.mode", "mode": m.mode.rawValue, "source": AddAccountWords.source(m.draft.source)]] : [] }
    static func field(_ f: String) -> [ControlRow] {
        guard m.showsSummary else { return [] }
        let v = m.text(AddAccountModel.Field(f))
        return [["ax": "\(A).summary.\(f)", "value": (f == "imapPort" || f == "smtpPort") ? (Int(v) ?? v) : v, "editable": m.mode == .manual, "shown": m.manualOpen]]
    }
    static func manual() -> [ControlRow] {
        guard m.showsSummary || m.stage == .entry else { return [] }
        let c = AddAccountControls.footer(m).left.first { $0.id == "manual" }
        return [["ax": "\(A).manual", "text": c?.title ?? AddAccountWords.manual, "visible": c != nil && m.mode == .detected && m.stage == .summary, "open": m.manualOpen]]
    }

    static func validation() -> [ControlRow] {
        m.lines.enumerated().map { ["ax": "\(A).validation.line.\($0.offset + 1)", "text": $0.element.text, "state": $0.element.state.rawValue, "details": $0.element.details] }
    }
    static func accounts() -> [ControlRow] {
        m.added.map { ["ax": "\(A).accounts.\($0.id)", "id": $0.id, "address": $0.address, "name": $0.name, "colour": $0.colour, "progress": progress($0), "mark": $0.kind.rawValue] }
    }
    static func tenant() -> [ControlRow] { m.isTenant && m.showsSummary ? [["ax": "\(A).tenant", "text": AddAccountWords.tenant]] : [] }
    /// The one line about the provider under the found servers: an app password (with its link), or why Microsoft takes no password.
    static func providerNote() -> [ControlRow] {
        guard m.stage == .summary || m.stage == .validating else { return [] }
        if m.oauthKind == .microsoft && !m.oauthSaved(.microsoft) {
            return [["ax": "\(A).provider-note", "kind": "microsoft", "note": "microsoft-no-password", "text": AddAccountWords.microsoftNoPassword, "link": "", "guide": AddAccountWords.guideFile]]
        }
        if let p = m.provider, !p.steps.isEmpty {
            return [["ax": "\(A).provider-note", "kind": p.name.lowercased(), "note": "app-password", "text": AddAccountWords.appPasswordNote(p.name), "link": p.link, "link_words": p.linkWords]]
        }
        return []
    }
    /// Bring-your-own sign-in: shown for a Microsoft or Google provider.
    static func oauth() -> [ControlRow] {
        guard let k = m.oauthKind, m.stage == .summary else { return [] }
        return [["ax": "\(A).oauth", "kind": k.rawValue, "client_saved": m.oauthSaved(k), "phase": m.oauthPhase.name, "words": m.oauthPhase.words, "url": { if case .waiting(let u) = m.oauthState { return u }; return "" }(),
                 "caption": AddAccountWords.appIdCap, "guide_link": AddAccountWords.guideLinkShort, "guide": AddAccountWords.guideFile,
                 "default_client": m.usesDefaultClient, "field_shown": !m.usesDefaultClient && !m.oauthSaved(k), "own_id_link": m.usesDefaultClient ? AddAccountWords.useOwnId : "", "text": m.oauthSaved(k) ? "" : m.text(.appId).isEmpty ? "" : "•••", "length": m.text(.appId).count]]
    }
    static func appPasswordSteps() -> [ControlRow] {
        guard m.showsSummary, let p = m.provider, !p.steps.isEmpty else { return [] }
        return [["ax": "\(A).app-password.steps.1", "step": 1, "text": AddAccountWords.appPasswordNote(p.name), "link": p.link, "link_words": p.linkWords]]
    }
    static func error() -> [ControlRow] {
        // SPEC A87: `text` is the plain sentence for a known cause; `cause` its id; `details` the server's raw words, served only while open
        m.hasProblem ? [["ax": "\(A).error", "text": m.errorText, "link": m.errorLink, "host": m.errorHost, "header": m.errorIsServer ? m.errorHeader : m.errorText,
                         "cause": m.hasDetails ? m.errorEngineCode ?? m.errorCause.map(AddAccountWords.causeId) ?? "" : "", "details_available": m.hasDetails,
                         "details_open": m.hasDetails && m.detailsOpen, "details": m.hasDetails && m.detailsOpen ? m.errorDetails : "",
                         "details_label": m.detailsOpen ? AddAccountWords.detailsHide : AddAccountWords.detailsShow,
                         "paths": m.consentPaths, "consent_link": m.consentLink, "consent_link_text": m.consentLinkLine, "consent_copy_label": m.consentLink.isEmpty ? "" : AddAccountWords.copy]] : []
    }
    static func notice() -> [ControlRow] { m.notice.isEmpty ? [] : [["ax": "\(A).notice", "text": m.notice]] }

    /// What an added account is doing: connecting for the first moments, then reading the inbox, then ready; a sync error
    /// is shown in the server's words.
    static func progress(_ a: AddAccountModel.Added) -> String {
        let state = AppModel.shared.core.syncStates().first { $0.accountId == a.id }
        if let e = state?.lastError {
            if EngineRefusal.owns(e.code) { return EngineRefusal.sentence(e.code ?? "", domain: a.address.split(separator: "@").last.map { String($0).lowercased() } ?? "") }
            // a sync failure comes by its code: the catalogue's sentence for it, the engine's words behind Details; an older row keeps its words
            return SyncStatusWords.failure(code: e.code, words: e.reasonWords)?.sentence ?? e.reasonWords
        }
        if state?.historyFetched == true || (state?.messagesIndexed ?? 0) > 0 { return AddAccountWords.ready }
        return CoreHost.shared.clock.nowMs() - a.atMs < AddAccountMetrics.connectingMs ? AddAccountWords.connecting : AddAccountWords.fetching
    }

    /// The step under the added account, read from its sync state at draw time (owner, 23:15: the sheet stayed on "Connecting…"
    /// while the sync had failed). `state`: connecting · fetching · paused · fetched · failed; `count` the caption beside running
    /// words; `details` the engine's own English words behind a failure (the Details toggle), empty for any other state.
    /// Each state's words are the catalogue sentence for the code the core published (`SyncStatusWords`), never the engine's text.
    struct AddedStep: Equatable { let state: String; let words: String; let count: String; var details: String = "" }

    static func addedStep(_ a: AddAccountModel.Added?) -> AddedStep {
        let W = AddAccountWords.self
        guard let a else { return AddedStep(state: "connecting", words: W.connectingWords, count: W.thenFetching) }
        let s = AppModel.shared.core.syncStates().first { $0.accountId == a.id }
        let n = Int(s?.messagesIndexed ?? 0)
        let count = SyncStatusWords.messageCount(n)
        if let e = s?.lastError {
            let coded = SyncStatusWords.failure(code: e.code, words: e.reasonWords)
            return AddedStep(state: "failed", words: progress(a), count: "", details: coded == nil ? "" : e.reasonWords)
        }
        if s?.historyFetched == true { return AddedStep(state: "fetched", words: count, count: "") }
        let status = SyncStatusWords.status(s?.providerStatusWords)
        if status?.code == SyncStatusWords.paused { return AddedStep(state: "paused", words: status?.sentence ?? W.fetchingWords, count: n > 0 ? count : "") }
        if status?.code == SyncStatusWords.fetching { return AddedStep(state: "fetching", words: status?.sentence ?? W.fetchingWords, count: n > 0 ? count : "") }
        if status?.code == SyncStatusWords.connecting { return AddedStep(state: "connecting", words: W.connectingWords, count: W.thenFetching) }
        if n > 0 || progress(a) == W.fetching { return AddedStep(state: "fetching", words: W.fetchingWords, count: n > 0 ? count : "") }
        return AddedStep(state: "connecting", words: W.connectingWords, count: W.thenFetching)
    }
}
