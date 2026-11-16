import Foundation

/// The fuzzy match. Pure: no state, no clock, so typing never waits on it.
enum PaletteMatch {
    /// Items whose text holds every query letter in order, best first (ties keep their order). Empty query: all.
    static func rank<T>(_ items: [T], _ query: String, text: (T) -> String) -> [T] {
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        guard !q.isEmpty else { return items }
        let scored = items.enumerated().compactMap { i, item in score(q, text(item).lowercased()).map { (i, $0, item) } }
        return scored.sorted { $0.1 != $1.1 ? $0.1 > $1.1 : $0.0 < $1.0 }.map(\.2)
    }

    /// Letters in order (a subsequence). Starting the text, starting a word and running on score more; a gap costs.
    static func score(_ q: String, _ text: String) -> Int? {
        let t = Array(text), qs = Array(q)
        var ti = 0, total = 0, prev = -2
        for ch in qs {
            guard ti < t.count, let at = t[ti...].firstIndex(of: ch) else { return nil }
            var s = 1
            if at == 0 { s += 12 } else if t[at - 1] == " " || t[at - 1] == "›" { s += 8 }
            if at == prev + 1 { s += 6 }
            total += s - min(at - ti, 4)
            prev = at; ti = at + 1
        }
        if text == q { total += 40 } else if text.hasPrefix(q) { total += 20 }
        return total
    }
}
