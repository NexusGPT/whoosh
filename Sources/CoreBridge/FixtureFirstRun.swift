import Foundation

/// `fixture install-state new` (probes-r1, rounds 5 and 6, plan R7): the fixture core serves a NEW installation instead of the pre-filled
/// sample, field by field as the REAL core serves it. The values are NOT typed here: `FixtureFirstRunData.swift` is generated from the golden
/// `docs/FIRST-RUN.json` (checks/checklib/firstrun_data.py) and this file applies each class of it to the fixture core: labels, providers, the reply
/// filter, auto-draft, the Replies sources, the label AI's refusal. Mail and accounts stay (compare the settings, not the mail). The Sources
/// pane lists the first two fixture accounts (the golden's `$ACCOUNT_1` and `$ACCOUNT_2`). `fixture reset` brings the sample back.
extension FixtureCore {
    public func installNew() {
        lock.withLock { restoreSeedLocked(); applyNewInstallLocked() }
        emit()
    }

    func applyNewInstallLocked() {
        newInstall = true
        let g = FirstRunGolden.data
        applyLabels(g["labels"] as? [[String: Any]] ?? [])
        applyProviders(g["providers"] as? [[String: Any]] ?? [])
        flowCard = [:]; flowRejected = [:]; filedBy = [:]; assigned = [:]
        folderRows = []
        replyBookStore = nil
    }

    // MARK: class labels

    private func fact(_ name: String) -> MessageFact? {
        ["unread": .unread, "unanswered": .unanswered, "has_attachment": .hasAttachment, "starred": .starred, "vip_sender": .vipSender,
         "cc_me": .ccMe, "mailing_list": .mailingList, "replied_by_me": .repliedByMe][name]
    }

    private func rules(_ rows: [[String: Any]]) -> RuleSet {
        RuleSet(conditions: rows.compactMap { c in
            guard c["kind"] as? String == "fact", let f = (c["fact"] as? String).flatMap(fact) else { return nil }
            return .fact(fact: f, wanted: c["wanted"] as? Bool ?? true)
        }, matchMode: .all)
    }

    private func applyLabels(_ golden: [[String: Any]]) {
        labelRows = labelRows.filter { $0.kind == .builtIn }                       // no owner label
        let keep = Set(labelRows.map(\.id))
        labelRuleSets = [:]; entryChainIds = [:]; exitChains = [:]
        for l in golden {
            guard let id = l["id"] as? String, keep.contains(id) else { continue }
            let reads = l["ai_reads"] as? [String: Bool] ?? [:]
            setLabel(id) {
                $0.description = l["description"] as? String ?? ""
                $0.entryEnabled = l["entry_enabled"] as? Bool ?? true
                $0.aiEnabled = l["ai_enabled"] as? Bool ?? true
                $0.exitEnabled = l["exit_enabled"] as? Bool ?? false
                $0.exitFallback = l["exit_fallback"] as? String
                $0.pinned = l["pinned"] as? Bool ?? false
                $0.aiReads = AiReads(subject: reads["subject"] ?? true, body: reads["body"] ?? true, restOfThread: reads["rest_of_thread"] ?? false, attachments: reads["attachments"] ?? false)
            }
            let chains = l["chains"] as? [String: [[[String: Any]]]] ?? [:]
            let entry = chains["entry"] ?? [], exit = chains["exit"] ?? []
            if !entry.isEmpty { labelRuleSets[id] = entry.map(rules); entryChainIds[id] = entry.indices.map { UInt32($0) } }
            if !exit.isEmpty { exitChains[id] = exit.enumerated().map { LabelChain(chainId: UInt32($0.offset), rules: rules($0.element)) } }
            flowEdited.insert(id)                                                   // figures derived (0 messages), not the sample's example
        }
        syncTabsFromLabels()
    }

    // MARK: class providers

    private func applyProviders(_ golden: [[String: Any]]) {
        agents.providers = golden.map { p in
            AiProvider(id: p["id"] as? String ?? "", name: p["name"] as? String ?? "", models: p["models"] as? [String] ?? [],
                       connected: p["connected"] as? Bool ?? false, keychain: nil)
        }
    }

    /// class refusal: why the label AI sorted nothing (`label/<id>/flow/ai_refusal`), served while no decision model is chosen.
    func firstRunRefusalLocked() -> String? {
        guard newInstall, settings.decision == nil, let r = FirstRunGolden.data["refusal"] as? [String: Any],
              let d = try? JSONSerialization.data(withJSONObject: r, options: [.sortedKeys]) else { return nil }
        return String(data: d, encoding: .utf8)
    }
}

extension FirstRunGolden {
    /// The embedded subset of the golden, parsed once.
    nonisolated(unsafe) static let data: [String: Any] = (try? JSONSerialization.jsonObject(with: Data(json.utf8))) as? [String: Any] ?? [:]
}

extension FixtureReplyBook {
    /// class filter, auto, sources: the Replies settings of a new installation. No calendar, no local folder, no pass, no draft, no note.
    func makeNewInstall(accounts: [String]) {
        let g = FirstRunGolden.data
        calendars = []; calendarEvents = []
        passes = []; drafts = []; notes = []; localFolders = []
        uses[.auto]?.enabled = g["auto"] as? Bool ?? false
        var f = defaultReplyFilter()
        let gf = g["filter"] as? [String: Any] ?? [:]
        f.aiDescription = gf["ai_description"] as? String ?? ""
        f.aiEnabled = gf["ai_enabled"] as? Bool ?? true
        f.entryEnabled = gf["entry_enabled"] as? Bool ?? true
        f.exitEnabled = gf["exit_enabled"] as? Bool ?? true
        filter = f
        mailFolders = []
        for acc in g["sources"] as? [[String: Any]] ?? [] {
            guard let n = acc["account"] as? Int, accounts.indices.contains(n - 1) else { continue }
            for x in acc["folders"] as? [[String: Any]] ?? [] {
                let p = x["progress"] as? [String: Any] ?? [:]
                let progress: ReplySyncProgress = p["state"] as? String == "syncing" ? .syncing(percent: UInt8(p["percent"] as? Int ?? 0))
                    : p["state"] as? String == "up_to_date" ? .upToDate : .notStarted
                mailFolders.append(ReplySource(source: .mailFolder(accountId: accounts[n - 1], folder: x["name"] as? String ?? ""),
                                               enabled: x["enabled"] as? Bool ?? false, itemCount: 0, progress: progress))
            }
        }
    }
}
