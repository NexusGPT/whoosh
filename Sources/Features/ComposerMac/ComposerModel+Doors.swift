import Foundation

/// Round 6 (doors-compose): the acts behind controls that had no click door of their own. The views call the same functions, so
/// a driven click and the owner's click take one path. Nothing here changes what a control does.
extension ComposerModel {
    /// The × on a recipient chip: `field` is `to`, `cc` or `bcc`, `i` the chip's place in its row.
    @discardableResult
    func removeChip(_ field: String, _ i: Int) -> Bool {
        guard isOpen, !isCounting else { return false }
        switch field {
        case "to": guard to.indices.contains(i) else { return false }; to.remove(at: i)
        case "cc": guard cc.indices.contains(i) else { return false }; cc.remove(at: i)
        case "bcc": guard bcc.indices.contains(i) else { return false }; bcc.remove(at: i)
        default: return false
        }
        return true
    }

    /// A row of the list under To: the address becomes a chip.
    @discardableResult
    func pickCompletion(_ i: Int, _ field: RecipientField = .to) -> Bool {
        let list = Array(offered(field).prefix(5))
        guard isOpen, list.indices.contains(i) else { return false }
        completionIndex = i; completionMoved = true
        return commitRecipient(field)
    }

    /// "Show quoted text" / "Hide quoted text" under the message.
    @discardableResult
    func toggleQuoted() -> Bool {
        guard isOpen, !quoted.isEmpty else { return false }
        quotedFolded.toggle()
        return true
    }

    /// "Pick a date…" of the Send later and Remind panels: the picker and its button open and close; a new opening starts a day ahead.
    @discardableResult
    func toggleDatePicker() -> Bool {
        datePicking.toggle()
        if datePicking { dateChosen = Date().addingTimeInterval(3600 * 24); dateRefusal = nil }
        return true
    }

    /// The Schedule button under the date picker: a time that has passed is refused in a sentence.
    @discardableResult
    func scheduleChosen(_ m: AppModel) -> Bool {
        if case .refused(let r) = TimeInWords.validate(dateChosen, now: Date()) { dateRefusal = r; return true }
        return schedule(dateChosen, m)
    }

    /// The "Set the reminder" button under the date picker.
    @discardableResult
    func setChosenReminder(_ m: AppModel) -> Bool {
        if case .refused(let r) = TimeInWords.validate(dateChosen, now: Date()) { dateRefusal = r; return true }
        return setReminder(dateChosen, m)
    }

    /// One time line of the Send later panel: schedules it, or says in a sentence why it cannot be chosen now.
    @discardableResult
    func scheduleOption(_ kind: SnoozeOption.Kind, _ m: AppModel) -> Bool {
        guard isOpen, sendLaterOpen, let o = TimeInWords.options(now: Date()).first(where: { $0.kind == kind }) else { return false }
        guard let d = o.when, o.refusal == nil else { dateRefusal = o.refusal; return o.refusal != nil }
        return schedule(d, m)
    }
}
