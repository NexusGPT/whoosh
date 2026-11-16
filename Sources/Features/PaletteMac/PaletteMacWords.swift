import Foundation

/// Every string the palette draws, through the catalogue (docs/lanes/i18n.md). English is the key.
enum PaletteMacWords {
    static var commandsPlaceholder: String { Words.text("Type a command or jump to an account, tab or label") }
    static var accountsPlaceholder: String { Words.text("Move to which account") }
    static var commandsHeader: String { Words.text("Commands") }
    static var jumpHeader: String { Words.text("Go to") }
    static var accountsHeader: String { Words.text("Move to") }
    static var escKey: String { Words.text("esc") }
    static var tabKey: String { Words.text("Tab") }
    static var move: String { Words.text("Move") }
    static var run: String { Words.text("Run") }
    static var moveHere: String { Words.text("Move here") }
    static var goTo: String { Words.text("Go") }
    static var close: String { Words.text("Close") }
    static var nothingHere: String { Words.text("Nothing to run here") }
    static func empty(_ q: String) -> String { q.isEmpty ? nothingHere : noMatch(q) }
    static func noMatch(_ q: String) -> String { Words.text("No command matches “%@”", q) }
    static var moveTitle: String { Words.text("Move to another account") }
    static var generalSettings: String { Words.text("General settings") }
    /// vip-mark: the one command on the focused mail's sender, by whether that sender is a VIP now
    static var markVip: String { Words.text("Mark as VIP") }
    static var removeVip: String { Words.text("Remove from VIPs") }
    static var tabKind: String { Words.text("Tab") }
    static var labelKind: String { Words.text("Label") }
    static var accountKind: String { Words.text("Account") }
    static var settingsKind: String { Words.text("Settings") }
    static var accountsAxLabel: String { Words.text("Accounts") }
}
