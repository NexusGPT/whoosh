import Foundation

/// What the socket reads of Settings › Réponses: the values the panes draw, in the words and figures they draw them with, so a check proves
/// the screen and the core agree after every click.
@MainActor
enum RepliesRows {
    static func state(_ s: RepliesStore) -> ControlRow {
        let a = s.use(.auto), d = s.use(.onDemand), f = s.filter, r = s.retrieval, c = s.counts
        func facts(_ rule: RuleSet?) -> [String: String] {
            Dictionary(uniqueKeysWithValues: RepliesFacts.order.map { fact in
                let st = rule.map { RepliesFacts.state($0.conditions, fact) } ?? .any
                return (messageFactKey(fact: fact), st == .yes ? "yes" : (st == .no ? "no" : "any"))
            })
        }
        let scopeIds: [String] = { if case .labels(let ids) = f?.scope { return ids } else { return [] } }()
        var row: ControlRow = [
            "auto_enabled": a?.enabled ?? NSNull(), "auto_tone": a?.toneId ?? NSNull(), "auto_match_thread": a?.matchThread ?? NSNull(),
            "on_demand_tone": d?.toneId ?? NSNull(), "on_demand_match_thread": d?.matchThread ?? NSNull(),
            "instruction_before": s.onDemand.instructionBefore, "change_after": s.onDemand.changeAfter,
            "scope_all": f.map { if case .all = $0.scope { return true } else { return false } } ?? NSNull(), "scope_label_ids": scopeIds,
            "entry_on": f?.entryEnabled ?? NSNull(), "ai_on": f?.aiEnabled ?? NSNull(), "exit_on": f?.exitEnabled ?? NSNull(),
            "entry_match": f.map { matchModeKey(v: $0.entryRule.matchMode) } ?? NSNull(), "exit_match": f.map { matchModeKey(v: $0.exitRule.matchMode) } ?? NSNull(),
            "entry_facts": facts(f?.entryRule), "exit_facts": facts(f?.exitRule), "sender": RepliesFacts.sender(f?.entryRule.conditions ?? []),
            "reads": f.map { ["subject": $0.aiReads.subject, "body": $0.aiReads.body, "rest": $0.aiReads.restOfThread, "attachments": $0.aiReads.attachments] } ?? [:],
            "description": RepliesWords.aiDescriptionShown(f?.aiDescription ?? ""),   // as the card shows it (round 6)
            "counts": c.map { ["received": Int($0.received), "after_entry": Int($0.afterEntry), "after_ai": Int($0.afterAi), "drafts": Int($0.drafts)] } ?? [:],
            "auto_lines": c.map { [RepliesWords.kept($0.afterEntry, of: $0.received), RepliesWords.kept($0.afterAi, of: $0.afterEntry), RepliesWords.kept($0.afterExit, of: $0.afterAi)] } ?? [],
            "sources_line": RepliesWords.sourcesOn(s.sources.enabledTotal), "search_scope": retrievalScopeKey(v: s.sources.scope),
        ]
        row["retrieval"] = r.map { r -> ControlRow in
            ["per_question": Int(r.excerptsPerQuestion), "size": excerptSizeKey(v: r.excerptSize), "fields": RepliesWords.fieldsLine(r.attachedFields),
             "keep": Int(r.keepN), "clash": clashRuleKey(v: r.clashRule), "ai_filter": r.aiFilterQuestion ?? NSNull(), "prompt": r.retrievalPrompt]
        } ?? NSNull()
        row["voice"] = ["text": s.voice?.toneText ?? "", "sample": Int(s.voice?.sampleSize ?? 0), "picked_sample": s.voiceSample.map { Int($0) } ?? NSNull(),
                        "line": RepliesWords.lastAnalysis(s.voice, s.analysis), "examples": Int(s.voice?.samePersonExamples ?? 0),
                        "tones": s.tones.map(\.name)] as ControlRow
        row["prompt"] = ["version": s.prompt.map { Int($0.version) } ?? NSNull(), "text": s.prompt?.text ?? "", "versions": s.versions.count,
                         "variables": s.variables.count, "notes": s.notes.map(\.title), "preview": s.preview ?? NSNull(), "preview_of_draft": s.previewOfDraft] as ControlRow
        row["ui"] = ["page": RepliesRouter.current().rawValue, "popover": s.popover?.rawValue ?? NSNull(), "open_accounts": Array(s.openAccounts).sorted(), "picker_open": RepliesFolderPicker.isOpen,
                     "error": s.error ?? NSNull(), "refusals": s.refusals,
                     "trial_draft": s.trial.map { $0.draftText ?? "" } ?? NSNull(), "trial_kind": s.trial.map { replyUseKindKey(v: $0.kind) } ?? NSNull()] as ControlRow
        return row
    }

    /// One row per tile and calendar, as Sources draws them: the words on the tile, on or off, its sync state.
    static func sources(_ s: RepliesStore) -> [ControlRow] {
        var out: [ControlRow] = []
        for a in s.sources.accounts {
            out.append(["ax": RepliesAx.id("sources", "account", a.accountId), "kind": "account", "line": RepliesWords.foldersLine(a),
                        "open": s.openAccounts.contains(a.accountId)])
            for (i, f) in a.folders.enumerated() {
                out.append(["ax": RepliesAx.id("sources", "account", a.accountId, "folder", "\(i)"), "kind": "mail_folder", "name": RepliesWords.folderName(f.source),
                            "enabled": f.enabled, "count": RepliesWords.count(f), "progress": RepliesWords.progress(f.progress),
                            "gone": f.isGone, "line": f.isGone ? RepliesWords.goneLine : "",
                            "wish": wish(s, f)])
            }
        }
        for (i, f) in s.sources.local.enumerated() {
            out.append(["ax": RepliesAx.id("sources", "local", "\(i)"), "kind": "local_folder", "name": RepliesWords.folderName(f.source), "enabled": f.enabled,
                        "path": { if case .localFolder(let p) = f.source { return p } else { return "" } }(), "count": RepliesWords.count(f)])
        }
        for c in s.sources.calendars {
            out.append(["ax": RepliesAx.id("sources", "calendar", c.calendarId), "kind": "calendar", "name": c.name, "enabled": c.enabled])
        }
        return out
    }

    /// The owner's remembered choice for a gone folder's return, as the core reads it (null: the folder is present).
    private static func wish(_ s: RepliesStore, _ f: ReplySource) -> Any {
        guard let w = s.sources.wish(f) else { return NSNull() }
        return w
    }

    static func labels(_ s: RepliesStore) -> [ControlRow] {
        let ticked: [String] = { if case .labels(let ids) = s.filter?.scope { return ids } else { return [] } }()
        return s.labels.map { ["ax": RepliesAx.id("filter", "scope", "label", $0.id), "id": $0.id, "name": $0.name, "ticked": ticked.contains($0.id)] }
    }
}
