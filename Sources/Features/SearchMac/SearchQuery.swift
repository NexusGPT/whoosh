import Foundation

/// One filter of the bar, shown as a removable pill. It is made by Tab on a suggestion or by typing its colon form (`from:"Linea Supply"`,
/// `to:"Claire Martin"`, `subject:"quote"`, `attachment:"quote"`, `label:"Work"`, `before:2026-06-01`, `after:2026-06-01`, `has:attachment`, `from:@lorvan-hotels.example`);
/// both read back to the same pill, so a removal always takes out exactly its own words. Typed words never make one (SPEC A142, A143).
public struct SearchChip: Equatable, Sendable {
    public enum Kind: String, Sendable { case from, to, before, after, attachment, subject, attname, label
        /// SPEC A153: every mail whose sender's address is at this domain; `value` is the domain without its @, the colon form is `from:@<domain>`
        case domain }
    public let kind: Kind
    /// the words of the filter; for `before` and `after` the day as written in the colon form ("2026-06-01")
    public let value: String
    /// the words in the bar this chip came from, so removing the chip removes exactly them
    public let source: String
    /// a `before` or `after` pill: the time it resolves to (nil only for the phone's month words)
    public var when: Date? = nil

    /// What the pill says about its value: the day written in the app's language for a date, the words for the rest.
    public var valueTitle: String {
        if kind == .domain { return "@" + value }
        guard kind == .before || kind == .after, let when else { return value }
        return SearchQuery.dayTitle(when)
    }

    public var title: String {
        switch kind {
        case .from: return Words.text("From %@", valueTitle)
        case .to: return Words.text("To %@", valueTitle)
        case .before: return Words.text("Before %@", valueTitle)
        case .after: return Words.text("After %@", valueTitle)
        case .attachment: return Words.text("Has attachment")
        case .subject: return Words.text("Subject %@", valueTitle)
        case .attname: return Words.text("Attachment %@", valueTitle)
        case .label: return Words.text("Label %@", valueTitle)
        case .domain: return Words.text("From %@", valueTitle)
        }
    }

    private var dayWords: String { when.map { SearchQuery.day($0) } ?? value }

    /// The words that make this chip in the bar, in the form the chip is written back in (what Tab puts there) and sent to the engine in.
    public var canonical: String {
        switch kind {
        case .from: return "from:\"\(value)\""
        case .to: return "to:\"\(value)\""
        case .subject: return "subject:\"\(value)\""
        case .attname: return "attachment:\"\(value)\""
        case .label: return "label:\"\(value)\""
        case .domain: return "from:@\(value)"
        case .before: return when == nil ? "before " + value : "before:" + dayWords
        case .after: return when == nil ? "after " + value : "after:" + dayWords
        case .attachment: return "has:attachment"
        }
    }
}

/// The bar's text read as filters, an exact phrase and plain words. Case-blind; a filter word that is not
/// understood stays as plain words and is named in `notUnderstood`, never dropped.
public struct SearchQuery: Equatable, Sendable {
    public var chips: [SearchChip] = []
    public var words: [String] = []
    public var phrase: String?
    public var notUnderstood: [String] = []

    public var isEmpty: Bool { chips.isEmpty && words.isEmpty && phrase == nil }

    static let keywords: Set<String> = ["from", "before", "has"]
    static let english: Calendar = { var c = Calendar(identifier: .gregorian); c.locale = Locale(identifier: "en_US_POSIX"); return c }()
    static let months = english.monthSymbols.map { $0.lowercased() }
    static let shortMonths = english.shortMonthSymbols.map { $0.lowercased() }
    /// `from:"Linea Supply"` · `to:Claire` · `subject:quote` · `attachment:"price list"` · `label:Work` · `before:2026-06-01` · `after:june` · `has:attachment`
    static let colonForm = try! NSRegularExpression(pattern: "\\b(from|to|subject|attachment|label|before|after|has):(\"[^\"]*\"|[^\\s\"]+)", options: [.caseInsensitive])

