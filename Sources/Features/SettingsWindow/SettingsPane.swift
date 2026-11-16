import SwiftUI

/// The twelve panes of the Settings window, in sidebar order and in their four groups. Ten answer to ⌘1 to ⌘0; Calendars and Replies have no digit.
enum SettingsPane: String, CaseIterable, Identifiable, Hashable {
    case general, accounts, agendas, viewing, signatures, decisions, junk, ai, replies, notifications, shortcuts, outside
    var id: String { rawValue }

    enum Group: String, CaseIterable {
        case mail, sorting, ai, alerts
        var title: String {
            switch self {
            case .mail: return Words.text("Mail")
            case .sorting: return Words.text("Sorting")
            case .ai: return Words.text("AI")
            case .alerts: return Words.text("Alerts and keys")
            }
        }
        var panes: [SettingsPane] { SettingsPane.allCases.filter { $0.group == self } }
    }

    var group: Group {
        switch self {
        case .general, .accounts, .agendas, .viewing, .signatures: return .mail
        case .decisions, .junk: return .sorting
        case .ai, .replies: return .ai
        case .notifications, .shortcuts, .outside: return .alerts
        }
    }

    var title: String {
        switch self {
        case .general: return Words.text("General")
        case .accounts: return Words.text("Accounts")
        case .agendas: return Words.text("Calendars")
        case .viewing: return Words.text("Viewing")
        case .signatures: return Words.text("Signatures")
        case .decisions: return Words.text("Decisions")
        case .junk: return Words.text("Junk")
        case .ai: return Words.text("AI and agents")
        case .replies: return Words.text("Replies")
        case .notifications: return Words.text("Notifications")
        case .shortcuts: return Words.text("Shortcuts")
        case .outside: return Words.text("Outside apps")
        }
    }

    /// The one line under the pane's name in its header.
    var line: String {
        switch self {
        case .general: return Words.text("Language, look and the message list. Changes apply at once.")
        case .accounts: return Words.text("Your mail accounts, their health and how each looks in the list.")
        case .agendas: return Words.text("The calendars on this Mac that replies can read.")
        case .viewing: return Words.text("How a message reads: fonts, images and the reading pane.")
        case .signatures: return Words.text("What closes your mail, account by account.")
        case .decisions: return Words.text("Which model sorts your mail, and what it may file on its own.")
        case .junk: return Words.text("What counts as junk, and what happens to it.")
        case .ai: return Words.text("Providers, keys and the agents you allow.")
        case .replies: return Words.text("Drafts written for you, from your own sources.")
        case .notifications: return Words.text("Banners, sounds and the notch, account by account.")
        case .shortcuts: return Words.text("The keys you already know, and a hint when you reach for the mouse.")
        case .outside: return Words.text("Apps on this Mac that may read or act on your mail.")
        }
    }

    /// The white SF Symbol and the system hue of the coloured square.
    var symbol: String {
        switch self {
        case .general: return "gearshape"
        case .accounts: return "at"
        case .agendas: return "calendar"
        case .viewing: return "eye"
        case .signatures: return "signature"
        case .decisions: return "sparkles"
        case .junk: return "trash"
        case .ai: return "cpu"
        case .replies: return "arrowshape.turn.up.left.fill"   // the same symbol as RepliesIcon in SettingsRepliesMac
        case .notifications: return "bell.fill"
        case .shortcuts: return "keyboard"
        case .outside: return "powerplug"
        }
    }

    var hue: ThemeColor {
        switch self {
        case .general: return SettingsHue.gray
        case .accounts: return SettingsHue.blue
        case .agendas: return SettingsHue.red
        case .viewing: return SettingsHue.indigo
        case .signatures: return SettingsHue.orange
        case .decisions: return SettingsHue.purple
        case .junk: return SettingsHue.brown
        case .ai: return SettingsHue.pink
        case .replies: return SettingsHue.purple
        case .notifications: return SettingsHue.red
        case .shortcuts: return SettingsHue.gray
        case .outside: return SettingsHue.teal
        }
    }

    /// The panes with no digit: the digits of the others stay where they were.
    static let keyless: Set<SettingsPane> = [.agendas, .replies]
    /// ⌘1 ... ⌘9, then ⌘0, over the panes that have a key.
    var key: String {
        if Self.keyless.contains(self) { return "" }
        if self == .outside { return "0" }
        return String(Self.allCases.filter { !Self.keyless.contains($0) }.firstIndex(of: self)! + 1)
    }
    /// Empty for a pane without a key: the sidebar draws no key cap for it and no chord reaches it.
    var chord: String { key.isEmpty ? "" : "⌘" + key }

    /// The surface slot a pane answers to (the control socket, the palette and `shot` know the four slots).
    var slot: SurfaceSlot? {
        switch self {
        case .general: return .macSettingsGeneral
        case .accounts: return .macSettingsAccounts
        case .decisions, .ai: return .macSettingsAI
        case .outside: return .macSettingsMcp
        case .replies: return SurfaceSlot("mac-settings-replies")
        default: return nil
        }
    }

    static func pane(for slot: SurfaceSlot) -> SettingsPane? {
        switch slot {
        case .macSettingsGeneral: return .general
        case .macSettingsAccounts: return .accounts
        case .macSettingsAI: return .decisions
        case .macSettingsMcp: return .outside
        default: return slot.id == "mac-settings-replies" ? .replies : nil
        }
    }
}
