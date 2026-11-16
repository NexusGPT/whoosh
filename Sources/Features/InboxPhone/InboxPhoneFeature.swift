import Foundation

/// The iPhone inbox's registrations: rows, clicks and swipes register here, never in a hub.
/// ax ids follow `<surface>.<region>[.<element>...][.<index>]` (checks/inboxphone-check.py).
public enum InboxPhoneFeature: FeatureModule {
    public static let id = "inboxphone"
    public static let surfaces: [String] = ["iphone-inbox", "iphone-mailboxes"]

    static var state: InboxPhoneState { .shared }

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "iphone-inbox.list": { m in InboxPhoneProbe.list(m) },
            "iphone-inbox.tabs": { m in InboxPhoneProbe.tabs(m) },
            "iphone-inbox.title": { m in [["ax": "iphone-inbox.title", "text": state.title(m)]] },
            "iphone-inbox.undo": { _ in
                guard let u = state.undo, state.undoVisible() else { return [] }
                return [["ax": "iphone-inbox.undo", "text": "\(u.verb) · \(u.sender)", "button": "Undo"]]
            },
            "iphone-inbox.failure": { _ in state.failure.map { [["ax": "iphone-inbox.failure", "text": $0]] } ?? [] },
            "iphone-inbox.swipe-settings": { _ in
                [["ax": "iphone-inbox.swipe-settings", "leading": state.swipes.leading.rawValue, "trailing": state.swipes.trailing.rawValue]]
            },
            "iphone-inbox.refresh": { _ in [["ax": "iphone-inbox.refresh", "refreshed_at": state.refreshedAt.timeIntervalSince1970]] },
            "iphone-mailboxes.accounts": { m in InboxPhoneProbe.accounts(m) },
            "iphone-inbox.accounts": { m in InboxPhoneProbe.inboxAccounts(m) },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        var c: [String: (AppModel) -> Bool] = [
            "iphone-inbox.undo.button": { m in state.undoLast(m) },
            "iphone-inbox.list.refresh": { m in state.refresh(m); return true },
            "iphone-inbox.back": { m in PhoneRouter.shared.showMailboxes(); m.surface = "iphone-mailboxes"; return true },
            "iphone-inbox.select": { _ in state.selecting.toggle(); return true },
            "iphone-mailboxes.all": { m in state.picked = []; state.apply(m); PhoneRouter.shared.showInbox(); m.surface = "iphone-inbox"; return true },
            "iphone-mailboxes.show-selected": { m in
                guard !state.picked.isEmpty else { return false }
                state.apply(m); PhoneRouter.shared.showInbox(); m.surface = "iphone-inbox"; return true
            },
            // The doors the screens' notes ask for, each the real control's own call (PhoneRouter), live only where the real control is drawn:
            "iphone-inbox.compose": { _ in guard PhoneRouter.shared.onInbox else { return false }; PhoneRouter.shared.compose(); return true },
            "iphone-inbox.search": { _ in guard PhoneRouter.shared.onInbox else { return false }; PhoneRouter.shared.openSearch(); return true },
            "iphone-mailboxes.settings": { _ in guard PhoneRouter.shared.onMailboxes else { return false }; PhoneRouter.shared.openSettings(); return true },
            "iphone-mailboxes.labels": { _ in guard PhoneRouter.shared.onMailboxes else { return false }; PhoneRouter.shared.openLabelsContacts(); return true },
        ]
        let m = AppModel.shared
        for i in 0..<max(state.rows(m).count, 1) {
            let row = "iphone-inbox.list.row.\(i)"
            c["\(row).swipe-archive"] = { state.act(.archive, row: i, in: $0) }
            c["\(row).swipe-snooze"] = { state.act(.snooze, row: i, in: $0) }
            c["\(row).swipe-leading"] = { state.act(state.swipes.leading, row: i, in: $0) }
            c["\(row).swipe-trailing"] = { state.act(state.swipes.trailing, row: i, in: $0) }
            c["\(row).menu.pin"] = { state.act(.pin, row: i, in: $0) }
        }
        for (i, id) in m.tabIds.enumerated() {
            c["iphone-inbox.tabs.\(i)"] = { m in m.tab = id; m.focusIndex = 0; m.refresh(); return true }
        }
        for a in SwipeAction.allCases {
            c["iphone-inbox.swipe-settings.leading.\(a.rawValue)"] = { _ in state.swipes.leading = a; return true }
            c["iphone-inbox.swipe-settings.trailing.\(a.rawValue)"] = { _ in state.swipes.trailing = a; return true }
        }
        for a in PhoneAccounts.all(m) {
            c["iphone-mailboxes.account.\(a.id).tick"] = { _ in
                if state.picked.contains(a.id) { state.picked.remove(a.id) } else { state.picked.insert(a.id) }
                return true
            }
        }
        return c
    }
}

/// The `rows` answers: the same facts the screen draws, in the order it draws them.
@MainActor
enum InboxPhoneProbe {
    static func list(_ m: AppModel) -> [ControlRow] {
        let s = InboxPhoneState.shared
        return s.rows(m).enumerated().map { i, r in
            let a = PhoneAccounts.of(r.accountId, in: m)
            return ["ax": "iphone-inbox.list.row.\(i)", "id": r.messageId, "sender": r.senderName, "subject": r.subject,
                    "preview": r.preview, "unread": r.unread, "pinned": r.pinned, "account": r.accountId,
                    "mark_label": a.markLabel, "mark_colour": a.colour, "mark_icon": a.markIcon, "mark_attention": a.needsAttention,
                    "swipe_leading": s.swipes.leading.rawValue, "swipe_trailing": s.swipes.trailing.rawValue,
                    "selected": s.selected.contains(r.messageId)]
        }
    }

    static func tabs(_ m: AppModel) -> [ControlRow] {
        m.tabIds.enumerated().map { i, id in
            let t = m.counts.tabs.first { $0.tabId == id }
            return ["ax": "iphone-inbox.tabs.\(i)", "id": id, "title": m.host.fixture?.tabName(id) ?? id,
                    "unread": Int(t?.unread ?? 0), "on": id == m.tab]
        }
    }

    /// `iphone-inbox.accounts`: the accounts the phone draws (id, name, address, colour, group), in its order. No `note` field: the phone draws no push note,
    /// and the cases that read it say so (J7.1 no-push-account-says-so) until the owner gives its words.
    static func inboxAccounts(_ m: AppModel) -> [ControlRow] {
        PhoneAccounts.all(m).enumerated().map { i, a in
            let row: ControlRow = ["ax": "iphone-inbox.accounts.\(i)", "id": a.id, "name": a.name, "address": a.address, "colour": a.colour, "group": a.group ?? ""]
            return row
        }
    }

    static func accounts(_ m: AppModel) -> [ControlRow] {
        let s = InboxPhoneState.shared
        return PhoneAccounts.all(m).enumerated().map { i, a in
            ["ax": "iphone-mailboxes.account.\(i)", "id": a.id, "name": a.name, "address": a.address, "group": a.group ?? "",
             "mark_label": a.markLabel, "mark_colour": a.colour, "mark_icon": a.markIcon, "picked": s.picked.contains(a.id)]
        }
    }
}
