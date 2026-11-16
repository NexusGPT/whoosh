import Foundation
import Observation

/// How far the meaning index has come, for the two screens that say it: the main window's status line ("indexing 10 %") and the
/// card "Search" in Settings > AI and agents ("Indexing your mail: 1,240 of 12,000"). Board: Index I2, the owner's pick.
///
/// One source. The builder announces progress (`MeaningIndexBuilder.onUpdate`, at most once per EMBED_NOTIFY_MS while it works, and once
/// at each start, pause and stop); `MeaningSearch` turns that into the notification `IndexProgress.didChange` carrying the builder's
/// `Status`, read on the builder's own queue so the main thread never takes the ranker's lock. `IndexProgressModel` listens on the main
/// queue and both screens read it. Nothing polls and nothing runs while the index is idle.
public enum IndexProgress {
    /// Posted by `MeaningSearch` for each builder announcement; `userInfo["status"]` is a `MeaningIndexBuilder.Status`.
    public static let didChange = Notification.Name("whoosh.meaning-index.progress")
    public static let statusKey = "status"

    /// The index is being built: ids wait for a step.
    public static func isBuilding(_ s: MeaningIndexBuilder.Status) -> Bool { s.queued > 0 }
    /// Waiting for memory to free up (only while something is queued).
    public static func isPaused(_ s: MeaningIndexBuilder.Status) -> Bool { s.queued > 0 && s.pausedForMemory }
    /// Mails stored plus mails still waiting: the share's whole.
    public static func total(_ s: MeaningIndexBuilder.Status) -> Int { s.indexed + s.queued }
    /// The share done, 0...1 (nil when there is nothing to build).
    public static func fraction(_ s: MeaningIndexBuilder.Status) -> Double? {
        let t = total(s)
        return t > 0 && isBuilding(s) ? Double(s.indexed) / Double(t) : nil
    }

    /// A count in the chosen language's number format.
    static func number(_ n: Int) -> String { n.formatted(.number.locale(Locale(identifier: Words.current))) }

    /// The part after the middle dot of the status line: "indexing 10 %", "indexing paused", or nil when nothing is queued
    /// (the line is then exactly what it was).
    static func statusLinePart(_ s: MeaningIndexBuilder.Status) -> String? {
        guard isBuilding(s) else { return nil }
        if isPaused(s) { return Words.text("indexing paused") }
        let percent = Int((fraction(s) ?? 0) * 100)   // rounded down: never says 100 % before it is done
        return Words.text("indexing %@ %", number(percent))
    }

    /// The quiet line under "Search by meaning", with the share for its 4 pt bar (nil when no bar is drawn).
    struct SettingsLine: Equatable {
        var text: String
        var fraction: Double?
    }

    /// The Settings line for a state; nil when there is nothing to say (no mails, nothing indexed, nothing failed).
    static func settingsLine(_ s: MeaningIndexBuilder.Status) -> SettingsLine? {
        if isBuilding(s) {
            if isPaused(s) {
                return SettingsLine(text: Words.text("Paused at %@ of %@: memory is full. It resumes by itself.", number(s.indexed), number(total(s))),
                                    fraction: fraction(s))
            }
            return SettingsLine(text: Words.text("Indexing your mail: %@ of %@", number(s.indexed), number(total(s))), fraction: fraction(s))
        }
        if s.failed > 0 {
            let whole = s.indexed + s.failed
            let miss = s.failed == 1 ? Words.text("1 mail could not be read.") : Words.text("%@ mails could not be read.", number(s.failed))
            return SettingsLine(text: Words.text("%@ of %@ indexed.", number(s.indexed), number(whole)) + " " + miss, fraction: nil)
        }
        guard s.indexed > 0 else { return nil }
        return SettingsLine(text: s.indexed == 1 ? Words.text("1 mail indexed.") : Words.text("%@ mails indexed.", number(s.indexed)), fraction: nil)
    }
}

/// The ONE observable the status line and the Settings card read. It moves only when an announcement arrives (and only if the
/// figures changed), so an idle index costs nothing and a socket read of it changes nothing on screen.
@MainActor @Observable
final class IndexProgressModel {
    static let shared = IndexProgressModel()

    private(set) var status: MeaningIndexBuilder.Status
    @ObservationIgnored private var listener: NSObjectProtocol?

    init(center: NotificationCenter = .default, status: MeaningIndexBuilder.Status = .init()) {
        self.status = status
        listener = center.addObserver(forName: IndexProgress.didChange, object: nil, queue: .main) { [weak self] note in
            let s = note.userInfo?[IndexProgress.statusKey] as? MeaningIndexBuilder.Status   // a plain value crosses, not the notification
            MainActor.assumeIsolated { if let s { self?.take(s) } }
        }
    }

    /// An announcement: take its figures; the same figures change nothing (no redraw).
    func take(_ s: MeaningIndexBuilder.Status) { if s != status { status = s } }
}
