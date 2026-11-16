import SwiftUI

/// Registration for the settingsaimac lane: AI providers, agents, and the Decisions home (docs/DECISION-MODEL.md v2).
public enum SettingsAIMacFeature: FeatureModule {
    public static let id = "settingsaimac"
    public static let surfaces: [String] = ["mac-settings-ai"]
    /// How many entries of a list and how many tree nodes the entry doors reach (a guard in each door checks the live list and tree).
    private static let entryDoors = 12, nodeDoors = 48
    /// How many added models the doors of the list reach (`decisions.added.<k>`); a guard in each door reads the live list.
    private static let addedDoors = 8

    /// The added line k (from 1), as the list holds it now.
    @MainActor private static func added(_ k: Int) -> DecisionModelEntry? {
        let added = DecisionModels.shared.lines.filter { DecisionModels.shared.isAdded($0.id) }
        return k <= added.count ? added[k - 1] : nil
    }

    /// The Settings scene's "Decisions" tab (appshell round 3): this pane fills it; there is no window of its own.
    public static func view(for slot: SurfaceSlot) -> AnyView? {
        slot == .macSettingsAI ? AnyView(DecisionsPaneView()) : nil
    }

    /// `type <text>` while the AI pane is the current surface and a draft agent is open: the draft's name field takes it. A closed
    /// Settings window (the surface is another one) returns false, so the text goes to the surface that holds the keys.
    public static func typeText(_ text: String, _ m: AppModel) -> Bool {
        guard !text.isEmpty, hasFocus(m) else { return false }
        return AISettingsStore.shared.typeIntoDraft(text)
    }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        let s = AISettingsStore.shared
        var out: [String: (AppModel) throws -> [ControlRow]] = [
            "mac-settings-ai.section": { _ in [["section": s.sectionName]] },
            "mac-settings-ai.speech": { _ in [["state": s.speech.rawValue, "engine": s.speechKind, "title": SpeechGroup.title, "row_title": SpeechGroup.rowTitle]] },
            "mac-settings-ai.speech.line": { _ in [["key": s.dictationKey ?? "", "link": s.dictationKey == nil]] },
            "mac-settings-ai.speech.link": { _ in [["pane": SettingsWindowModel.shared.pane.rawValue]] },
            "mac-settings-ai.speech.engines": { _ in
                s.syncFromCore()
                return [["engine": "local", "chosen": s.speechKind == "local"]] + s.providers.filter(\.connected).map { ["engine": $0.id, "chosen": s.speechKind == $0.id] }
            },
            "mac-settings-ai.speech.retry": { _ in s.speech == .failed ? [["reason": s.speechWords ?? AIWords.speechFailed]] : [] },
            "mac-settings-ai.provider.chatgpt": { _ in [["state": ChatGPTState.words(), "models": [String](), "note": AIWords.chatgptNote, "logo": ProviderMark.chatgpt.asset]] },
            "mac-settings-ai.logos": { _ in ProviderMark.probeRows },
            "mac-settings-ai.agents.form": { _ in s.formMissing.map { [["missing": $0, "link": "mac-settings-ai.provider"]] } ?? [] },
            // A pure read: it never takes the shared typed buffer (the draft name arrives through `typeText` below).
            "mac-settings-ai.agents.new.row": { _ in AgentsRows.newRow() },
            "mac-settings-ai.agents.card": { _ in
                s.syncFromCore()
                return (s.draftAgent.map { [$0] } ?? []).plus(s.agents).map { ["name": $0.name, "allowed": $0.allowed.map(AIWords.words), "editing": $0.id == s.draftAgent?.id,
                                                                                    "provider": $0.providerId, "model": $0.model, "refusal": s.agentRefusals[$0.id] ?? ""] }
            },
            "mac-settings-ai.label-menu": { m in AILabelTree.nodes(m.core).map { ["path": $0] } },
            "mac-settings-ai.decisions.page.labels.proposal": { _ in
                s.seedLabels()
                return s.proposals["labels"].map { [["path": $0, "parent_in_tree": AILabelTree.nodes().contains(AILabelTree.parent($0))]] } ?? []
            },
            "mac-settings-ai.decisions.pages": { _ in s.pages.map { ["id": $0.id, "title": $0.title] } },
            // A pure read: it answers what the screen holds and never re-reads the store (a read that repaired the screen hid a stale line).
            "mac-settings-ai.decisions.local": { _ in [DecisionModelRow.row(DecisionModelCatalogue.laya, s)] },
            // every line of the list, in its order (SPEC A133, pick M1)
            "mac-settings-ai.decisions.models": { _ in s.engine == .local ? DecisionModels.shared.lines.map { DecisionModelRow.row($0, s) } : [] },
            // the card's lines in the order they are drawn: each model's id, then "other", the line that adds one (pick M1, model-door)
            "mac-settings-ai.decisions.lines": { _ in s.engine == .local ? DecisionModels.shared.lines.map { ["line": $0.id] } + [["line": "other"]] : [] },
            "mac-settings-ai.decisions.other": { _ in s.engine == .local ? [OtherModelLine.row(s)] : [] },
            "mac-settings-ai.decisions.local.next": { _ in
                s.engine == .local && { if case .ready = s.modelState(DecisionModels.shared.line(id: s.chosenModel)) { return true } else { return false } }() ? [["key": "R", "text": AIWords.layaNext.replacingOccurrences(of: "%@", with: "R")]] : []
            },
            "mac-settings-ai.decisions.jev.consent": { _ in s.syncDecision(); return [["given": s.jevConsent, "refusal": s.engineRefusal ?? ""]] },
            // The consent card's state in the words the checks read (`given` or `asked`); the same fact as `decisions.jev.consent`.
            "mac-settings-ai.decisions.consent": { _ in s.syncDecision(); return [["provider": "jev", "state": s.jevConsent ? "given" : "asked", "refusal": s.engineRefusal ?? ""]] },
            // Why each message of the last preview was decided as it was: the page, the entry that answered, the outcome, and the sentence naming them.
            "mac-settings-ai.decisions.result": { _ in
                // hid-ai: a message `fixture decide` ran (since the last preview) answers alone: its entry, outcome, why and whether a model was asked
                if let d = s.lastDecision, let i = s.pageIndex(d.page) {
                    return [["page": d.page, "message": d.message, "entry": d.decision.entry ?? 0, "outcome": d.decision.outcome,
                             "why": "\(s.pages[i].title) · \(d.decision.decidedBy)", "model_called": d.decision.modelCalled]]
                }
                guard let id = s.lastPreviewPage, let i = s.pageIndex(id) else { return [] }
                let title = s.pages[i].title
                return (s.previews[id] ?? []).map { ["page": id, "message": $0.id, "entry": $0.entry, "outcome": $0.outcome, "why": "\(title) · \($0.decidedBy)"] }
            },
            // The rule "Add a rule" opened last, as the editor holds it: its words, its outcome and the sender or domain it names.
            "mac-settings-ai.decisions.rule-editor": { _ in
                guard let d = s.ruleDraft, let i = s.pageIndex(d.page), let n = s.pages[i].entries.firstIndex(where: { $0.id == d.entry }) else { return [] }
                let e = s.pages[i].entries[n], key = e.match ?? ""
                let from = key.hasPrefix("from:") ? String(key.dropFirst(5)) : ""
                return [["page": d.page, "n": n + 1, "text": e.text, "outcome": e.outcome, "from": from,
                         "domain": from.split(separator: "@").last.map(String.init) ?? ""]]
            },
            "mac-settings-ai.decisions.jev.key": { _ in s.settleAll(); return [["connected": s.jevConnected, "state": s.jevState.map { aiProviderStateKey(s: $0) } ?? "", "shown": s.jevChip, "note": s.refusals["jev"] ?? ""]] },
            "mac-settings-ai.decisions.engine": { _ in
                s.syncDecision()
                return AISettingsStore.Engine.allCases.map { e in
                    // round 6: the segment the screen draws (`shownEngine`: Jev while its key or consent is asked for), not only the saved one
                    // (main88b and main92 sweep-settingsai engine-jev-asks-for-its-key read Laya chosen while the pane showed Jev and its refusal)
                    ["id": e.rawValue, "chosen": s.shownEngine == e,
                     "state": e == .jev ? (s.jevConnected ? AIWords.connected : "needs key") : "ready",
                     "refusal": s.shownEngine == e ? (s.engineRefusal ?? "") : "",
                     "labels_by": e == .local ? (AISettingsStore.labelsByApple ? "apple" : "laya") : (e == .jev ? "jev" : "none"),
                     "logo": e == .jev ? ProviderMark.jev.asset : (e == .local ? ProviderMark.laya.asset : "none")]
                }
            },
        ]
        for p in s.providers.map(\.id) {
            out["mac-settings-ai.provider.\(p)"] = { _ in
                s.settleAll()   // the socket reads the answer, not "checking" (a view never waits)
                let pr = s.providers.first { $0.id == p }
                return [["state": s.state(of: p), "models": pr?.models ?? [], "note": s.refusals[p] ?? "", "logo": ProviderMark.of(p)?.asset ?? "none"]]
            }
            out["mac-settings-ai.provider.\(p).check"] = { _ in [["state": s.isChecking(p) ? "checking" : "idle"]] }
            out["mac-settings-ai.provider.\(p).key"] = { _ in [["length": (s.keyDrafts[p] ?? "").count, "focused": s.keyFocus == p]] }
            out["mac-settings-ai.provider.\(p).models"] = { _ in s.settleAll(); return (s.providers.first { $0.id == p && $0.connected }?.models ?? []).map { ["model": $0] } }
        }
        // The model menu of card n (1 = the draft when one is open): its entries as rows.
        for n in 1...8 {
            out["mac-settings-ai.agents.model.\(n)"] = { _ in
                s.syncFromCore()
                guard n <= s.cards.count else { return [] }
                let a = s.cards[n - 1]
                return s.modelChoices().map { c in
                    ["provider": c.provider.id, "model": c.model ?? "", "enabled": c.enabled,
                     "current": c.model != nil && c.provider.id == a.providerId && c.model == a.model,
                     "note": c.enabled ? "" : AIWords.addKeyAbove]
                }
            }
        }
        for page in s.pages.map(\.id) {
            out["mac-settings-ai.decisions.page.\(page).entries"] = { _ in
                s.seedLabels()
                guard let i = s.pageIndex(page) else { return [] }
                return s.pages[i].entries.enumerated().map { n, e in
                    ["n": n + 1, "kind": e.kind.rawValue, "text": e.text, "outcome": e.outcome, "enabled": e.enabled, "trust": e.trust as Any]
                }
            }
            out["mac-settings-ai.decisions.page.\(page).preview"] = { _ in
                (s.previews[page] ?? []).map { ["message": $0.id, "entry": $0.entry, "outcome": $0.outcome, "decided_by": $0.decidedBy, "differs": $0.differs] }
            }
            // the teaching block (DECISION-MODEL.md v2 §3): the corrections in words, their number, what they share, and the fixes offered ("make-rule", "refit")
            out["mac-settings-ai.decisions.page.\(page).teaching"] = { _ in
                s.syncDecision()
                let t = s.teaching(page)
                return [["count": AIWords.corrections(t.corrections), "corrections": t.corrections, "shared": t.shared ?? "", "offers": t.offers, "note": s.refitNotes[page] ?? ""]]
            }
            // where the page's preview stands: idle, running, done or refused (a check waits on this, never on time)
            out["mac-settings-ai.decisions.page.\(page).preview.state"] = { _ in [["state": s.previewState(page).rawValue]] }
        }
        for entry in DecisionModels.shared.lines where entry.family != .laya && !DecisionModels.shared.isAdded(entry.id) {
            out[DecisionModelRow.ax(entry)] = { _ in [DecisionModelRow.row(entry, s)] }
        }
        // an added line k (model-door): its doors are registered before it exists, so they are counted; the live list decides
        for k in 1...Self.addedDoors {
            out["mac-settings-ai.decisions.added.\(k)"] = { _ in Self.added(k).map { [DecisionModelRow.row($0, s)] } ?? [] }
        }
        return out
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        let s = AISettingsStore.shared
        var out: [String: (AppModel) -> Bool] = [
            "mac-settings-ai.speech.local": { _ in s.chooseLocalSpeech(); return true },
            "mac-settings-ai.agents.new": { _ in s.section = .agents; s.questionFocus = nil; s.newAgent(); return true },
            "mac-settings-ai.agents.duplicate": { _ in
                s.syncFromCore()
                guard let a = s.agents.last else { return false }
                s.duplicate(a); return true
            },
            "mac-settings-ai.speech.shortcuts": { _ in s.openShortcuts(); return true },
            "mac-settings-ai.speech.engine.local": { _ in s.chooseSpeech(provider: nil); return true },
            "mac-settings-ai.agents.name": { _ in s.focusDraftName() },
            "mac-settings-ai.agents.form.link": { _ in guard s.formMissing != nil else { return false }; s.section = .providers; return true },
            "mac-settings-ai.agents.save": { _ in s.saveDraft(); return true },
            // INTERIM fixture controls: the root's `fixture paste` and `fixture speech-fail` verbs are not served at
            // this commit (FixtureVerbs.swift). The store also listens for AIFixture's notifications, the shape asked of the shell.
            "mac-settings-ai.fixture.paste.anthropic-key": { _ in s.paste(AIFixture.anthropicKey, into: "anthropic"); return true },
            "mac-settings-ai.fixture.paste.bad-key": { _ in s.paste("sk-bad", into: "anthropic"); return true },
            "mac-settings-ai.fixture.speech-fail": { _ in AIFixture.failSpeech(); return true },
            "mac-settings-ai.nav.providers": { _ in s.section = .providers; return true },
            "mac-settings-ai.nav.agents": { _ in s.section = .agents; return true },
            "mac-settings-ai.nav.decisions": { _ in s.section = .decisions; return true },
            "mac-settings-ai.decisions.back": { _ in s.section = .decisions; return true },
        ]
        for p in s.providers.map(\.id) {
            out["mac-settings-ai.provider.\(p).key"] = { _ in s.section = .providers; s.keyFocus = p; return true }
            out["mac-settings-ai.provider.\(p).connect"] = { _ in s.connect(p); return true }
        }
        for p in s.providers.map(\.id) {
            out["mac-settings-ai.speech.engine.\(p)"] = { _ in
                guard s.providers.first(where: { $0.id == p })?.connected == true else { return false }
                s.chooseSpeech(provider: p); return true
            }
        }
        // The model menu of card n (1 = the draft when one is open): one door per entry, k counting the entries as the rows list them.
        for n in 1...8 {
            for k in 1...24 {
                out["mac-settings-ai.agents.model.\(n).choose.\(k)"] = { _ in
                    s.syncFromCore()
                    let choices = s.modelChoices()
                    guard n <= s.cards.count, k <= choices.count, choices[k - 1].enabled else { return false }
                    s.setModel(s.cards[n - 1].id, to: choices[k - 1]); return true
                }
            }
        }
        for (w, p) in [("read", AgentPermission.readMail), ("search", .search), ("draft", .draft), ("label", .label)] {
            out["mac-settings-ai.agents.allow.\(w)"] = { _ in
                guard let id = s.draftAgent?.id ?? s.agents.last?.id else { return false }
                s.toggle(p, on: id); return true
            }
        }
        // a line's three doors (the list of on-device models): its Download or Retry, its Cancel, its circle
        for entry in DecisionModels.shared.lines where !DecisionModels.shared.isAdded(entry.id) {
            let ax = DecisionModelRow.ax(entry)
            out[ax + ".cancel"] = { _ in
                guard case .downloading = s.modelState(entry) else { return false }
                s.cancelModel(entry.id); return true
            }
            out[ax + ".download"] = { _ in guard case .ready = s.modelState(entry) else { s.downloadModel(entry.id); return true }; return false }
            out[ax + ".choose"] = { _ in guard s.engine == .local, s.chosenModel != entry.id else { return false }; s.chooseModel(entry.id); return true }
        }
        // the same doors of an added line k, read at the click (the line may come and go), and its Remove
        for k in 1...Self.addedDoors {
            let ax = "mac-settings-ai.decisions.added.\(k)"
            out[ax + ".cancel"] = { _ in
                guard let entry = Self.added(k), case .downloading = s.modelState(entry) else { return false }
                s.cancelModel(entry.id); return true
            }
            out[ax + ".download"] = { _ in
                guard let entry = Self.added(k) else { return false }
                guard case .ready = s.modelState(entry) else { s.downloadModel(entry.id); return true }; return false
            }
            out[ax + ".choose"] = { _ in
                guard let entry = Self.added(k), s.engine == .local, s.chosenModel != entry.id else { return false }
                s.chooseModel(entry.id); return true
            }
            out[ax + ".remove"] = { _ in
                guard let entry = Self.added(k), DecisionModelRow.removable(entry, s) else { return false }
                s.removeModel(entry.id); return true
            }
        }
        // the line "Other model": Add, the field (the socket's `type` then fills it), Check (Return) and leaving the field (Escape)
        out["mac-settings-ai.decisions.other.add"] = { _ in guard s.engine == .local, !s.otherOpen else { return false }; s.openOther(); return true }
        out["mac-settings-ai.decisions.other.field"] = { _ in guard s.otherOpen else { return false }; s.keyFocus = AISettingsStore.otherField; return true }
        out["mac-settings-ai.decisions.other.check"] = { _ in guard s.otherOpen, !s.otherChecking else { return false }; s.checkOther(); return true }
        out["mac-settings-ai.decisions.other.leave"] = { _ in guard s.otherOpen else { return false }; s.closeOther(); return true }
        out["mac-settings-ai.decisions.jev.connect"] = { _ in
            let k = s.keyDrafts["jev"] ?? ""; s.keyDrafts["jev"] = nil
            s.connectJev(k); return true
        }
        out["mac-settings-ai.decisions.jev.key.field"] = { _ in s.keyFocus = "jev"; return true }
        out["mac-settings-ai.decisions.jev.test"] = { _ in s.settleAll(); guard s.jevConnected else { return false }; s.testJev(); return true }
        out["mac-settings-ai.decisions.jev.consent.allow"] = { _ in s.setConsent(true); return true }
        out["mac-settings-ai.decisions.jev.consent.withdraw"] = { _ in s.setConsent(false); return true }
        for e in AISettingsStore.Engine.allCases {
            out["mac-settings-ai.decisions.engine.\(e.rawValue)"] = { _ in s.chooseEngine(e); return true }
        }
        for page in s.pages {
            let id = page.id
            out["mac-settings-ai.nav.page.\(id)"] = { _ in s.section = .page(id); return true }
            out["mac-settings-ai.decisions.open.\(id)"] = { _ in s.section = .page(id); return true }
            out["mac-settings-ai.decisions.page.\(id).preview"] = { _ in s.section = .page(id); s.runPreview(page: id); return true }
            out["mac-settings-ai.decisions.page.\(id).apply"] = { _ in s.apply(page: id); return true }
            out["mac-settings-ai.decisions.page.\(id).add-rule"] = { _ in s.addRule(page: id); return true }
            out["mac-settings-ai.decisions.page.\(id).add-model"] = { _ in s.addModel(page: id); return true }
            out["mac-settings-ai.decisions.page.\(id).teaching.make-rule"] = { _ in s.ruleFromCorrections(page: id) }
            out["mac-settings-ai.decisions.page.\(id).teaching.refit"] = { _ in s.refit(page: id) }
            out["mac-settings-ai.decisions.page.\(id).proposal.accept"] = { _ in s.acceptProposal(page: id); return true }
            out["mac-settings-ai.decisions.page.\(id).proposal.dismiss"] = { _ in guard s.proposals[id] != nil else { return false }; s.proposals[id] = nil; return true }
            for n in 2..<(page.entries.count + 1) {
                out["mac-settings-ai.decisions.page.\(id).entry.\(n).move-up"] = { _ in
                    guard let i = s.pageIndex(id), n < s.pages[i].entries.count else { return false }
                    s.move(page: id, from: n - 1, to: n - 2); return true
                }
            }
            // The doors of an entry n (the list can grow by an added rule, so n runs past today's count and the live list decides):
            // its switch (the default, last, has none), the click in a model entry's question (`type` then edits it), and, on the Labels page,
            // `node.<k>`: the rule's label becomes row k of `mac-settings-ai.label-menu` as listed (the tree node with its path).
            for n in 1...Self.entryDoors {
                out["mac-settings-ai.decisions.page.\(id).entry.\(n).enabled"] = { _ in
                    guard let i = s.pageIndex(id), n < s.pages[i].entries.count else { return false }
                    s.update(page: id, entry: s.pages[i].entries[n - 1].id) { $0.enabled.toggle() }; return true
                }
                out["mac-settings-ai.decisions.page.\(id).entry.\(n).question"] = { _ in
                    guard let i = s.pageIndex(id), n <= s.pages[i].entries.count, s.pages[i].entries[n - 1].kind == .model else { return false }
                    s.questionFocus = (id, s.pages[i].entries[n - 1].id); return true
                }
                // the trust menu's level k (the entries as `InterimTrust.levels` lists them, Strict first): the door of the button the menu item draws
                for k in 1...3 {
                    out["mac-settings-ai.decisions.page.\(id).entry.\(n).trust.\(k)"] = { _ in
                        guard let i = s.pageIndex(id), n <= s.pages[i].entries.count, let t = s.pages[i].entries[n - 1].trust else { return false }
                        let levels = InterimTrust.levels(shipped: s.pages[i].entries[n - 1].shippedTrust ?? t)
                        guard k <= levels.count else { return false }
                        s.update(page: id, entry: s.pages[i].entries[n - 1].id) { $0.trust = levels[k - 1].1 }; return true
                    }
                }
                guard id == "labels" else { continue }
                for k in 1...Self.nodeDoors {
                    out["mac-settings-ai.decisions.page.labels.entry.\(n).node.\(k)"] = { m in
                        let tree = AILabelTree.nodes(m.core)
                        guard let i = s.pageIndex(id), n <= s.pages[i].entries.count, s.pages[i].entries[n - 1].kind == .rule, k <= tree.count else { return false }
                        s.update(page: id, entry: s.pages[i].entries[n - 1].id) { $0.outcome = tree[k - 1] }; return true
                    }
                }
            }
        }
        AIFixture.listen(s)
        let open: (AppModel) -> Bool = { m in WindowBridge.open(.macSettingsAI, in: m) }
        out["mac-palette.result.ai-settings"] = open   // the palette lane wins by registry order once it lands
        out["mac-palette.result.decisions"] = { m in s.section = .decisions; return WindowBridge.open(.macSettingsAI, in: m) }
        out["mac-settings-ai.open"] = open
        return out
    }

    public static func paletteCommands() -> [(title: String, key: String)] { [(AIWords.paletteTitle, ""), (AIWords.paletteDecisions, "")] }
}

private extension Array {
    func plus(_ other: [Element]) -> [Element] { self + other }
}