    public static func monthIndex(_ word: String) -> Int? {
        let w = word.lowercased()
        return months.firstIndex(of: w) ?? shortMonths.firstIndex(of: w)
    }

    /// "2026-06-01": a day as the colon form writes it.
    static func day(_ d: Date) -> String {
        let c = english.dateComponents([.year, .month, .day], from: d)
        func pad(_ n: Int?, _ width: Int) -> String { let t = String(n ?? 0); return String(repeating: "0", count: max(0, width - t.count)) + t }
        return pad(c.year, 4) + "-" + pad(c.month, 2) + "-" + pad(c.day, 2)
    }

    /// The start of the day a colon form names: "2026-06-01", or a month's name (that month of this year, as the phone's "before June" has always meant).
    static func date(day s: String) -> Date? {
        let parts = s.split(separator: "-").compactMap { Int($0) }
        if parts.count == 3, s.split(separator: "-")[0].count == 4 {
            return english.date(from: DateComponents(year: parts[0], month: parts[1], day: parts[2]))
        }
        guard let m = monthIndex(s) else { return nil }
        var c = english.dateComponents([.year], from: Date()); c.month = m + 1; c.day = 1
        return english.date(from: c)
    }

    /// A day in the app's language ("1 Jun 2026").
    static func dayTitle(_ d: Date) -> String {
        let f = DateFormatter(); f.locale = Locale(identifier: Words.current); f.dateStyle = .medium; f.timeStyle = .none
        return f.string(from: d)
    }

    /// The field's reading of its text: only typed colon forms are pills. Typed words, "from", "has attachment", "before June", stay words
    /// until Tab on a suggestion turns them into one (SPEC A142, A143). `SearchQuery(_:)` also reads the phone's spoken filters.
    public static func typed(_ raw: String) -> SearchQuery { SearchQuery(raw, spokenFilters: false) }

    public init() {}

    /// `spokenFilters`: the words "has attachment", "before <month or date word>" and a lone snooze word also make their filter. The phone's filter
    /// words are written that way; the Mac field never reads them so (`typed`).
    public init(_ raw: String, spokenFilters: Bool = true) {
        var rest = String(raw.unicodeScalars.map { CharacterSet.controlCharacters.contains($0) ? " " : Character($0) })
        for m in Self.colonForm.matches(in: rest, range: NSRange(rest.startIndex..., in: rest)).reversed() {
            guard let all = Range(m.range, in: rest), let k = Range(m.range(at: 1), in: rest), let v = Range(m.range(at: 2), in: rest) else { continue }
            let value = rest[v].trimmingCharacters(in: CharacterSet(charactersIn: "\" "))
            let key = rest[k].lowercased()
            var kind: SearchChip.Kind = ["from": .from, "to": .to, "subject": .subject, "attachment": .attname, "label": .label, "before": .before, "after": .after, "has": .attachment][key] ?? .from
            var when: Date?
            var shown = value
            if kind == .before || kind == .after {
                guard let d = Self.date(day: value) else { continue }   // not a day: the words stay words
                when = d; shown = Self.day(d)
            } else if kind == .from, value.hasPrefix("@") {
                // `from:@lorvan-hotels.example`: the sender's domain (SPEC A153), a pill of its own, never a person
                shown = String(value.dropFirst()).trimmingCharacters(in: .whitespaces)
                guard !shown.isEmpty else { continue }
                kind = .domain
            } else if kind == .attachment {
                guard value.lowercased() == "attachment" || value.lowercased() == "attachments" else { continue }
                shown = ""
            }
            if !value.isEmpty { chips.insert(SearchChip(kind: kind, value: shown, source: String(rest[all]), when: when), at: 0) }
            rest.replaceSubrange(all, with: " ")
        }
        chips = chips.reduce(into: []) { acc, c in if !acc.contains(where: { $0.kind == c.kind }) { acc.append(c) } }
        let clean = rest
        if let open = clean.firstIndex(of: "\"") {
            let after = clean.index(after: open)
            if let close = clean[after...].firstIndex(of: "\"") {
                let p = clean[after..<close].trimmingCharacters(in: .whitespaces)
                if !p.isEmpty { phrase = p }
                rest = String(clean[..<open]) + " " + String(clean[clean.index(after: close)...])
            }
        }
        let tokens = rest.replacingOccurrences(of: "\"", with: " ").split(whereSeparator: \.isWhitespace).map(String.init)
        parse(tokens, spoken: spokenFilters)
        if spokenFilters { bareDateWord() }
    }

