import Foundation

/// One message's facts as the reader draws them. They come from `core.message(id)` (root delta 6): from name and
/// address, time, attachment names, the sign-in code and the tracker count. A message the core does not answer
/// for keeps only what the list row already says.
struct ReaderFact: Equatable {
    let id: String
    let from: String
    let address: String
    let time: String
    let preview: String
    let attachments: [String]
    let code: String?
    let trackers: Int
    /// The recipients as the core serves them ("Name <address>" or a bare address); Cc is not served (no field yet).
    let to: [String]
    /// Cc as the core serves it; nil while the contract has no cc field (contract delta 12). The header keeps its slot.
    let cc: [String]? = nil
    let receivedAtMs: Int64

    init(_ m: Message) {
        // a mail with no sender name shows its address as the name (owner's pictures 213 and 214: an empty sender line)
        id = m.id; from = m.senderName.isEmpty ? m.fromAddress : m.senderName; address = m.fromAddress; time = ReaderWords.when(m.receivedAtMs)
        preview = m.preview; attachments = m.attachmentNames; code = m.loginCode; trackers = Int(m.trackersBlocked)
        to = m.to; receivedAtMs = m.receivedAtMs
    }

    /// The message's own text without the quoted reply, from the core's body; nil where the core holds none.
    @MainActor
    static func wholeText(_ id: String) -> String? {
        guard let b = ReaderBodies.shared.body(AppModel.shared, id) else { return nil }
        guard let quote = b.quotedRanges.first else { return b.plainText }
        let bytes = Array(b.plainText.utf8)
        return String(decoding: bytes.prefix(Int(quote.start)), as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
    }

    init(row r: InboxRow) {
        id = r.messageId; from = r.senderName; address = r.senderAddress; time = ReaderWords.when(r.receivedAtMs)
        preview = r.preview; attachments = []; code = nil; trackers = 0
        to = []; receivedAtMs = r.receivedAtMs
    }
}

extension ReaderWords {
    /// "Today 09:12", "Mon 14:02", else "4 Nov 09:12", in the app's language: "Today" from the catalogue, the weekday
    /// and month names from the language's own calendar words (Words.current), 24-hour like the fixture.
    static func when(_ ms: Int64, now: Date = Date(), calendar: Calendar = .current) -> String {
        let d = Date(timeIntervalSince1970: Double(ms) / 1000)
        let f = DateFormatter()
        f.calendar = calendar; f.locale = Locale(identifier: Words.current); f.timeZone = calendar.timeZone
        let time = Self.clock(d, calendar)
        if calendar.isDate(d, inSameDayAs: now) { return Words.text("Today %@", default: "Today %@", time) }
        if let days = calendar.dateComponents([.day], from: d, to: now).day, days >= 0, days < 7 {
            f.setLocalizedDateFormatFromTemplate("EEE")
        } else {
            f.setLocalizedDateFormatFromTemplate("dMMM")
        }
        return f.string(from: d) + " " + time
    }

    /// The header's full date: "Sunday, November 8, 2026 at 17:46 CET" / "dimanche 8 novembre 2026 à 17:46 CET".
    /// The date in the language's own words, the joining word from the catalogue, the zone as its short name.
    static func fullDate(_ ms: Int64, calendar: Calendar = .current) -> String {
        let d = Date(timeIntervalSince1970: Double(ms) / 1000)
        let f = DateFormatter()
        f.calendar = calendar; f.locale = Locale(identifier: Words.current); f.timeZone = calendar.timeZone
        f.dateStyle = .full; f.timeStyle = .none
        let zone = DateFormatter()
        zone.locale = Locale(identifier: "en_GB"); zone.timeZone = calendar.timeZone; zone.dateFormat = "zzz"  // "CET": a name, not a word
        return Words.text("%1$@ at %2$@ %3$@", f.string(from: d), clock(d, calendar), zone.string(from: d))
    }

    private static func clock(_ d: Date, _ calendar: Calendar) -> String {
        let f = DateFormatter()
        f.calendar = calendar; f.locale = Locale(identifier: "en_GB"); f.timeZone = calendar.timeZone; f.dateFormat = "HH:mm"
        return f.string(from: d)
    }

    static func trackers(_ n: Int) -> String {
        n == 0 ? Words.text("No trackers") : n == 1 ? Words.text("1 tracker blocked") : Words.text("%@ trackers blocked", n)
    }
    static func codeLine(_ code: String) -> String { Words.text("Sign-in code") + " " + code }
}
