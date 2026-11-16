import Foundation
import os

/// One line per step of connecting a calendar, in the unified log (`log show --predicate 'subsystem == "io.nexus.whoosh" AND category == "calendars"'`).
/// Counts and words only: never an event's title, place or attendee, never a calendar's name.
enum CalendarLog {
    private static let logger = Logger(subsystem: "io.nexus.whoosh", category: "calendars")

    static func line(_ words: String) { logger.notice("\(words, privacy: .public)") }
}
