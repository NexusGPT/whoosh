import Foundation

/// What macOS says about this app's access to the owner's calendars. Three words, as the pane shows them: the system's
/// "restricted" and "write only" answers both mean the same thing for a reader of events (it cannot read), so they are `refused`.
enum CalendarAccessState: String, Sendable {
    case notAsked = "not-asked"
    case allowed
    case refused
}

/// One calendar as macOS lists it. `colourHex` is "#RRGGBB" (empty when the system gave none); `account` is the account's name
/// (iCloud, a work account), empty for a calendar that belongs to no account.
struct CalendarInfo: Equatable, Sendable, Identifiable {
    let id: String
    let name: String
    let account: String
    let colourHex: String
}

/// One event, as an excerpt a draft can read: title, start, end, attendees, place (docs/lanes/reply-sources.brief.md, part 3).
struct CalendarEventInfo: Equatable, Sendable, Identifiable {
    let id: String
    let calendarId: String
    let title: String
    let start: Date
    let end: Date
    let attendees: [String]
    let location: String
}

/// The days the replies may look at: the last 7 and the next 14 (the default of the reply-sources lane).
struct CalendarWindow: Equatable, Sendable {
    let start: Date
    let end: Date

    static let daysBack = 7
    static let daysAhead = 14

    static func around(_ now: Date) -> CalendarWindow {
        let day: TimeInterval = 86_400
        return CalendarWindow(start: now.addingTimeInterval(-Double(daysBack) * day), end: now.addingTimeInterval(Double(daysAhead) * day))
    }
}
