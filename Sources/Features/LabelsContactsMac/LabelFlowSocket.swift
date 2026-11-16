import Foundation

/// The three cards on the control socket: the region `mac-labels.inspector.flow` (a row per card, a row per chain with its eight facts, rows and
/// mode), `.foot` (the four figures as the screen says them) and `.why` (the lines « Pourquoi ici ? » shows), and the click ids that drive every
/// control of the cards through the same model calls the real controls make. A chain is named `entry` (the first Entry chain), `other1`..,
/// `exit`. Ids of the first Entry chain also answer without the `entry.` (`rule.0.value`, `match.all`), as in the round 2 inspector.
@MainActor
enum LabelFlowSocket {
    static let ax = LabelInspectorSocket.ax
    private static var m: LabelInspectorModel { LabelInspectorModel.shared }

    static func rows() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "\(ax).flow": { _ in cards() + chains() },
            "\(ax).foot": { _ in [foot()] },
            "\(ax).why": { _ in m.whyLines.enumerated().map { ["ax": "\(ax).why.\($0.offset)", "index": $0.offset, "text": $0.element] } },
        ]
    }

    private static func cards() -> [ControlRow] {
        let f = m.draft.flow
        return [
            ["ax": "\(ax).flow.entry", "card": "entry", "title": Words.text("Entry rules"), "on": f.entryOn, "count": m.entryCountText, "chains": 1 + f.others.count,
             "after": m.countsAfter?.entry.map { $0 as Any } ?? NSNull(), "after_text": m.entryAfterText ?? NSNull()],
            ["ax": "\(ax).flow.ai", "card": "ai", "title": Words.text("AI sorting"), "on": f.aiOn, "count": m.aiCountText, "after": m.countsAfter?.ai.map { $0 as Any } ?? NSNull(), "after_text": m.aiAfterText ?? NSNull(),
             "reads": ["subject": f.reads.subject, "body": f.reads.body, "rest_of_thread": f.reads.restOfThread, "attachments": f.reads.attachments],
             "error": m.refusal(for: [FlowRefusal.aiReadsNothing]) ?? NSNull(), "ai_refusal": m.aiRefusalShown ?? NSNull(), "door": m.refusalDoor.map { "\(LabelInspectorSocket.ax).\($0.id)" } ?? NSNull(),
             "rerunning": m.rerunning],
            ["ax": "\(ax).flow.exit", "card": "exit", "title": Words.text("Exit filter"), "on": f.exitOn, "count": m.exitCountText, "after": NSNull(),   // the preview carries no kept figure (EnginePreview, LabelProof.swift), so none is shown
             "fallback": f.fallback ?? NSNull(), "fallback_name": m.fallbackName(f.fallback) ?? NSNull(),
             "error": m.refusal(for: [FlowRefusal.exitNeedsFallback, FlowRefusal.badFallback]) ?? NSNull()],
        ]
    }

    private static func chains() -> [ControlRow] {
        var out: [ControlRow] = []; let d = m.draft
        var named: [(String, ChainDraft)] = [("entry", d.entry)]
        for (i, c) in d.flow.others.enumerated() { named.append(("other\(i + 1)", c)) }
        if let e = d.flow.exits.first { named.append(("exit", e)) } else { named.append(("exit", ChainDraft(rows: []))) }
        for (key, c) in named {
            let facts = Dictionary(uniqueKeysWithValues: zip(ChainDraft.order, c.facts).map { (messageFactKey(fact: $0), state($1)) })
            out.append(["ax": "\(ax).\(key)", "chain": key, "match": c.matchMode == .all ? "all" : "any", "facts": facts, "set": c.setFacts,
                        "rows": c.rows.map { ["kind": $0.kind.rawValue, "field": $0.kind.fieldTitle, "op": $0.op.rawValue, "values": $0.allValues] as [String: Any] }, "rows_n": c.rows.count,
                        "chain_id": c.chainId.map { Int($0) as Any } ?? NSNull()])
        }
        return out
    }

    private static func state(_ s: FactState) -> String { switch s { case .any: "any"; case .yes: "yes"; case .no: "no" } }

    private static func foot() -> ControlRow {
        let parts = m.provenanceParts
        var row: ControlRow = ["ax": "\(ax).foot", "text": parts.map { $0.joined(separator: " · ") } ?? NSNull(), "parts": parts ?? [], "why_open": m.whyOpen]
        row["now_after"] = m.nowAfterLine() ?? NSNull()
        if let c = m.counts { row["caught_by_entry"] = c.entry; row["sorted_by_ai"] = c.ai; row["kept_by_exit"] = c.kept; row["sent_to_fallback"] = c.sent }
        row["unsorted"] = (m.whyOpen ? m.app.focusedMessageId.flatMap { LabelInspectorFoot.unsorted(m.core.state(path: "message/\($0)/filing")) } : nil) ?? NSNull()   // the line « Pourquoi ici ? » draws when the AI could not sort the mail in focus
        row["held"] = m.counts.map { $0.held(aiOn: m.saved.flow.aiOn, exitOn: m.saved.flow.exitOn) as Any } ?? NSNull()
        return row
    }

    private static func typed(_ a: AppModel) -> String { let t = a.typed; a.typed = ""; return t }
    static func ref(_ key: String) -> ChainRef? {
        if key == "entry" { return .entry }
        if key == "exit" { return .exit }
        guard key.hasPrefix("other"), let n = Int(key.dropFirst(5)), n >= 1, m.draft.flow.others.indices.contains(n - 1) else { return nil }
        return .other(m.draft.flow.others[n - 1].id)
    }

    static func clicks() -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [
            "\(ax).flow.entry.switch": { _ in guard m.isOpen else { return false }; m.setCard(.entry, !m.draft.flow.entryOn); return true },
            "\(ax).flow.ai.switch": { _ in guard m.isOpen else { return false }; m.setCard(.ai, !m.draft.flow.aiOn); return true },
            "\(ax).flow.exit.switch": { _ in guard m.isOpen else { return false }; m.setCard(.exit, !m.draft.flow.exitOn); return true },
            "\(ax).flow.ai.reads.subject": { _ in guard m.isOpen else { return false }; m.toggleRead(.subject); return true },
            "\(ax).flow.ai.reads.body": { _ in guard m.isOpen else { return false }; m.toggleRead(.body); return true },
            "\(ax).flow.ai.reads.rest_of_thread": { _ in guard m.isOpen else { return false }; m.toggleRead(.restOfThread); return true },
            "\(ax).flow.ai.reads.attachments": { _ in guard m.isOpen else { return false }; m.toggleRead(.attachments); return true },
            "\(ax).flow.exit.fallback.primary": { _ in guard m.isOpen else { return false }; m.setFallback("primary"); return true },
            "\(ax).flow.exit.fallback.junk": { _ in guard m.isOpen else { return false }; m.setFallback("junk"); return true },
            "\(ax).other.add": { _ in m.addOtherChain() },
            "\(ax).why": { _ in m.toggleWhy() },
        ]
        for l in AppModel.shared.core.labels() where l.kind == .owner {
            t["\(ax).flow.exit.fallback.\(l.id)"] = { _ in guard m.isOpen else { return false }; m.setFallback(l.id); return true }
        }
        let keys = ["entry", "other1", "other2", "other3", "other4", "exit"]
        for key in keys { chainClicks(key, prefix: "\(ax).\(key)", into: &t) }
        chainClicks("entry", prefix: ax, into: &t)
        for n in 1...4 { t["\(ax).other\(n).remove"] = { _ in
            guard m.isOpen, case .other(let id)? = ref("other\(n)") else { return false }
            m.removeChain(id); return true } }
        return t
    }

    /// Every control of one chain: its mode, each fact's three states, Add rule, and rule row `i`'s value, commit, remove, kind and account.
    private static func chainClicks(_ key: String, prefix: String, into t: inout [String: (AppModel) -> Bool]) {
        func go(_ f: @escaping (ChainRef) -> Void) -> (AppModel) -> Bool { { _ in guard m.isOpen, let r = ref(key) else { return false }; f(r); return true } }
        t["\(prefix).match.all"] = go { m.setMode($0, .all) }
        t["\(prefix).match.any"] = go { m.setMode($0, .any) }
        t["\(prefix).rule.add"] = go { m.addRule($0) }
        for (i, fact) in ChainDraft.order.enumerated() {
            for (word, s) in [("any", FactState.any), ("yes", .yes), ("no", .no)] {
                t["\(prefix).fact.\(messageFactKey(fact: fact)).\(word)"] = go { m.setFact($0, i, s) }
            }
        }
        for i in 0..<12 {
            func row(_ r: ChainRef) -> RuleDraft? { let rows = m.chain(r)?.rows ?? []; return rows.indices.contains(i) ? rows[i] : nil }
            t["\(prefix).rule.\(i).value"] = { a in
                guard m.isOpen, let r = ref(key), let rule = row(r) else { return false }
                let v = typed(a); if !v.isEmpty { m.editRule(r, rule.id) { $0.pending = v } }; return true
            }
            t["\(prefix).rule.\(i).commit"] = { _ in guard m.isOpen, let r = ref(key), let rule = row(r) else { return false }; m.commitPending(r, rule.id); return true }
            t["\(prefix).rule.\(i).remove"] = { _ in guard m.isOpen, let r = ref(key), let rule = row(r) else { return false }; m.removeRule(r, rule.id); return true }
            for k in RuleKind.allCases {
                t["\(prefix).rule.\(i).kind.\(k.rawValue)"] = { _ in guard m.isOpen, let r = ref(key), let rule = row(r) else { return false }; m.editRule(r, rule.id) { $0.setField(k) }; return true }
            }
            for o in RuleOp.allCases {
                t["\(prefix).rule.\(i).op.\(o.rawValue)"] = { _ in guard m.isOpen, let r = ref(key), let rule = row(r), rule.kind.operators.contains(o) else { return false }; m.editRule(r, rule.id) { $0.op = o }; return true }
            }
            for a in AppModel.shared.core.accounts() {
                t["\(prefix).rule.\(i).account.\(a.id)"] = { _ in
                    guard m.isOpen, let r = ref(key), let rule = row(r) else { return false }
                    m.editRule(r, rule.id) { if !$0.values.contains(a.id) { $0.values.append(a.id) } }; return true
                }
            }
        }
    }
}
