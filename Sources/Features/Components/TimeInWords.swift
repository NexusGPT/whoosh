import Foundation

/// The snooze rules of docs/CONSTANTS.md (SNOOZE_LATER_TODAY "+3 h rounded", SNOOZE_TOMORROW "08:00 next day",
/// SNOOZE_NEXT_WEEK "Monday 08:00 local"). The generated WhooshConstants skips these three because their cells are
/// words, so the numbers they hold are written here once and listed as a token request in docs/lanes/components.md.
public enum SnoozeRule {
    public static let laterTodayHours = 3
    public static let roundToMinutes = 30
    public static let morningHour = 8
    public static let mondayWeekday = 2          // Calendar weekday of Monday
}

/// One line of the snooze picker. `when` is nil only for the date picker line.
public struct SnoozeOption: Equatable, Sendable, Identifiable {
    public enum Kind: String, Sendable { case laterToday, tomorrowMorning, nextWeek, pickDate }
    public let kind: Kind
    public let title: String
    public let when: Date?
    /// Why this line cannot be chosen right now, in words; nil when it can.
    public let refusal: String?
    public var id: String { kind.rawValue }
}

/// Time in words: the three snooze words, and the refusal of any time that has already passed.
public enum TimeInWords {
    public enum Verdict: Equatable, Sendable {
        case accepted(Date)
        case refused(reason: String)
    }

    public static func options(now: Date, calendar: Calendar = .current) -> [SnoozeOption] {
        let step = Double(SnoozeRule.roundToMinutes * 60)
        let raw = now.addingTimeInterval(Double(SnoozeRule.laterTodayHours * 3600))
        let later = Date(timeIntervalSinceReferenceDate: (raw.timeIntervalSinceReferenceDate / step).rounded() * step)
        let laterRefusal = calendar.isDate(later, inSameDayAs: now) ? nil : "Later today has run out. Try tomorrow morning."
        let tomorrow = calendar.nextDate(after: calendar.startOfDay(for: now), matching: morning(), matchingPolicy: .nextTime)
        let nextDay = tomorrow.map { calendar.isDate($0, inSameDayAs: now) ? (calendar.date(byAdding: .day, value: 1, to: $0) ?? $0) : $0 }
        // "Next week" is the coming Monday, never today (the engine's rule, core/crates/actions/src/constants.rs next_week): on a Monday before the
        // morning hour the next Monday 08:00 is today's, an hour or less away and earlier than "Tomorrow morning" (seen 16 Nov, a Monday, 06:44).
        let comingMonday = calendar.nextDate(after: now, matching: DateComponents(hour: SnoozeRule.morningHour, minute: 0, weekday: SnoozeRule.mondayWeekday), matchingPolicy: .nextTime)
        let monday = comingMonday.map { calendar.isDate($0, inSameDayAs: now) ? (calendar.date(byAdding: .day, value: 7, to: $0) ?? $0) : $0 }
        return [
            SnoozeOption(kind: .laterToday, title: "Later today", when: later, refusal: laterRefusal),
            SnoozeOption(kind: .tomorrowMorning, title: "Tomorrow morning", when: nextDay, refusal: nil),
            SnoozeOption(kind: .nextWeek, title: "Next week", when: monday, refusal: nil),
            SnoozeOption(kind: .pickDate, title: "Pick a date…", when: nil, refusal: nil),
        ]
    }

    /// A chosen time is accepted only in the future; a past time is refused with a sentence that names the clock.
    public static func validate(_ date: Date, now: Date) -> Verdict {
        guard date > now else {
            return .refused(reason: "That time has already passed. Pick a time after \(now.formatted(date: .omitted, time: .shortened)).")
        }
        return .accepted(date)
    }

    /// "Today 17:30", "Tomorrow 08:00", "Mon 9 Nov 08:00".
    public static func describe(_ date: Date, now: Date, calendar: Calendar = .current) -> String {
        let time = date.formatted(date: .omitted, time: .shortened)
        if calendar.isDate(date, inSameDayAs: now) { return "Today \(time)" }
        if let t = calendar.date(byAdding: .day, value: 1, to: now), calendar.isDate(date, inSameDayAs: t) { return "Tomorrow \(time)" }
        return "\(date.formatted(.dateTime.weekday(.abbreviated).day().month(.abbreviated))) \(time)"
    }

    private static func morning() -> DateComponents { DateComponents(hour: SnoozeRule.morningHour, minute: 0) }
}
