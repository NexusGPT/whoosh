import Foundation

/// What a chosen line does. A run that works closes the palette first, so the result meets the inbox, never the sheet;
/// a refusal keeps the sheet open and says why in one line. Every path ends on the same frame as the tap: nothing waits.
extension PalettePhoneModel {
    private enum Outcome { case done, screen(String), refused(String) }

    @discardableResult
    func run(_ item: PalettePhoneItem) -> Bool {
        switch perform(item) {
        case .done:
            record(item, "done", ""); close(); return true
        case .screen(let name):
            record(item, "done", ""); close(); app.overlay = name; return true
        case .refused(let words):
            record(item, "refused", words); line = words; return false
        }
    }

    private func record(_ item: PalettePhoneItem, _ outcome: String, _ words: String) {
        last = Run(id: item.id, title: item.title, outcome: outcome, words: words)
    }

    private func perform(_ item: PalettePhoneItem) -> Outcome {
        let inbox = InboxPhoneState.shared
        switch item.act {
        case .mail(let action):
            guard let id = app.focusedMessageId, let row = inbox.rows(app).firstIndex(where: { $0.messageId == id }) else {
                return .refused(PalettePhoneWords.noMail)
            }
            guard inbox.act(action, row: row, in: app) else { return .refused(PalettePhoneWords.noMail) }
            return inbox.failure.map(Outcome.refused) ?? .done
        case .undo:
            return inbox.undoLast(app) ? .done : .refused(inbox.failure ?? PalettePhoneWords.nothingToUndo)
        case .screen(let name, let needsMail):
            if needsMail && app.focusedMessageId == nil { return .refused(PalettePhoneWords.noMail) }
            return .screen(name)
        case .mailbox(let key):
            guard let entry = PalettePhoneMailboxes.all.first(where: { $0.key == key }) else { return .refused(item.title) }
            leaveFilters()
            if entry.box == nil { inbox.picked = []; inbox.apply(app) }
            app.mailbox = entry.box
            return .done
        case .account(let id):
            leaveFilters()
            app.mailbox = nil
            inbox.picked = [id]; inbox.apply(app)
            app.focusIndex = 0
            return .done
        case .tab(let id):
            leaveFilters()
            app.mailbox = nil
            app.tab = id; app.focusIndex = 0; app.refresh()
            return .done
        case .label(let id):
            app.smartFolderId = nil
            app.mailbox = nil
            LabelFilter.show(id, app)
            return .done
        }
    }

    /// A jump leaves the label and the smart folder the list was narrowed by.
    private func leaveFilters() {
        if app.labelId != nil { app.labelId = nil }
        if app.smartFolderId != nil { app.smartFolderId = nil }
    }
}
