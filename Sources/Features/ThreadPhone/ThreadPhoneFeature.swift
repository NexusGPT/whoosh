import Foundation

/// The iPhone thread's registrations: rows, clicks and keys register here, never in a hub.
/// ax ids follow `<surface>.<region>[.<element>...][.<index>]` (checks/threadphone-check.py).
public enum ThreadPhoneFeature: FeatureModule {
    public static let id = "threadphone"
    public static let surfaces: [String] = ["iphone-thread"]

    static var state: ThreadPhoneState { .shared }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "iphone-thread.open": { m in ThreadPhoneProbe.open(m) },
            "iphone-thread.subject": { m in ThreadPhoneProbe.subject(m) },
            "iphone-thread.participants": { m in ThreadPhoneProbe.participants(m) },
            "iphone-thread.account": { m in ThreadPhoneProbe.account(m) },
            "iphone-thread.messages": { m in ThreadPhoneProbe.messages(m) },
            "iphone-thread.code-pill": { m in ThreadPhoneProbe.codePill(m) },
            "iphone-thread.body": { m in ThreadPhoneProbe.body(m) },
            "iphone-thread.attachments": { m in ThreadPhoneProbe.attachments(m) },
            "iphone-thread.more": { m in ThreadPhoneProbe.more(m) },
            "iphone-thread.note": { m in ThreadPhoneProbe.note(m) },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        var c: [String: (AppModel) -> Bool] = [
            "iphone-thread.back": { m in state.close(m) },
            "iphone-thread.code-pill": { m in state.copyCode(m) },
            "iphone-thread.reply": { m in state.reply(m) },
            "iphone-thread.archive": { m in state.leave(.archive, m) },
            "iphone-thread.reload": { m in state.reload(m) },
            "iphone-thread.more": { _ in state.moreOpen.toggle(); return true },
            "iphone-thread.more.note": { m in menu { state.openNote(m); return true } },
            "iphone-thread.more.unread": { m in menu { state.markUnread(m) } },
            "iphone-thread.more.pin": { m in menu { state.togglePin(m) } },
            "iphone-thread.more.snooze": { m in menu { state.leave(.snooze, m) } },
            "iphone-thread.note.save": { m in state.noteOpen && state.saveNote(m) },
            "iphone-thread.note.close": { _ in guard state.noteOpen else { return false }; state.noteOpen = false; return true },
        ]
        let m = AppModel.shared
        // A tap on an inbox row opens its thread (the inbox registers the swipes and the menu, not the tap itself).
        for i in 0..<max(InboxPhoneState.shared.rows(m).count, 1) {
            c["iphone-inbox.list.row.\(i)"] = { m in state.open(row: i, in: m) }
        }
        // A tap on an earlier message unfolds it, a second tap folds it back.
        for i in 0..<max(state.thread(m)?.earlierCount ?? 0, 1) {
            c["iphone-thread.messages.\(i)"] = { m in
                guard let t = state.thread(m), i < t.earlierCount else { return false }
                state.toggleExpanded(i)
                return true
            }
        }
        return c
    }

    /// An item of the More panel works only while the panel is open, like the real tap.
    private static func menu(_ act: () -> Bool) -> Bool {
        guard state.moreOpen else { return false }
        state.moreOpen = false
        return act()
    }

    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        ["Escape": { m in
            guard m.overlay == ThreadPhoneState.overlayId else { return false }
            if state.noteOpen { state.noteOpen = false; return true }
            if state.moreOpen { state.moreOpen = false; return true }
            return state.close(m)
        }]
    }

    /// The note sheet owns its text while it is open: `type` writes into it.
    public static func typeText(_ text: String, _ m: AppModel) -> Bool {
        guard state.noteOpen else { return false }
        state.noteDraft += text
        return true
    }
}
