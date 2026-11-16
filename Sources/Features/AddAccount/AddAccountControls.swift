import Foundation

/// One button of the sheet, as the footer draws it and as the `controls` rows list it: the same value, so a check reading the rows
/// reads exactly what is on the screen, key caps included.
struct AddAccountControl: Identifiable {
    enum Look { case primary, standard, plain }
    let id: String
    let title: String
    var key: String?
    var look: Look = .standard
    var enabled = true
    var busy = false
    /// the folding chevron after the title (Manual setup and its kin)
    var chevron = false
}

@MainActor enum AddAccountControls {
    /// The footer: Manual setup (or Add another) at the left; Cancel and the stage's default button at the right.
    static func footer(_ m: AddAccountModel) -> (left: [AddAccountControl], right: [AddAccountControl]) {
        let W = AddAccountWords.self
        let cancel = AddAccountControl(id: "cancel", title: W.cancel, key: "esc", enabled: !m.confirmingStop)
        let inline = m.style == .inline
        func right(_ main: AddAccountControl) -> [AddAccountControl] { inline ? [main] : [cancel, main] }
        switch m.stage {
        case .entry:
            return ([manual(W.manual)], right(AddAccountControl(id: "continue", title: W.cont, key: "↩", look: .primary, enabled: m.addressLooksValid)))
        case .detecting:
            return ([manual(W.skipManual)], right(AddAccountControl(id: "continue", title: W.finding, look: .primary, enabled: false, busy: true)))
        case .summary:
            let title = m.manualOpen ? W.hideManual : (m.hasProblem ? W.serverSettings : W.manual)
            let main = m.hasProblem
                ? AddAccountControl(id: "retry", title: W.retry, key: "↩", look: .primary)
                : AddAccountControl(id: "add", title: W.add, key: "↩", look: .primary, enabled: !m.manualOpen || m.draftComplete)
            return ([manual(title)], right(main))
        case .validating:
            var off = manual(W.manual); off.enabled = false
            return ([off], right(AddAccountControl(id: "add", title: W.adding, look: .primary, enabled: false, busy: true)))
        case .added:
            return ([AddAccountControl(id: "add-another", title: W.addAnother, key: "⌘N")], [AddAccountControl(id: "done", title: W.done, key: "↩", look: .primary)])
        }
    }

    static func manual(_ title: String) -> AddAccountControl { AddAccountControl(id: "manual", title: title, look: .plain, chevron: true) }

    /// The two buttons of the Esc question.
    static func stopRow() -> [AddAccountControl] {
        [AddAccountControl(id: "keep-checking", title: AddAccountWords.keepChecking, key: "↩"), AddAccountControl(id: "stop", title: AddAccountWords.stop, key: "esc")]
    }

    /// Every button on screen now, in reading order.
    static func visible(_ m: AddAccountModel) -> [AddAccountControl] {
        var out: [AddAccountControl] = []
        if m.stage != .added {
            out.append(AddAccountControl(id: "reveal", title: m.revealed ? AddAccountWords.hidePassword : AddAccountWords.showPassword, look: .plain, enabled: !m.isWorking))
        }
        if m.showsFoundLine { out.append(AddAccountControl(id: "details", title: AddAccountWords.details, look: .plain)) }
        let f = footer(m)
        out += f.left + f.right
        if m.confirmingStop { out += stopRow() }
        return out
    }
}

extension AddAccountModel {
    var isWorking: Bool { stage == .detecting || stage == .validating }
    /// The found line (servers on one grey line, with Details): found settings, nothing refused, Manual setup folded.
    var showsFoundLine: Bool { stage == .summary && mode == .detected && !manualOpen && !hasProblem && draftComplete }
}
