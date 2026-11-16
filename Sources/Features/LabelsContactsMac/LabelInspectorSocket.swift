import Foundation

/// The inspector on the control socket: the region `mac-labels.inspector` (and its rules, matches and result), and the click
/// ids that drive every control of the view through the same model calls the real controls make. Text the socket's `type`
/// verb sent (`AppModel.typed`) goes into the field the click names.
@MainActor
enum LabelInspectorSocket {
    static let ax = "mac-labels.inspector"
    private static var m: LabelInspectorModel { LabelInspectorModel.shared }

    static func rows() -> [String: (AppModel) throws -> [ControlRow]] {
        LabelFlowSocket.rows().merging([
            ax: { _ in summary() },
            "\(ax).rules": { _ in
                m.draft.rules.enumerated().map { i, r in
                    ["ax": "\(ax).rule.\(i)", "index": i, "kind": r.kind.rawValue, "title": r.kind.title, "field": r.kind.fieldTitle, "op": r.op.rawValue, "op_title": r.op.title, "values": r.allValues,
                     "count": i < m.proof.perRule.count ? m.proof.perRule[i] : NSNull(), "error": (r.kind == .account || i == 0) ? (m.refusal.flatMap { $0.field == "rules" ? $0.words : nil } ?? NSNull()) : NSNull(),
                     "incomplete": r.hasBlankValue, "expanded": r.expanded, "shown": LabelRuleDoors.shown(r), "field_open": LabelRuleDoors.fieldOpen(r), "focused": m.focusedRule == r.id]
                }
            },
            "\(ax).matches": { _ in
                let p = m.proof
                let list = p.hasPreview ? p.join + p.leave + p.stay : p.rows
                return list.map { r in
                    ["ax": "\(ax).row.\(r.id)", "id": r.id, "sender": r.senderName, "subject": r.subject, "tag": r.tag,
                     "edge": r.edge?.rawValue ?? NSNull(), "highlight": r.highlight ?? NSNull()]
                }
            },
            // the empty list: the sentence it says while the AI cannot sort and the label has no rule, and the control that opens Decisions
            "\(ax).empty": { _ in
                guard m.isOpen, m.proof.rows.isEmpty, !m.proof.hasPreview, m.draft.rules.allSatisfy(\.isEmpty), let why = m.aiRefusalShown else { return [] }
                return [["ax": "\(ax).empty", "words": why, "door": m.refusalDoor.map { "\(ax).\($0.id)" } ?? NSNull()]]
            },
            // the URLs the Apple Intelligence door opened (a test copy opens nothing and records the URL it would open)
            "\(ax).apple-settings": { _ in [["ax": "\(ax).apple-settings", "opened": m.appleSettingsOpened, "count": m.appleSettingsOpened.count]] },
            "\(ax).result": { _ in
                guard let r = m.result else { return [] }
                return r.decided.map { ["ax": "\(ax).result.\($0.messageId)", "id": $0.messageId, "decided_by": $0.by, "card": $0.card ?? NSNull(), "chain": $0.chain ?? NSNull(), "filed": r.filed, "refiled": r.refiled, "unfiled": r.unfiled] }
                    + (r.decided.isEmpty ? [["ax": "\(ax).result", "filed": r.filed, "refiled": r.refiled, "unfiled": r.unfiled]] : [])
            },
        ]) { a, _ in a }
    }

    private static func summary() -> [ControlRow] {
        let p = m.proof, d = m.draft
        var row: ControlRow = [
            "ax": ax, "open": m.isOpen, "mode": m.isNew ? "new" : (m.labelId != nil ? "modify" : "closed"), "label_id": m.labelId ?? NSNull(),
            "name": d.name, "parent": d.parentId ?? NSNull(), "colour": d.colour ?? NSNull(), "scope": m.scopeAccountName ?? "all",
            "pinned": d.pinned, "meaning": d.meaning, "match": d.matchMode == .any ? "any" : "all", "rules": d.rules.count,
            "count": p.count, "unread": p.unread, "newest": p.newestMs.map { $0 as Any } ?? NSNull(), "counting": m.counting,
            "per_rule": p.perRule, "matches": p.rows.count, "join": p.join.count, "leave": p.leave.count, "stay": p.stay.count,
            "unaffected": p.unaffected, "preview": p.hasPreview, "changes": m.changes, "dirty": m.dirty, "showing": m.showMatches ? "matches" : "mailbox",
            "status": m.statusLine ?? NSNull(), "refusal": m.refusal?.words ?? NSNull(), "failure": m.failure ?? NSNull(),
            "can_write": m.canWrite, "was": p.was.map { $0 as Any } ?? NSNull(), "foot": m.footKind, "preview_line": m.previewLine ?? NSNull(), "engine_preview": m.proof.engine.map { ["by_rule": $0.byRule, "by_ai": $0.byAi, "leave": $0.leave, "stay": $0.stay, "ai_words": $0.aiWords ?? ""] as [String: Any] } ?? NSNull(), "closest": m.closestDomain ?? NSNull(),
        ]
        row["offer"] = m.offer.map { ["filed": $0.filed, "join": $0.join, "leave": $0.leave, "stay": $0.stay] as [String: Any] } ?? NSNull()
        return [row]
    }

