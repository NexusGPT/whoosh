import Foundation
#if os(macOS)
import AppKit
#endif

/// What each control of the sheet does. One method per control; the buttons, the keys and the socket's clicks all call these.
extension AddAccountModel {
    /// Manual setup / Details / Server settings / Skip to manual setup: one control that unfolds the settings, or folds them again.
    @discardableResult
    public func toggleManual() -> Bool {
        if stage == .validating || stage == .added { return false }
        return manualOpen ? hideManual() : openManual()
    }

    @discardableResult
    func openManual() -> Bool {
        switch stage {
        case .entry:
            stage = .summary
        case .detecting:
            generation += 1; held = nil; steps = []; result = nil; draft = AddAccountDraft(); stage = .summary
        case .summary:
            break
        default: return false
        }
        mode = .manual; draft.source = .manual; draft.fillUsual(); manualOpen = true; notice = ""
        return true
    }

    @discardableResult
    func hideManual() -> Bool {
        guard manualOpen else { return false }
        manualOpen = false
        if result == nil && steps.isEmpty && !edited {
            stage = .entry; mode = .detected; draft = AddAccountDraft()
        } else if !edited, let r = result, case .found(let c) = r.outcome, !hasProblem {
            mode = .detected; draft = AddAccountDraft(c)
        }
        return true
    }

    /// Return, by what the sheet shows: Continue, Add or Try again, Keep checking, Done.
    @discardableResult
    public func primaryAction() -> Bool {
        switch stage {
        case .entry: return continueTapped()
        case .summary: return addTapped()
        case .validating: return confirmingStop ? keepChecking() : false
        case .added: return doneTapped()
        case .detecting: return false
        }
    }

    /// Cancel / Esc. A check in flight asks first; the second Esc is Stop.
    @discardableResult
    public func escapePressed() -> Bool {
        if stage == .validating {
            if confirmingStop { return stopChecking() }
            confirmingStop = true; return true
        }
        return cancelTapped()
    }

    @discardableResult
    func cancelTapped() -> Bool {
        if stage == .validating { confirmingStop = true; return true }
        close(); return true
    }

    /// Keep checking: the question goes away, the check carries on.
    @discardableResult
    func keepChecking() -> Bool {
        guard stage == .validating, confirmingStop else { return false }
        confirmingStop = false; return true
    }

    /// Stop: nothing is saved. The answer of the abandoned check is dropped and the found state comes back.
    @discardableResult
    func stopChecking() -> Bool {
        guard stage == .validating else { return false }
        stopFlag.cancel(); stopFlag = StopFlag()
        generation += 1; held = nil
        lines = []; confirmingStop = false; stage = .summary
        return true
    }

    /// Done: the sheet closes on the account just added, selected in the main window.
    @discardableResult
    public func doneTapped() -> Bool {
        guard stage == .added || !added.isEmpty else { return false }
        if let id = added.last?.id { AppModel.shared.accountFilter = id; AppModel.shared.refresh() }
        close(); return true
    }

    /// Ends the flow. The accounts added in this session are forgotten with the sheet; the next opening starts clean.
    public func close() {
        startOver(); added = []
        if AppModel.shared.surface == AddAccountWords.surface { AppModel.shared.surface = SurfaceSlot.macMain.id }
        onClose?()
    }

    @discardableResult
    func toggleReveal() -> Bool { revealed.toggle(); return true }

    /// Details under a failed added-account step: only while that step is drawn with the engine's words behind it.
    @discardableResult
    func toggleStepDetails() -> Bool {
        let s = AddAccountRows.addedStep(added.last)
        guard stage == .added, s.state == "failed", !s.details.isEmpty else { return false }
        stepDetailsOpen.toggle(); return true
    }

    /// A link in the sheet. Under the fixture core nothing is opened; the link is recorded so a check reads it.
    @discardableResult
    func open(link: String) -> Bool {
        guard let u = URL(string: link), u.scheme == "https" else { return false }
        openedLinks.append(link)
        if AppModel.shared.host.fixture == nil {
            #if os(macOS)
            NSWorkspace.shared.open(u)   // the system browser, never a web view
            #else
            openURL?(u)
            #endif
        }
        return true
    }
}

extension AddAccountModel {
    /// A98: Copy next to the admin-consent link. Under the fixture core the pasteboard is never touched; the link is recorded so a check reads it.
    @discardableResult
    func copy(link: String) -> Bool {
        guard !link.isEmpty else { return false }
        copiedLinks.append(link)
        #if os(macOS)
        if AppModel.shared.host.fixture == nil {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(link, forType: .string)
        }
        #endif
        return true
    }
}

extension AddAccountModel {
    /// A control by its id (the footer's, the stop row's, Details, the eye): the buttons, the keys and the socket's clicks all land here.
    @discardableResult
    func perform(_ id: String) -> Bool {
        switch id {
        case "continue": return continueTapped()
        case "add", "retry": return addTapped()
        case "cancel": return cancelTapped()
        case "manual", "details": return toggleManual()
        case "add-another": return addAnother()
        case "done": return doneTapped()
        case "keep-checking": return keepChecking()
        case "stop": return stopChecking()
        case "reveal": return toggleReveal()
        default: return false
        }
    }
}
