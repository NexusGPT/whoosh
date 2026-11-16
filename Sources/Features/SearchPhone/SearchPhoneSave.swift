import Foundation

/// Keep a search as a smart folder (JOURNEYS J2.5), and the meaning model's line and door (J2.1, J5.4).
/// The core saves the folder (`Command.saveSmartFolder`); its refusal is said in the name card in the core's own words.
@MainActor
extension SearchPhoneModel {
    /// Save: the name card opens over the field. Only a search with words, filters or both can be kept.
    @discardableResult
    public func startSaving() -> Bool {
        settle()
        guard active, phase == .results || phase == .none, !ruleWords.isEmpty else { return false }
        menu = nil
        naming = true; name = ""; fieldSays = ""; savedAs = nil
        return true
    }

    @discardableResult
    public func cancelNaming() -> Bool {
        guard naming else { return false }
        naming = false; name = ""; fieldSays = ""
        return true
    }

    /// Save in the name card. An empty name, a mailbox's name and a name already taken are refused in words; the card stays open.
    @discardableResult
    public func confirmName() -> Bool {
        guard naming else { return false }
        let n = name.trimmingCharacters(in: .whitespacesAndNewlines)
        if n.isEmpty { fieldSays = Words.text("Give the folder a name"); return true }
        if app.counts.mailboxes.keys.contains(where: { $0.caseInsensitiveCompare(n) == .orderedSame }) { fieldSays = Words.text("the name exists already"); return true }
        let rule = ruleWords
        do {
            _ = try app.core.run(command: .saveSmartFolder(name: n, ruleWords: rule))
        } catch CoreError.Invalid(_, let words) {
            fieldSays = words; return true
        } catch CoreError.Action(let failure) {
            fieldSays = failure.reasonWords; return true
        } catch {
            fieldSays = "\(error)"; return true
        }
        app.lastFailure = nil
        app.refresh()
        remember(rule)
        naming = false; name = ""; fieldSays = ""; savedAs = n
        return true
    }

    // MARK: the meaning model

    /// Where the meaning model stands: the AI setup's one read model, the same need the Mac's line reads.
    var meaning: AINeedStatus { AISetupStatus.shared.status(.meaning) }

    /// The line shows while the field holds words or filters and the model is not usable (missing, downloading or failed).
    var meaningLineVisible: Bool { active && !isIdleInput && !meaning.usable }

    /// The line, in words for the phone. A model that is simply missing says so for this phone; a download, a failure or a refusal
    /// speaks in the AI setup's own sentence.
    var meaningWords: String {
        let x = meaning
        if x.state == .missing && x.reason == "meaning-missing" { return Words.text("Search by meaning is off: the meaning model is not on this iPhone.") }
        return x.words
    }

    /// The door's label: "Download model", or "Retry" after a failure.
    var meaningButton: String { AISetupMarkText.button(.meaning, .consequence, meaning) }

    /// The door: the AI setup's own fix for the meaning need (`AISetupDoors.fix`), the call the Mac's line makes.
    @discardableResult
    public func pressDoor() -> Bool {
        guard meaningLineVisible, meaning.state != .working else { return false }
        doorTaps += 1
        AISetupDoors.fix(.meaning)
        return true
    }
}
