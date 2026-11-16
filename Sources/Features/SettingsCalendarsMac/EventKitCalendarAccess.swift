import EventKit
import Foundation

/// The real calendars of this Mac, read only. One serial queue owns the event store: nothing here runs on the main thread, and every
/// answer goes back to the main thread as plain values. Nothing in this file writes an event, a calendar or a reminder.
///
/// Info.plist needs `NSCalendarsFullAccessUsageDescription` (the words are in docs/lanes/settingscalendars-mac.md).
final class EventKitCalendarAccess: CalendarAccessing, @unchecked Sendable {
    private let queue = DispatchQueue(label: "whoosh.calendar-access", qos: .userInitiated)
    /// Made on the first use, on the queue: a launch that never opens the pane pays nothing.
    private lazy var store = EKEventStore()
    private var observer: NSObjectProtocol?

    var state: CalendarAccessState { Self.currentState() }

    static func currentState() -> CalendarAccessState {
        let s = EKEventStore.authorizationStatus(for: .event)
        if s == .fullAccess { return .allowed }
        if s == .notDetermined { return .notAsked }
        return .refused
    }

    func requestAccess(_ done: @escaping @Sendable (CalendarAccessState) -> Void) {
        queue.async { [self] in
            store.requestFullAccessToEvents { granted, error in
                let now = Self.currentState()
                CalendarLog.line("macOS answered the calendar request: granted \(granted), error \(error == nil ? "none" : "yes")")
                DispatchQueue.main.async { done(now) }
            }
        }
    }

    func loadCalendars(_ done: @escaping @Sendable ([CalendarInfo]) -> Void) {
        queue.async { [self] in
            guard Self.currentState() == .allowed else { DispatchQueue.main.async { done([]) }; return }
            store.refreshSourcesIfNecessary()   // right after a grant the store can still hold the empty list it had before
            let list = store.calendars(for: .event).map { c in
                CalendarInfo(id: c.calendarIdentifier, name: c.title, account: c.source?.title ?? "", colourHex: Self.hex(c.cgColor))
            }
            DispatchQueue.main.async { done(list) }
        }
    }

    func loadEvents(in window: CalendarWindow, calendarIds: Set<String>, _ done: @escaping @Sendable ([CalendarEventInfo]) -> Void) {
        queue.async { [self] in
            guard Self.currentState() == .allowed else { DispatchQueue.main.async { done([]) }; return }
            let calendars = store.calendars(for: .event).filter { calendarIds.contains($0.calendarIdentifier) }
            guard !calendars.isEmpty else { DispatchQueue.main.async { done([]) }; return }
            let predicate = store.predicateForEvents(withStart: window.start, end: window.end, calendars: calendars)
            let events: [CalendarEventInfo] = store.events(matching: predicate).compactMap { e in
                guard let start = e.startDate, let end = e.endDate else { return nil }
                let people = (e.attendees ?? []).map { $0.name ?? $0.url.absoluteString.replacingOccurrences(of: "mailto:", with: "") }
                // A repeating event's occurrences share one identifier: the start makes each one its own.
                return CalendarEventInfo(id: "\(e.eventIdentifier ?? e.calendar.calendarIdentifier)@\(Int(start.timeIntervalSince1970))",
                                         calendarId: e.calendar.calendarIdentifier, title: e.title ?? "", start: start, end: end,
                                         attendees: people, location: e.location ?? "")
            }.sorted { $0.start < $1.start }
            DispatchQueue.main.async { done(events) }
        }
    }

    func observeChanges(_ handler: @escaping @Sendable () -> Void) {
        queue.async { [self] in
            if let old = observer { NotificationCenter.default.removeObserver(old) }
            // The closure never reads the notification: it only tells the main thread that something moved.
            observer = NotificationCenter.default.addObserver(forName: .EKEventStoreChanged, object: store, queue: nil) { _ in
                DispatchQueue.main.async { handler() }
            }
        }
    }

    /// "#RRGGBB" in sRGB, or "" when the system's colour cannot be read.
    static func hex(_ colour: CGColor?) -> String {
        guard let colour, let space = CGColorSpace(name: CGColorSpace.sRGB),
              let rgb = colour.converted(to: space, intent: .defaultIntent, options: nil),
              let k = rgb.components, k.count >= 3 else { return "" }
        func byte(_ v: CGFloat) -> Int { max(0, min(255, Int((v * 255).rounded()))) }
        return String(format: "#%02X%02X%02X", byte(k[0]), byte(k[1]), byte(k[2]))
    }
}
