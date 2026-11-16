import Foundation

/// The `rows` answers: the same facts the screen draws, in the order it draws them (the way InboxPhoneProbe does for the inbox).
/// ax ids follow `<surface>.<region>[.<element>...][.<index>]`; the same ids name the real controls.
@MainActor
enum SettingsPhoneProbe {
    static func nav(_ m: AppModel) -> [ControlRow] {
        let s = SettingsPhoneState.shared
        return [["ax": "iphone-settings.nav", "page": s.current?.rawValue ?? "home", "depth": s.path.count,
                 "open": m.overlay == "iphone-settings" || m.surface == "iphone-settings"]]
    }

    static func title(_ m: AppModel) -> [ControlRow] {
        [["ax": "iphone-settings.title", "text": SettingsPhoneWords.title, "language": Words.current]]
    }

    /// Home: the rows of the list, group by group.
    static func pages(_ m: AppModel) -> [ControlRow] {
        var i = 0
        var out: [ControlRow] = []
        for g in SettingsPhoneGroup.all {
            for p in g.pages {
                out.append(["ax": "iphone-settings.row.\(p.rawValue)", "id": p.rawValue, "group": g.id, "title": p.title,
                            "value": value(p, m), "attention": attention(p, m), "index": i])
                i += 1
            }
        }
        return out
    }

    static func summary(_ m: AppModel) -> [ControlRow] {
        let s = AccountsSession.shared
        let n = s.accounts.count
        let failing = s.accounts.filter { SettingsPhoneAccounts.failing($0, s) }.count
        return [["ax": "iphone-settings.summary", "count": n, "failing": failing, "marks": min(n, SettingsPhoneMetrics.markStack),
                 "line": n == 0 ? SettingsPhoneWords.accountsFooter : SettingsPhoneWords.accountsLead(count: n, failing: failing)]]
    }

    /// Sync: the line, the Mac's name, the age of the last sync in seconds, the reason, whether Retry shows.
    static func sync(_ m: AppModel) -> [ControlRow] {
        let r = SettingsPhoneSync.read(m)
        let now = SettingsPhoneSync.nowMs()
        return [["ax": "iphone-settings.sync", "line": SettingsPhoneSync.line(r), "device": r.device,
                 "age_s": r.age(nowMs: now).map { Double($0) } ?? -1.0, "time": SettingsPhoneSync.detail(r),
                 "state": r.phase.rawValue, "reason": r.reason, "retry": r.retry,
                 "retries": SettingsPhoneState.shared.retries]]
    }

    static func accounts(_ m: AppModel) -> [ControlRow] {
        let s = AccountsSession.shared
        return SettingsPhoneAccounts.ordered(s).enumerated().map { i, a in
            let mark = SettingsPhoneAccounts.mark(a, s)
            return ["ax": "iphone-settings.accounts.row.\(i)", "id": a.id, "name": a.name, "address": a.address, "group": a.group ?? "",
                    "state": SettingsPhoneAccounts.stateWords(a, s), "failing": SettingsPhoneAccounts.failing(a, s),
                    "mark_label": a.name, "mark_colour": a.colour, "mark_icon": mark.markIcon, "mark_attention": mark.needsAttention,
                    "order": i]
        }
    }

    /// Notifications: one row per account in the core's order (the fixture's order), each with the setting the Mac keeps and synced to the phone (J4.8).
    static func notifications(_ m: AppModel) -> [ControlRow] {
        let s = AccountsSession.shared
        s.reload()   // a pure read of the core: the list is the core's now, not what an earlier screen held
        return s.accounts.enumerated().map { i, a in
            let n = s.setting(a.id)
            return ["ax": "iphone-settings.notifications.\(i)", "id": a.id, "name": a.name, "enabled": n.enabled,
                    "delivery": n.delivery.map { "\($0)" } ?? "both", "order": i]
        }
    }

    /// Language: System first, then the catalogue's languages, each with whether it is the pick.
    static func language(_ m: AppModel) -> [ControlRow] {
        let picked = GeneralStore.shared.language
        return SettingsPhoneLanguagePage.choices.map { c in
            ["ax": "iphone-settings.language.\(c.code)", "code": c.code, "name": c.name, "on": c.code == picked]
        }
    }

    static func appearance(_ m: AppModel) -> [ControlRow] {
        let picked = GeneralStore.shared.appearance
        return GeneralStore.Appearance.allCases.map { a in
            ["ax": "iphone-settings.appearance.\(a.rawValue)", "id": a.rawValue, "title": SettingsPhoneWords.theme(a.rawValue), "on": a == picked]
        }
    }

    /// AI: which model writes and which sorts, its state, and the missing-setup mark's sentence ("" when nothing is missing).
    static func ai(_ m: AppModel) -> [ControlRow] {
        let status = AISetupStatus.shared
        return SettingsPhoneAI.needs.map { n in
            let x = status.status(n)
            return ["ax": "iphone-settings.ai.\(n.rawValue)", "need": n.rawValue, "title": SettingsPhoneAI.title(n), "name": SettingsPhoneAI.name(n, m),
                    "state": x.state.rawValue, "reason": x.reason, "missing": !x.usable, "mark": x.usable ? "" : x.words]
        }
    }

    private static func value(_ p: SettingsPhonePage, _ m: AppModel) -> String {
        switch p {
        case .accounts: return "\(AccountsSession.shared.accounts.count)"
        case .sync:
            let r = SettingsPhoneSync.read(m)
            return r.phase == .inSync || r.phase == .failing ? r.device : (r.phase == .waiting ? SettingsPhoneWords.notSyncedYet : "")
        case .ai: return SettingsPhoneAI.allReady() ? SettingsPhoneWords.ready : ""
        case .language: return SettingsPhoneLanguagePage.choices.first { $0.code == GeneralStore.shared.language }?.name ?? SettingsPhoneWords.system
        case .appearance: return SettingsPhoneWords.theme(GeneralStore.shared.appearance.rawValue)
        }
    }

    private static func attention(_ p: SettingsPhonePage, _ m: AppModel) -> Bool {
        let s = AccountsSession.shared
        switch p {
        case .sync: return SettingsPhoneSync.read(m).phase == .failing
        case .ai: return !SettingsPhoneAI.allReady()
        case .accounts: return s.accounts.contains { SettingsPhoneAccounts.failing($0, s) }
        default: return false
        }
    }
}
