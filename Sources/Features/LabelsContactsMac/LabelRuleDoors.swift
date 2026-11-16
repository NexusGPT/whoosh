import Foundation

/// The doors of the three small controls on a rule row that had none: the "+N more" chip, the x on a value chip, and the quiet "+" that gives
/// the value field back. Each acts only while the control it stands for is drawn (the same tests the row's view makes), and calls the model
/// exactly as the control does. Ids: `<chain prefix>.rule.<i>.more`, `.chip.<j>.remove`, `.addvalue` (the first Entry chain also answers
/// without its `entry.`, like every other rule id).
@MainActor
enum LabelRuleDoors {
    private static var m: LabelInspectorModel { LabelInspectorModel.shared }

    /// What the row shows of its values: all of them, or the first three until "+N more" is pressed.
    static func shown(_ r: RuleDraft) -> Int { r.expanded || r.values.count <= 3 ? r.values.count : 3 }
    /// The value field is on screen (an account row has the account menu instead): empty row, the field holds the keys, or text is pending.
    static func fieldOpen(_ r: RuleDraft) -> Bool { r.kind != .account && (r.values.isEmpty || m.focusedRule == r.id || !r.pending.isEmpty) }

    static func clicks() -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [:]
        for key in ["entry", "other1", "other2", "other3", "other4", "exit"] {
            for prefix in key == "entry" ? ["\(LabelInspectorSocket.ax).entry", LabelInspectorSocket.ax] : ["\(LabelInspectorSocket.ax).\(key)"] {
                for i in 0..<12 {
                    func find() -> (ChainRef, RuleDraft)? {
                        guard m.isOpen, let r = LabelFlowSocket.ref(key) else { return nil }
                        let rows = m.chain(r)?.rows ?? []
                        return rows.indices.contains(i) ? (r, rows[i]) : nil
                    }
                    t["\(prefix).rule.\(i).more"] = { _ in
                        guard let (r, row) = find(), shown(row) < row.values.count else { return false }
                        m.editRule(r, row.id) { $0.expanded = true }; return true
                    }
                    t["\(prefix).rule.\(i).addvalue"] = { _ in
                        guard let (_, row) = find(), row.kind != .account, !fieldOpen(row) else { return false }
                        m.focusRequest = .rule(row.id)
                        m.focusedRule = row.id        // a copy with no SwiftUI focus change still answers `field_open`; the view's focus writes the same value
                        return true
                    }
                    for j in 0..<12 {
                        t["\(prefix).rule.\(i).chip.\(j).remove"] = { _ in
                            guard let (r, row) = find(), j < shown(row) else { return false }
                            let v = row.values[j]
                            m.editRule(r, row.id) { $0.values.removeAll { $0 == v } }; return true
                        }
                    }
                }
            }
        }
        return t
    }
}

extension LabelInspectorModel {
    /// The nearest known sender domain the empty-proof pane offers ("Did you mean"): only while that pane is drawn (rows empty, not counting, no preview).
    var closestDomain: String? {
        guard isOpen, showMatches, !counting, !proof.hasPreview, proof.rows.isEmpty,
              let r = draft.rules.first(where: { $0.kind == .domain && !$0.allValues.isEmpty }), let typed = r.allValues.last else { return nil }
        return DomainNearness.closest(to: typed, in: corpus.map(\.facts.fromAddress))?.domain
    }

    /// Which foot the inspector draws: the Re-run question, Create, Save, or the clean Close.
    var footKind: String { offer != nil ? "offer" : (isNew ? "create" : (dirty ? "apply" : "clean")) }
}
