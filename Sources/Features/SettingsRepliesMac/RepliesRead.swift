import Foundation

/// The reads of Settings › Réponses: each `reply/...` state path (core/crates/contracts/src/reply_ai_state.rs) as the contract's own types, so a
/// write sends back exactly what was read. A key the JSON lacks gives the contract's default, never a guess; a rule's conditions decode with the
/// label inspector's `StoredRules` (one decoder for stored conditions).
enum RepliesRead {
    typealias O = [String: Any]

    static func object(_ json: String?) -> O? { StoredRules.object(json) as? O }
    static func list(_ json: String?) -> [O] { StoredRules.object(json) as? [O] ?? [] }
    static func u32(_ v: Any?) -> UInt32 { (v as? NSNumber)?.uint32Value ?? 0 }
    static func flag(_ v: Any?) -> Bool { (v as? Bool) ?? false }

    static func use(_ o: O?, _ kind: ReplyUseKind) -> ReplyUse? {
        guard let o else { return nil }
        return ReplyUse(kind: kind, enabled: flag(o["enabled"]), toneId: o["tone_id"] as? String ?? "voice",
                        matchThread: flag(o["match_thread"]), promptId: o["prompt_id"] as? String ?? "replies")
    }

    static func rule(_ o: Any?) -> RuleSet {
        let r = o as? O ?? [:]
        return RuleSet(conditions: StoredRules.conditions(r["conditions"]), matchMode: StoredRules.mode(r["match"]))
    }

    static func filter(_ o: O?) -> ReplyFilter? {
        guard let o, let entry = o["entry"] as? O, let ai = o["ai"] as? O, let exit = o["exit"] as? O else { return nil }
        let scopeO = o["scope"] as? O ?? [:]
        let scope: ReplyLabelScope = flag(scopeO["all"]) ? .all : .labels(labelIds: scopeO["label_ids"] as? [String] ?? [])
        let reads = AiReads(subject: flag(ai["subject"]), body: flag(ai["body"]), restOfThread: flag(ai["rest_of_thread"]), attachments: flag(ai["attachments"]))
        return ReplyFilter(scope: scope, entryEnabled: flag(entry["enabled"]), entryRule: rule(entry["rule"]), aiEnabled: flag(ai["enabled"]),
                           aiReads: reads, aiDescription: ai["description"] as? String ?? "", exitEnabled: flag(exit["enabled"]), exitRule: rule(exit["rule"]))
    }

    static func counts(_ o: O?) -> ReplyStepCounts? {
        guard let o else { return nil }
        return ReplyStepCounts(days: u32(o["days"]), received: u32(o["received"]), afterEntry: u32(o["after_entry"]), afterAi: u32(o["after_ai"]),
                               afterExit: u32(o["after_exit"]), drafts: u32(o["drafts"]))
    }

    static func ref(_ o: Any?) -> ReplySourceRef? {
        guard let o = o as? O else { return nil }
        switch o["kind"] as? String {
        case "mail_folder": return .mailFolder(accountId: o["account_id"] as? String ?? "", folder: o["folder"] as? String ?? "")
        case "local_folder": return .localFolder(path: o["path"] as? String ?? "")
        case "calendar": return .calendar(calendarId: o["calendar_id"] as? String ?? "")
        default: return nil
        }
    }

    static func progress(_ o: Any?) -> ReplySyncProgress {
        let p = o as? O ?? [:]
        switch p["state"] as? String {
        case "syncing": return .syncing(percent: UInt8(min(100, u32(p["percent"]))))
        case "up_to_date": return .upToDate
        case "failed": return .failed(reasonWords: p["reason_words"] as? String ?? "")
        case "gone": return .gone   // delta 24: no longer on the server; the row is off and keeps its last count
        default: return .notStarted
        }
    }

    static func source(_ o: O) -> ReplySource? {
        guard let r = ref(o["source"]) else { return nil }
        return ReplySource(source: r, enabled: flag(o["enabled"]), itemCount: (o["item_count"] as? NSNumber)?.uint64Value ?? 0, progress: progress(o["progress"]))
    }

    static func sources(_ o: O?) -> RepliesSources {
        guard let o else { return .empty }
        let accounts = (o["accounts"] as? [O] ?? []).map { a in
            ReplyAccountSources(accountId: a["account_id"] as? String ?? "", folders: (a["folders"] as? [O] ?? []).compactMap(source),
                                enabledFolders: u32(a["enabled_folders"]), totalFolders: u32(a["total_folders"]),
                                enabledItems: (a["enabled_items"] as? NSNumber)?.uint64Value ?? 0)
        }
        let calendars = (o["calendars"] as? [O] ?? []).map { c in
            ReplyCalendar(calendarId: c["calendar_id"] as? String ?? "", name: c["name"] as? String ?? "", accountLabel: c["account_label"] as? String ?? "",
                          colour: c["colour"] as? String ?? "", enabled: flag(c["enabled"]))
        }
        // delta 24: each gone folder carries the owner's remembered switch (`owner_enabled`, null while present), kept by its source key
        var owner: [String: Bool] = [:]
        for f in (o["accounts"] as? [O] ?? []).flatMap({ $0["folders"] as? [O] ?? [] }) + (o["local_folders"] as? [O] ?? []) {
            if let key = f["key"] as? String, let on = f["owner_enabled"] as? Bool { owner[key] = on }
        }
        return RepliesSources(accounts: accounts, local: (o["local_folders"] as? [O] ?? []).compactMap(source), calendars: calendars,
                              scope: retrievalScopeFromKey(key: o["scope"] as? String ?? "") ?? .thisAccount, enabledTotal: Int(u32(o["enabled_total"])), owner: owner)
    }

    static func retrieval(_ o: O?) -> RetrievalSettings? {
        guard let o else { return nil }
        let f = o["attached_fields"] as? O ?? [:]
        return RetrievalSettings(retrievalPrompt: o["retrieval_prompt"] as? String ?? "", questionsMax: u32(o["questions_max"]),
                                 excerptsPerQuestion: u32(o["excerpts_per_question"]), excerptSize: excerptSizeFromKey(key: o["excerpt_size"] as? String ?? "") ?? .paragraph,
                                 attachedFields: ExcerptFields(date: flag(f["date"]), sender: flag(f["sender"]), folder: flag(f["folder"]), subject: flag(f["subject"])),
                                 keepN: u32(o["keep_n"]), clashRule: clashRuleFromKey(key: o["clash_rule"] as? String ?? "") ?? .keepBothDated,
                                 aiFilterQuestion: o["ai_filter_question"] as? String, scope: retrievalScopeFromKey(key: o["scope"] as? String ?? "") ?? .thisAccount)
    }
}

/// What the Sources pane draws: one card per account, the folders of this Mac, the calendars, and where an account's reply searches.
struct RepliesSources: Equatable {
    var accounts: [ReplyAccountSources]
    var local: [ReplySource]
    var calendars: [ReplyCalendar]
    var scope: RetrievalScope
    var enabledTotal: Int
    /// The owner's remembered switch of each gone source, by `replySourceKey` (delta 24); absent for a present source.
    var owner: [String: Bool] = [:]
    static let empty = RepliesSources(accounts: [], local: [], calendars: [], scope: .thisAccount, enabledTotal: 0)

    /// What the owner asked for this gone folder's return; nil while it is present.
    func wish(_ s: ReplySource) -> Bool? { s.isGone ? owner[replySourceKey(source: s.source)] : nil }
}
