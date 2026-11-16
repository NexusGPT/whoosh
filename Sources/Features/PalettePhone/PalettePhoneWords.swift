import SwiftUI

/// Every string the phone palette draws, through the catalogue (docs/lanes/i18n.md). English is the key.
/// Lines the Mac palette already owns are read from PaletteMacWords: one name for one thing.
enum PalettePhoneWords {
    static var placeholder: String { Words.text("Find a command") }
    static var suggested: String { Words.text("Suggested") }
    static var commandsHeader: String { PaletteMacWords.commandsHeader }
    static var goToHeader: String { PaletteMacWords.jumpHeader }
    static var close: String { PaletteMacWords.close }
    static var clear: String { Words.text("Clear") }
    static var kindMail: String { Words.text("Mail") }
    static var kindComposer: String { Words.text("Composer") }
    static var kindSearch: String { Words.text("Search") }
    static var kindMailbox: String { Words.text("Mailbox") }
    static var kindLabel: String { PaletteMacWords.labelKind }
    static var kindTab: String { PaletteMacWords.tabKind }
    static var settings: String { Words.text("Settings") }
    static var noMail: String { Words.text("Select a mail first") }
    static var nothingToUndo: String { Words.text("Nothing to undo") }
    static var emptyLine: String { Words.text("Try another word.") }
    static func noMatch(_ q: String) -> String { PaletteMacWords.noMatch(q) }
    static var nothingHere: String { PaletteMacWords.nothingHere }
}

/// The phone's own numbers, each private to this screen and listed in docs/lanes/palettephone.md for the ledger.
enum PalettePhoneMetrics {
    /// How much of the screen height the sheet takes. Under 1, so a strip of the inbox stays above it: a tap there closes the sheet.
    static let sheetFraction: CGFloat = 0.92
    /// A row is the Mailboxes screen's account row: tile and two lines inside one 56 pt target.
    static let rowHeight = PhoneMetrics.accountRow
    static let tile = PhoneMetrics.tileSmall
    /// The search field's height: Apple's 44 pt target.
    static let fieldHeight = PhoneMetrics.roundButton
    /// The separator starts under the title, past the tile.
    static let separatorInset = PhoneMetrics.gutter + PhoneMetrics.tileSmall + WhooshSpacing.s12
}
