import Foundation

/// Every sentence of the Agendas pane, in one place. A title is a noun that names the thing; one plain short line per setting.
enum CalendarWords {
    static var accessTitle: String { Words.text("Calendar access") }
    static var allowAccess: String { Words.text("Allow access") }
    static var chipNotAsked: String { Words.text("Not asked yet") }
    static var chipAllowed: String { Words.text("Allowed") }
    static var chipRefused: String { Words.text("Refused") }
    static var lineNotAsked: String { Words.text("Whoosh has not asked yet. It reads your events and never writes one.") }
    static var lineAsking: String { Words.text("Answer the macOS window.") }
    static var lineAllowed: String { Words.text("Whoosh reads your events on this Mac. It never writes one.") }
    static var lineRefused: String { Words.text("macOS refused access. Allow it in System Settings.") }
    static var lineNoPrompt: String { Words.text("macOS did not ask. Allow Calendars for Whoosh in System Settings.") }
    static var stepsTitle: String { Words.text("To allow access") }
    static var openSettings: String { Words.text("Open System Settings") }
    static var listTitle: String { Words.text("Calendars") }
    static var readOnly: String { Words.text("Read only") }
    static var reading: String { Words.text("Reading calendars") }
    static var emptyTitle: String { Words.text("No calendar on this Mac") }
    static var emptyLine: String { Words.text("Add one in the Calendar app and it appears here.") }
    static var onThisMac: String { Words.text("On this Mac") }
    static var untitled: String { Words.text("Untitled calendar") }
    static var openCalendars: String { Words.text("Open Calendars") }
    static var footnote: String { Words.text("Use a calendar as a source for replies in Settings › Replies › Sources.") }

    /// The three steps, in order. The number is drawn by the pane, not part of the sentence.
    static var steps: [String] {
        [Words.text("Open Privacy and Security in System Settings"), Words.text("Choose Calendars"), Words.text("Switch Whoosh on")]
    }

    static func line(for state: CalendarAccessState, asking: Bool, noPrompt: Bool = false) -> String {
        if asking { return lineAsking }
        if noPrompt && state == .notAsked { return lineNoPrompt }
        switch state {
        case .notAsked: return lineNotAsked
        case .allowed: return lineAllowed
        case .refused: return lineRefused
        }
    }

    static func chip(for state: CalendarAccessState) -> String {
        switch state {
        case .notAsked: return chipNotAsked
        case .allowed: return chipAllowed
        case .refused: return chipRefused
        }
    }
}