    /// A snooze word typed alone ("tomorrow", "next week", "later today", in the app's language) is the date filter `before <word>`: the
    /// field holds that word and nothing else of its kind (other filters may stand beside it). With any other word or a phrase it is plain text.
    private mutating func bareDateWord() {
        guard phrase == nil, !words.isEmpty, !chips.contains(where: { $0.kind == .before }),
              let d = SearchDateWords.match(words, from: 0), d.count == words.count else { return }
        chips.append(SearchChip(kind: .before, value: words.joined(separator: " "), source: words.joined(separator: " "), when: d.when))
        words = []
    }

    private mutating func parse(_ t: [String], spoken: Bool) {
        var i = 0
        while i < t.count {
            let w = t[i].lowercased()
            let next = i + 1 < t.count ? t[i + 1] : nil
            guard spoken else { words.append(t[i]); i += 1; continue }
            if w == "has", let n = next, n.lowercased() == "attachment" || n.lowercased() == "attachments" {
                add(SearchChip(kind: .attachment, value: "", source: "\(t[i]) \(n)")); i += 2; continue
            }
            if SearchDateWords.beforeWords.contains(w), next != nil {
                if let d = SearchDateWords.match(t, from: i + 1) {
                    let value = t[(i + 1)..<(i + 1 + d.count)].joined(separator: " ")
                    add(SearchChip(kind: .before, value: value, source: t[i...(i + d.count)].joined(separator: " "), when: d.when)); i += 1 + d.count; continue
                }
            }
            if w == "before", let n = next {
                if Self.monthIndex(n) != nil {
                    add(SearchChip(kind: .before, value: n, source: "\(t[i]) \(n)")); i += 2; continue
                }
                notUnderstood.append("\(t[i]) \(n)")
            }
            words.append(t[i]); i += 1
        }
    }

    private mutating func add(_ c: SearchChip) { if !chips.contains(where: { $0.kind == c.kind }) { chips.append(c) } }

    /// The bar's text with one chip's words taken out: removing a chip widens the search.
    public static func removing(_ chip: SearchChip, from text: String) -> String {
        guard let r = text.range(of: chip.source, options: .caseInsensitive) else { return text }
        var out = text; out.removeSubrange(r)
        return out.split(whereSeparator: \.isWhitespace).joined(separator: " ")
    }

    /// What the field shows after its chips: the text with every chip's words taken out. A trailing space stays,
    /// so a word just typed keeps its space.
    public static func draft(of text: String) -> String {
        let chips = SearchQuery.typed(text).chips
        guard !chips.isEmpty else { return text }
        var out = text
        for c in chips {
            if let r = out.range(of: c.source, options: .caseInsensitive) { out.removeSubrange(r) }
        }
        let trailing = out.last?.isWhitespace == true
        let joined = out.split(whereSeparator: \.isWhitespace).joined(separator: " ")
        return joined.isEmpty ? "" : joined + (trailing ? " " : "")
    }

    /// The bar's text for a chip set and a draft: the chips in their written-back form, then the draft.
    public static func text(chips: [SearchChip], draft: String) -> String {
        (chips.map(\.canonical) + (draft.isEmpty ? [] : [draft])).joined(separator: " ")
    }
}
