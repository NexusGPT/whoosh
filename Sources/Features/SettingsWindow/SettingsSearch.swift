import Foundation

/// One setting the search can find: where it lives and the row to ring. The words are read at search time, so
/// the index speaks the language the app speaks now.
struct SettingsEntry: Identifiable, Equatable {
    let ax: String
    let pane: SettingsPane
    let title: String
    let section: String
    var id: String { ax }
    var path: String { section.isEmpty ? pane.title : pane.title + " › " + section }
}

enum SettingsSearch {
    private static let G = "mac-settings-general", A = "mac-settings-accounts", D = "mac-settings-ai", M = "mac-mcp-notes", C = "mac-settings-calendars"

    /// Every setting of every pane, in pane order. A row that is not here cannot be found, so the check walks this list.
    @MainActor static func all() -> [SettingsEntry] {
        func e(_ ax: String, _ pane: SettingsPane, _ title: String, _ section: String = "") -> SettingsEntry {
            SettingsEntry(ax: ax, pane: pane, title: title, section: section)
        }
        func t(_ key: String) -> String { Words.text(key) }
        var out: [SettingsEntry] = [
            e("\(G).appearance.language-row", .general, t("Language"), t("Language")),
            e("\(G).appearance.theme", .general, t("Appearance"), t("Appearance")),
            e("\(G).appearance.density", .general, t("List density"), t("Appearance")),
            e("\(G).appearance.motion", .general, t("Motion"), t("Appearance")),
            e("\(G).appearance.emblems", .general, t("Show account emblems"), t("Message list")),
            e("\(G).privacy.pictures", .general, t("Load pictures when a mail opens"), t("Privacy")),
            e("\(G).appearance.undo-send", .general, t("Undo send"), t("Message list")),
            e("\(G).pinned.section", .general, t("Pinned labels"), t("Pinned labels")),
            e("\(G).privacy.section", .general, t("Privacy"), t("Privacy")),
            e("\(G).sync.note", .general, t("Sync"), t("Sync")),
            e("\(A).add", .accounts, t("Add account"), t("Accounts")),
            e("\(A).move-to-iphone", .accounts, t("Move accounts to iPhone"), t("Accounts")),
            e("\(A).find", .accounts, t("Find an account"), t("Accounts")),
            e("\(C).access", .agendas, t("Calendar access"), t("Calendar access")),
            e("\(D).decisions.engine", .decisions, t("Decision model"), t("Decision model")),
        ]
        // The Calendars group is drawn only once macOS Calendar access is allowed (CalendarsPaneView.swift:14), so it is searchable only then.
        if let at = out.firstIndex(where: { $0.ax == "\(C).access" }), CalendarsModel.shared.state == .allowed {
            out.insert(e("\(C).list.section", .agendas, t("Calendars"), t("Calendars")), at: at + 1)
        }
        if AISettingsStore.shared.engine == .jev {
            out.append(e("\(D).decisions.jev.key", .decisions, t("Jev API key"), t("Decision model")))
        }
        for p in AISettingsStore.shared.pages {
            out.append(e("\(D).decisions.open.\(p.id)", .decisions, Words.text(p.title), t("What it files")))
        }
        out += [
            e("\(D).providers.section", .ai, t("Providers"), t("Providers")),
            e("\(D).speech", .ai, t("Speech"), t("Speech")),
            e("\(D).agents.new", .ai, t("Agents"), t("Agents")),
            e("\(G).notch.buttons.section", .notifications, t("Buttons on a new mail"), t("Notch")),
            e("\(G).notch.show", .notifications, t("Show unread mail only"), t("Notch")),
            e("\(G).notch.focus-filter", .notifications, t("Follow my Focus"), t("Notch")),
            e("\(G).notch.hidden", .notifications, t("Hide the notch when I share my screen"), t("Notch")),
            e("\(G).notch.key", .notifications, t("The notch's key"), t("Notch")),
            e("\(G).alerts.section", .notifications, t("How a new mail shows"), t("Notch")),
            e("\(G).alerts.stay", .notifications, t("It stays"), t("Notch")),
            e("\(G).alerts.sound", .notifications, t("Sound"), t("Notch")),
            e("\(G).alerts.pictures", .notifications, t("Full mail with its pictures"), t("Notch")),
            e("\(G).notifications.section", .notifications, t("Notify me for"), t("Notifications")),
            e("\(G).shortcuts.section", .shortcuts, t("Key map"), t("Shortcuts")),
            e("\(G).shortcuts.hints", .shortcuts, t("Show a key hint after a click"), t("Shortcuts")),
            e("\(M).server", .outside, t("Local server"), t("Outside apps")),
            e("\(M).apps", .outside, t("Apps"), t("Outside apps")),
        ]
        return out
    }

    /// Case and accent blind; a title that starts with the query comes first, then a title that contains it, then a path that does.
    @MainActor static func results(for query: String) -> [SettingsEntry] {
        let q = fold(query)
        guard !q.isEmpty else { return [] }
        var starts: [SettingsEntry] = [], inTitle: [SettingsEntry] = [], inPath: [SettingsEntry] = []
        for e in all() {
            let title = fold(e.title)
            if title.hasPrefix(q) || title.contains(" " + q) { starts.append(e) }
            else if title.contains(q) { inTitle.append(e) }
            else if fold(e.path).contains(q) { inPath.append(e) }
        }
        return starts + inTitle + inPath
    }

    static func fold(_ s: String) -> String {
        s.trimmingCharacters(in: .whitespaces).folding(options: [.caseInsensitive, .diacriticInsensitive], locale: nil)
    }
}
