import Foundation

/// The macOS side of calendars, behind one protocol: read only, by construction (there is no method here that writes, and
/// `checks/settingscalendars-check.py` reads this folder for the calls that would).
///
/// Every `done` is called on the MAIN thread, once, and never waits on the caller: a view or a socket read never blocks on
/// EventKit ("The objective is <50ms for any operation"). Values handed out are plain Sendable structs.
protocol CalendarAccessing: AnyObject, Sendable {
    /// The system's answer now (a cheap class-level read, safe on any thread).
    var state: CalendarAccessState { get }
    /// Asks macOS for read access. The system shows its own window once; a second ask shows nothing and answers with the first answer.
    func requestAccess(_ done: @escaping @Sendable (CalendarAccessState) -> Void)
    /// The calendars macOS gives; empty unless access is allowed.
    func loadCalendars(_ done: @escaping @Sendable ([CalendarInfo]) -> Void)
    /// The events of the named calendars inside the window, oldest first; empty unless access is allowed.
    func loadEvents(in window: CalendarWindow, calendarIds: Set<String>, _ done: @escaping @Sendable ([CalendarEventInfo]) -> Void)
    /// The calendars or events changed outside Whoosh (the Calendar app, a sync, a switch in System Settings). Called on the main thread.
    func observeChanges(_ handler: @escaping @Sendable () -> Void)
}

/// Where the calendar list and the events go once read: the core's reply sources (`reply/sources` `calendars[]`, and the
/// excerpts of the enabled ones). The core's command for it is named in docs/lanes/settingscalendars-mac.md; until the lead
/// wires it, `RecordingReplyCalendarFeed` holds what would be handed over.
@MainActor
protocol ReplyCalendarFeed: AnyObject {
    /// The calendars the owner switched on as a source for replies (the core's `enabled` flags). Events of the others are never read.
    func enabledCalendarIds() -> Set<String>
    /// The calendar list as macOS gives it now (the core keeps the `enabled` flag of an id it knows, drops the ids that are gone).
    func replaceCalendars(_ rows: [CalendarInfo])
    /// The events of the enabled calendars inside the window; replaces what the core held for that window.
    /// Always called right after `replaceCalendars`; an implementation that sends one command sends it here.
    func replaceEvents(_ events: [CalendarEventInfo], window: CalendarWindow)
    /// What the other side holds now (the rig reads it to prove the hand over).
    func snapshot() -> CalendarFeedSnapshot
}

struct CalendarFeedSnapshot: Equatable {
    var calendars: [String]
    var enabled: [String]
    var eventCalendars: [String]
    var events: Int
}

/// The feed of this commit: it keeps what it was handed so the pane and the check can read it. It writes nowhere.
@MainActor
final class RecordingReplyCalendarFeed: ReplyCalendarFeed {
    static let shared = RecordingReplyCalendarFeed()
    private(set) var calendars: [CalendarInfo] = []
    private(set) var events: [CalendarEventInfo] = []
    /// The fixture core serves "Travail" on and "Personnel" off (Sources/CoreBridge/FixtureReplies.swift).
    var enabled: Set<String> = ["cal-work"]

    func enabledCalendarIds() -> Set<String> { enabled }
    func replaceCalendars(_ rows: [CalendarInfo]) { calendars = rows }
    func replaceEvents(_ events: [CalendarEventInfo], window: CalendarWindow) { self.events = events }
    func snapshot() -> CalendarFeedSnapshot {
        CalendarFeedSnapshot(calendars: calendars.map(\.id), enabled: enabled.sorted(), eventCalendars: Array(Set(events.map(\.calendarId))).sorted(), events: events.count)
    }
}
