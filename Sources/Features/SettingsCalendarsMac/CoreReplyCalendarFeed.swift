import Foundation

/// The adapter between the macOS calendars and the core's reply sources: `SyncReplyCalendars` (contracts delta 21). The core keeps the
/// switch of a calendar it knows, starts a new one OFF, drops one that is gone, and keeps the events of the enabled calendars only.
/// It sends nothing when nothing changed. Nothing here waits: the command is local and returns at once.
@MainActor
final class CoreReplyCalendarFeed: ReplyCalendarFeed {
    private struct Sent: Equatable { var calendars: [CalendarInfo]; var events: [CalendarEventInfo] }
    private var calendars: [CalendarInfo] = []
    private var last: Sent?
    private var core: WhooshCore { AppModel.shared.core }

    func enabledCalendarIds() -> Set<String> {
        Set(RepliesRead.sources(RepliesRead.object(core.state(path: "reply/sources"))).calendars.filter(\.enabled).map(\.calendarId))
    }

    /// Keeps the list; the command goes out with the events in `replaceEvents` (the model always calls the two in this order).
    func replaceCalendars(_ rows: [CalendarInfo]) { calendars = rows }

    func replaceEvents(_ events: [CalendarEventInfo], window: CalendarWindow) {
        let now = Sent(calendars: calendars, events: events)
        guard now != last else { return }
        let infos = calendars.map {
            ReplyCalendarInfo(calendarId: $0.id, name: $0.name.isEmpty ? CalendarWords.untitled : $0.name,
                              accountLabel: $0.account.isEmpty ? CalendarWords.onThisMac : $0.account, colour: Self.colour($0.colourHex))
        }
        let sent = events.map { e in
            CalendarEvent(eventId: e.id, calendarId: e.calendarId, title: e.title, startMs: Self.ms(e.start), endMs: Self.ms(max(e.start, e.end)),
                          attendees: e.attendees, location: e.location.isEmpty ? nil : e.location)
        }
        do {
            _ = try core.run(command: .syncReplyCalendars(calendars: infos, windowStartMs: Self.ms(window.start), windowEndMs: Self.ms(window.end), events: sent))
            last = now
            CalendarLog.line("calendars sent to the core: \(infos.count), events sent: \(sent.count)")
            RepliesStore.shared.reloadSources()
        } catch {
            CalendarLog.line("the core refused the calendar sync: \(RepliesStore.words(error))")
        }
    }

    func snapshot() -> CalendarFeedSnapshot {
        let sources = RepliesRead.sources(RepliesRead.object(core.state(path: "reply/sources")))
        let events = RepliesRead.list(core.state(path: "reply/calendars/events"))
        return CalendarFeedSnapshot(calendars: sources.calendars.map(\.calendarId), enabled: sources.calendars.filter(\.enabled).map(\.calendarId).sorted(),
                                    eventCalendars: Array(Set(events.compactMap { $0["calendar_id"] as? String })).sorted(), events: events.count)
    }

    /// Forgets what was sent (the rig's reset).
    func forget() { last = nil; calendars = [] }

    /// The core refuses a colour that is not "#RRGGBB" for the whole sync: a calendar with none gets the system grey.
    static func colour(_ hex: String) -> String { hex.count == 7 && hex.hasPrefix("#") && EmblemRGB(hex: hex) != nil ? hex : WhooshCalendarColour.noColour }
    private static func ms(_ d: Date) -> Int64 { Int64((d.timeIntervalSince1970 * 1000).rounded()) }
}
