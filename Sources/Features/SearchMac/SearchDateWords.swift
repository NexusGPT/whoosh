import Foundation

/// The date words search accepts after "before": exactly the snooze picker's own lines (TimeInWords.options in
/// Sources/Features/Components/TimeInWords.swift, the one source of the words and of the times they resolve to), each in
/// English and in the app's language through the one catalogue (Words). A word resolves to the time the picker's line
/// resolves to, so "before tomorrow" reaches the rows the picker's tomorrow would reach.
enum SearchDateWords {
    struct Entry { let words: [String]; let when: Date }

    /// The picker's lines that carry a time ("Pick a date…" has none), in both spellings, lower-cased and split into words.
    static func entries(now: Date = Date(timeIntervalSince1970: Double(CoreHost.shared.clock.nowMs()) / 1000)) -> [Entry] {
        TimeInWords.options(now: now).compactMap { o -> [Entry]? in
            guard let when = o.when else { return nil }
            let spellings = Set([o.title, Words.text(o.title)].map { $0.lowercased() })
            return spellings.map { Entry(words: $0.split(whereSeparator: \.isWhitespace).map(String.init), when: when) }
        }.flatMap { $0 }
    }

    /// The longest run of tokens from `start` that names one picker line: the whole line, or its leading words when no
    /// other line starts the same way ("tomorrow" is "Tomorrow morning"). Returns the words used and the time.
    static func match(_ t: [String], from start: Int, entries: [Entry] = entries()) -> (count: Int, when: Date)? {
        let maxLen = entries.map(\.words.count).max() ?? 0
        for n in stride(from: min(maxLen, t.count - start), through: 1, by: -1) {
            let typed = t[start..<(start + n)].map { $0.lowercased() }
            let hits = entries.filter { $0.words.count >= n && Array($0.words.prefix(n)) == typed }
            let times = Set(hits.map(\.when))
            if times.count == 1, let when = times.first { return (n, when) }
        }
        return nil
    }

    /// "before" in the app's language too ("Avant %@" gives "avant"); only a word-before-value form is a keyword.
    static var beforeWords: Set<String> { keywords("Before %@", english: "before") }
    /// "after" the same way ("Après %@").
    static var afterWords: Set<String> { keywords("After %@", english: "after") }

    private static func keywords(_ format: String, english: String) -> Set<String> {
        let local = Words.text(format).lowercased()
        guard local.hasSuffix("%@") else { return [english] }
        let w = local.dropLast(2).trimmingCharacters(in: .whitespaces)
        return w.isEmpty || w.contains(" ") ? [english] : [english, w]
    }
}