    // MARK: clicks

    /// The typed text, taken once.
    private static func typed(_ a: AppModel) -> String { let t = a.typed; a.typed = ""; return t }

    static func clicks() -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [
            "\(ax).open": { _ in LabelInspector.toggle() },
            "\(ax).open.new": { _ in LabelInspector.openNew(parent: nil) },
            "\(ax).toolbar": { _ in m.isOpen ? m.close() : LabelInspector.toggle() },
            "\(ax).close": { _ in m.close() }, "\(ax).cancel": { _ in m.close() }, "\(ax).done": { _ in m.footKind == "clean" && m.close() },
            "\(ax).create": { _ in m.create() }, "\(ax).apply": { _ in m.apply() },
            "\(ax).rerun": { _ in m.rerun() }, "\(ax).notnow": { _ in m.notNow() },
            "\(ax).blur": { _ in m.blur() },
            "\(ax).name": { a in let v = typed(a); guard m.isOpen, !v.isEmpty else { return m.isOpen }; m.edit { $0.name = v }; return true },
            "\(ax).meaning": { a in let v = typed(a); guard m.isOpen else { return false }; m.edit { $0.meaning = v }; return true },
            "\(ax).pinned": { _ in guard m.isOpen else { return false }; m.edit { $0.pinned.toggle() }; return true },
            "\(ax).inside.top": { _ in guard m.isOpen else { return false }; m.edit { $0.parentId = nil }; return true },
            "\(ax).scope.all": { _ in guard m.isOpen else { return false }; m.edit { $0.scope = .all }; return true },
            "\(ax).colour.none": { _ in guard m.isOpen else { return false }; m.edit { $0.colour = nil }; return true },
            "\(ax).drop": { a in guard m.isOpen, let id = a.focusedMessageId else { return false }; return m.ruleFromMessage(id) },
            "\(ax).decisions": { _ in m.aiRefusalShown != nil && m.openDecisions() },
            "\(ax).laya.download": { _ in m.downloadLaya() },
            "\(ax).apple.settings": { _ in m.openAppleSettings() },
            "\(ax).useclosest": { _ in m.closestDomain != nil && useClosest() },
            "\(ax).switch.matches": { _ in guard m.isOpen else { return false }; m.showMatches = true; return true },
            "\(ax).switch.mailbox": { _ in guard m.isOpen else { return false }; m.showMatches = false; return true },
            "\(ax).proof.hold": { _ in guard m.isOpen else { return false }; m.holdCounting.toggle(); m.counting = m.holdCounting; if !m.holdCounting { m.recompute() }; return true },
        ]
        for (i, s) in LabelPalette.swatches.enumerated() {
            t["\(ax).colour.\(i)"] = { _ in guard m.isOpen else { return false }; m.edit { $0.colour = s }; return true }
        }
        t.merge(LabelFlowSocket.clicks()) { a, _ in a }
        t.merge(LabelRuleDoors.clicks()) { a, _ in a }
        for l in AppModel.shared.core.labels() {   // every label opens (the built-in À répondre carries the board's example flow)
            t["\(ax).open.modify.\(l.id)"] = { _ in LabelInspector.openModify(l.id) }
            guard l.kind == .owner else { continue }
            t["\(ax).inside.\(l.id)"] = { _ in guard m.isOpen else { return false }; m.edit { $0.parentId = l.id }; return true }
            t["\(ax).open.new.\(l.id)"] = { _ in LabelInspector.openNew(parent: l.id) }
        }
        for a in AppModel.shared.core.accounts() {
            t["\(ax).scope.\(a.id)"] = { _ in guard m.isOpen else { return false }; m.edit { $0.scope = .account(accountId: a.id) }; return true }
        }
        return t
    }

    private static func useClosest() -> Bool {
        guard m.isOpen, let r = m.draft.rules.first(where: { $0.kind == .domain && !$0.allValues.isEmpty }), let typed = r.allValues.last,
              let near = DomainNearness.closest(to: typed, in: m.corpus.map(\.facts.fromAddress)) else { return false }
        m.editRule(r.id) { $0.pending = ""; $0.values = [near.domain] }
        return true
    }
}
