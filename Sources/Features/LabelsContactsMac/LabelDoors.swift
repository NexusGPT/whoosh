import Foundation

/// The label keys' doors into the core (contract delta 10): Y takes the label in view off the focused thread, [ and ] do the
/// same then step to the next or the previous thread, ⇧Y takes every label off, V moves the thread to a label (labels it and
/// archives it, one undo). The list's key handlers (InboxList/LabelKeys.swift) call these; each is ONE core command, so Z undoes it.
@MainActor
enum LabelDoors {
    /// The label Y acts on: the open tab's label when a pinned owner label is open and the row carries it, else the row's first.
    static func inView(_ m: AppModel, _ row: InboxRow) -> MailLabel? {
        let all = m.core.labels()
        if let tab = all.first(where: { $0.id == m.tab && $0.kind == .owner }), row.labelNames.contains(tab.name) { return tab }
        guard let first = row.labelNames.first else { return nil }
        return all.first { $0.name == first }
    }

    /// `step` 0 stays on the thread, +1 goes to the next thread, -1 to the previous one, once the label is off.
    @discardableResult
    static func remove(_ m: AppModel, step: Int, all: Bool = false) -> Bool {
        guard let row = m.shownRow else { return false }
        let id = row.messageId
        let label = all ? nil : inView(m, row)
        guard all ? !row.labelNames.isEmpty : label != nil else {
            m.lastFailure = ActionFailed(accountName: "Whoosh", reasonWords: Words.text("This thread has no label to remove"), retryable: false)
            return false
        }
        let index = m.focusIndex
        let command: Command = all ? .removeAllLabels(messageIds: [id]) : .removeLabel(messageIds: [id], labelId: label?.id ?? "")
        guard m.perform(command) else { return false }
        if m.resultsShow { return true }   // the inbox focus is not the mail acted on; the result list follows (SearchMacModel.acted)
        refocus(m, from: index, gone: !m.rows.contains { $0.messageId == id }, id: id, by: step)
        return true
    }

    /// V's pick: the focused thread goes to `labelId` (labelled, then archived), as one action with one undo.
    @discardableResult
    static func move(_ m: AppModel, to labelId: String) -> Bool {
        guard let id = m.shownMessageId else { return false }
        let index = m.focusIndex
        guard m.perform(.moveToLabel(messageIds: [id], labelId: labelId)) else { return false }
        if m.overlay == moveOverlay { m.overlay = nil }
        if m.resultsShow { return true }
        refocus(m, from: index, gone: !m.rows.contains { $0.messageId == id }, id: id, by: 0)
        return true
    }

    /// The overlay V opens (Shortcuts' "move" command).
    static let moveOverlay = "move"

    /// After a removal the thread may have left the view (a pinned-label tab): the next thread is then already at `index`.
    private static func refocus(_ m: AppModel, from index: Int, gone: Bool, id: String, by delta: Int) {
        guard !m.rows.isEmpty else { return }
        if gone {
            m.focusIndex = min(max(0, index + (delta < 0 ? -1 : 0)), m.rows.count - 1)
        } else {
            m.focus(messageId: id)
            if delta != 0 { _ = m.moveFocus(delta) }
        }
    }

    /// Owner labels a thread can be moved to, in tree order (the move picker's rows).
    static func moveTargets(_ m: AppModel) -> [MailLabel] {
        m.core.labels().filter { $0.kind == .owner }
    }
}

/// The doors on the control socket: the move picker's rows, and one click per target that moves the focused thread there
/// (the click the picker's row makes). They act only while V's picker is open.
@MainActor
enum LabelDoorsSocket {
    static let ax = "mac-labels.move"

    static func rows() -> [String: (AppModel) throws -> [ControlRow]] {
        ["\(ax).targets": { m in
            guard m.overlay == LabelDoors.moveOverlay else { return [] }
            let st = LabelMoveState.shared
            return st.targets(m).enumerated().map { i, t in
                ["ax": "\(ax).\(t.label.id)", "id": t.label.id, "name": t.label.name, "path": t.path, "highlighted": i == st.highlight]
            }
        }]
    }

    static func clicks() -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [:]
        // Esc's door: the picker closes and nothing moves
        t["\(ax).close"] = { m in
            guard m.overlay == LabelDoors.moveOverlay else { return false }
            m.overlay = nil; return true
        }
        // the typed words narrow the picker (socket `type` then this click), the way the field does
        t["\(ax).filter"] = { m in
            guard m.overlay == LabelDoors.moveOverlay else { return false }
            LabelMoveState.shared.query = m.typed; m.typed = ""; LabelMoveState.shared.highlight = 0; return true
        }
        for l in LabelDoors.moveTargets(AppModel.shared) {
            t["\(ax).\(l.id)"] = { m in m.overlay == LabelDoors.moveOverlay && LabelDoors.move(m, to: l.id) }
        }
        return t
    }
}
