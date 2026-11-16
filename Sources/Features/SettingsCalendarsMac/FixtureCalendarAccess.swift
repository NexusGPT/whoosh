import Foundation

/// The calendars of the rig: the two of the design board (Travail, Personnel) and an access state the check sets with
/// `fixture calendar-access <not-asked|allowed|refused|will-refuse|no-prompt|no-answer>`. The rig never asks macOS for anything: it never reaches EventKit.
/// Answers come back at once, on the caller's thread (the main thread), so a socket read after a click sees the result.
final class FixtureCalendarAccess: CalendarAccessing, @unchecked Sendable {
    private let lock = NSLock()
    private var current: CalendarAccessState = .notAsked
    private var refuseNext = false
    private var silent = false   // `no-prompt`: macOS answers at once and shows nothing; the status stays not determined
    private var hang = false     // `no-answer`: macOS never calls back

    var state: CalendarAccessState { lock.withLock { current } }

    /// `will-refuse` is "not asked yet, and the next ask is refused". False for a word that is none of the four.
    func set(_ word: String) -> Bool {
        lock.withLock {
            switch word {
            case "not-asked": current = .notAsked; refuseNext = false; silent = false; hang = false
            case "allowed": current = .allowed; refuseNext = false; silent = false; hang = false
            case "refused": current = .refused; refuseNext = false; silent = false; hang = false
            case "will-refuse": current = .notAsked; refuseNext = true; silent = false; hang = false
            case "no-prompt": current = .notAsked; refuseNext = false; silent = true; hang = false
            case "no-answer": current = .notAsked; refuseNext = false; silent = false; hang = true
            default: return false
            }
            return true
        }
    }

    func reset() { _ = set("not-asked") }

    func requestAccess(_ done: @escaping @Sendable (CalendarAccessState) -> Void) {
        let (answer, never): (CalendarAccessState, Bool) = lock.withLock {
            if current == .notAsked && !silent && !hang { current = refuseNext ? .refused : .allowed }
            return (current, hang && current == .notAsked)
        }
        if never { return }
        done(answer)
    }

    func loadCalendars(_ done: @escaping @Sendable ([CalendarInfo]) -> Void) {
        done(state == .allowed ? Self.calendars : [])
    }

    func loadEvents(in window: CalendarWindow, calendarIds: Set<String>, _ done: @escaping @Sendable ([CalendarEventInfo]) -> Void) {
        guard state == .allowed else { done([]); return }
        let now = window.start.addingTimeInterval(Double(CalendarWindow.daysBack) * 86_400)   // the window starts 7 days before now
        done(Self.events(around: now).filter { calendarIds.contains($0.calendarId) })
    }

    func observeChanges(_ handler: @escaping @Sendable () -> Void) {}

    /// Ids match the fixture core's reply sources (Sources/CoreBridge/FixtureReplies.swift) so a switch there finds its calendar here.
    static let calendars: [CalendarInfo] = [
        CalendarInfo(id: "cal-work", name: "Travail", account: "assem@nexus-example.com", colourHex: WhooshCalendarColour.fixtureWork),
        CalendarInfo(id: "cal-personal", name: "Personnel", account: "assem@icloud.com", colourHex: WhooshCalendarColour.fixturePersonal),
    ]

    static func events(around now: Date) -> [CalendarEventInfo] {
        let h: TimeInterval = 3_600
        func ev(_ id: String, _ cal: String, _ title: String, _ hoursFromNow: Double, _ length: Double, _ who: [String], _ place: String) -> CalendarEventInfo {
            CalendarEventInfo(id: id, calendarId: cal, title: title, start: now.addingTimeInterval(hoursFromNow * h),
                              end: now.addingTimeInterval((hoursFromNow + length) * h), attendees: who, location: place)
        }
        return [
            ev("ev-1", "cal-work", "Point client", 20, 1, ["marie@oranta.example"], "Bureau"),
            ev("ev-2", "cal-work", "Appel fournisseur", 44, 0.5, [], ""),
            ev("ev-3", "cal-work", "Revue de contrat", 68, 1.5, ["paul@lorvan.example"], "Salle 2"),
            ev("ev-4", "cal-personal", "Dentiste", 30, 1, [], "Cabinet"),
        ]
    }
}
