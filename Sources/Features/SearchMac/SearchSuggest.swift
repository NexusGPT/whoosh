import Foundation

/// One row of the dropdown under the field. `group` lets the view draw a hairline between groups.
public struct SearchSuggestion: Equatable, Identifiable, Sendable {
    /// `from` and `to` are the two rows of a person, `filter` the dates and "has attachment" the typed words name
    public enum Kind: String, Sendable { case search, from, to, label, folder, subject, attachment, filter, recent
        /// SPEC A153: "From domain <domain>", offered right under "Search all mail for …"; Tab makes the pill "From @<domain>"
        case domain }
    public let id: String
    public let kind: Kind
    public let title: String
    /// the bold span of the title (the typed letters the row matched), as a character range of `title`
    public let bold: Range<Int>?
    /// the address at the right of a person's row
    public let detail: String
    public let accountId: String?
    /// what Tab or Enter makes of the row: a pill to add (and the typed words it replaces), words to run, or nothing
    public let chip: SearchChip?
    public let replaces: String?
    public let words: String?
    public let group: Int
}

/// One person the engine knows, as the suggestions need them: how many mails came from them and how many were sent to them.
public struct PersonSuggestion: Equatable, Sendable, Decodable {
    public let name: String
    public let address: String
    public let fromCount: Int
    public let toCount: Int
    public init(name: String, address: String, fromCount: Int, toCount: Int) { self.name = name; self.address = address; self.fromCount = fromCount; self.toCount = toCount }
    enum CodingKeys: String, CodingKey { case name, address, fromCount = "from_count", toCount = "to_count" }
}

/// The people the field offers, asked of the engine by the first letters typed (SPEC A143: every address of the mail received and sent, not the rows on screen).
/// The core answers the state path `people/prefix/<letters>` with a JSON array, best first, at most a handful:
/// `[{"name":"Claire Martin","address":"claire.martin@lorvan.example","from_count":24,"to_count":11}]`.
/// `<letters>` is what the owner typed last, lower case. A person matches when the letters start their name, a word of their name or their address.
/// A person with `from_count` 0 is offered only as To; with `to_count` 0 only as From. No answer (nil, not JSON) is no people.
public enum PeopleRead {
    public static func path(_ letters: String) -> String { "people/prefix/" + letters }

    public static func people(core: WhooshCore, prefix: String) -> [PersonSuggestion] {
        let letters = prefix.trimmingCharacters(in: .whitespaces).lowercased()
        guard !letters.isEmpty, let json = core.state(path: path(letters)), let d = json.data(using: .utf8),
              let list = try? JSONDecoder().decode([PersonSuggestion].self, from: d) else { return [] }
        return list
    }
}

/// The sender domains the field offers, asked of the engine by the letters typed last (`senderDomains(prefix:)`, SPEC A153): the domains that start with
/// them, most mail first. "@" before the letters is the owner's own way to say it is a domain; the engine is asked the letters alone.
public enum DomainRead {
    public static func letters(_ typed: String) -> String {
        String(typed.trimmingCharacters(in: .whitespaces).drop(while: { $0 == "@" })).lowercased()
    }

    public static func domains(core: WhooshCore, prefix: String) -> [SenderDomain] {
        let l = letters(prefix)
        return l.isEmpty ? [] : core.senderDomains(prefix: l)
    }

    /// "3 people", "1 person": what stands at the right of the row.
    static func peopleWords(_ n: Int) -> String { n == 1 ? Words.text("1 person") : Words.text("%lld people", n) }
}

/// The suggestions for the field's draft, from one engine read for people and local data otherwise (no network). Order: Search all mail, the people
/// as From and To, a subject, an attachment name, the filter words typed (has attachment, before, after), labels and smart folders, a gap, recent searches.
public enum SearchSuggest {
    public struct Source {
        var people: [PersonSuggestion]
        var labels: [String]
        var folders: [(name: String, rule: String)]
        var recents: [String]
        /// the subject of a mail on screen that holds the typed words, if one does
        var subject: (String) -> String? = { _ in nil }
        /// the sender domains the last typed letters start (SPEC A153), best first
        var domains: [SenderDomain] = []
    }

    /// A typed word of 3 letters or more matches a word of a name that starts with it, compared without accents and case: "supp" and "sup"
    /// match "Supply", "supplier" does not (the suggestion must contain what was typed, never only a shared start).
    static func startsWith(_ word: String, _ typed: String) -> Bool {
        let opts: String.CompareOptions = [.caseInsensitive, .diacriticInsensitive]
        return !typed.isEmpty && word.folding(options: opts, locale: nil).hasPrefix(typed.folding(options: opts, locale: nil))
    }

    /// The span of `name` a typed word matches (its word start to the typed word's end), and the typed word.
    static func match(_ name: String, typed: [String]) -> (range: Range<Int>, word: String)? {
        var offset = 0
        for part in name.split(separator: " ", omittingEmptySubsequences: false) {
            let w = String(part)
            for t in typed where t.count >= 3 && startsWith(w, t) {
                return (offset..<(offset + min(t.count, w.count)), t)
            }
            offset += w.count + 1
        }
        return nil
    }

    /// The span of `name` the typed letters start (a word of it), as a character range, for the bold letters.
    static func boldRange(_ name: String, typed: String) -> Range<Int>? {
        guard !typed.isEmpty else { return nil }
        var offset = 0
        for part in name.split(separator: " ", omittingEmptySubsequences: false) {
            let w = String(part)
            if startsWith(w, typed) { return offset..<(offset + min(typed.count, w.count)) }
            offset += w.count + 1
        }
        return nil
    }

    /// The pills the typed words name: "has attachment", "before june", "after tomorrow". Each is offered, never made by the typing alone.
    /// `phrase` is the typed words the pill replaces when it is taken.
    static func filterRows(_ words: [String]) -> [(phrase: String, chip: SearchChip)] {
        var out: [(phrase: String, chip: SearchChip)] = []
        let low = words.map { $0.lowercased() }
        let has = Words.text("Has attachment").lowercased().split(whereSeparator: \.isWhitespace).map(String.init)
        for i in low.indices {
            if i + 1 < low.count, low[i] == "has", low[i + 1] == "attachment" || low[i + 1] == "attachments" {
                out.append((words[i...(i + 1)].joined(separator: " "), SearchChip(kind: .attachment, value: "", source: "")))
            } else if has.count > 1, i + has.count <= low.count, Array(low[i..<(i + has.count)]) == has {
                out.append((words[i..<(i + has.count)].joined(separator: " "), SearchChip(kind: .attachment, value: "", source: "")))
            }
            for (kind, keys) in [(SearchChip.Kind.before, SearchDateWords.beforeWords), (.after, SearchDateWords.afterWords)] where keys.contains(low[i]) && i + 1 < low.count {
                var when: Date?, used = 0
                let cal = SearchQuery.english
                func monthStart(_ m: Int) -> Date? { SearchQuery.date(day: SearchQuery.months[m]) }
                // "June": the month; "3 june" or "june 3": that day. `before:` names the day itself; `after:` is inclusive, so "after June" is the next month's
                // first day and "after 3 june" the 4th (the engine keeps mail ON or after the day's midnight)
                if let m = SearchQuery.monthIndex(low[i + 1]), i + 2 < low.count, let n = Int(low[i + 2]), (1...31).contains(n) {
                    when = monthStart(m).flatMap { cal.date(byAdding: .day, value: n - 1 + (kind == .after ? 1 : 0), to: $0) }; used = 2
                } else if let n = Int(low[i + 1]), (1...31).contains(n), i + 2 < low.count, let m = SearchQuery.monthIndex(low[i + 2]) {
                    when = monthStart(m).flatMap { cal.date(byAdding: .day, value: n - 1 + (kind == .after ? 1 : 0), to: $0) }; used = 2
                } else if let m = SearchQuery.monthIndex(low[i + 1]) {
                    when = monthStart(m).flatMap { kind == .after ? cal.date(byAdding: .month, value: 1, to: $0) : $0 }; used = 1
                } else if let d = SearchDateWords.match(words, from: i + 1) { when = d.when; used = d.count }
                if let when { out.append((words[i...(i + used)].joined(separator: " "), SearchChip(kind: kind, value: SearchQuery.day(when), source: "", when: when))) }
            }
        }
        return out
    }

    /// The words in the field stay words: a sender, a recipient, a subject, an attachment name, a date or "has attachment" becomes a pill only when
    /// the owner takes its row with Tab (SPEC A142, A143). People come from the engine (`Source.people`).
    public static func build(draft: String, chips: [SearchChip], source: Source, recentMax: Int) -> [SearchSuggestion] {
        let typedWords = draft.split(whereSeparator: \.isWhitespace).map(String.init)
        let plain = draft.trimmingCharacters(in: .whitespaces)
        var out: [SearchSuggestion] = []
        func add(_ k: SearchSuggestion.Kind, _ id: String, _ title: String, bold: Range<Int>? = nil, detail: String = "", account: String? = nil,
                 chip: SearchChip? = nil, replaces: String? = nil, words: String? = nil, group: Int) {
            out.append(SearchSuggestion(id: id, kind: k, title: title, bold: bold, detail: detail, accountId: account, chip: chip, replaces: replaces, words: words, group: group))
        }
        // "Search all mail for “<the typed words>”": only words typed in the field. A pill alone (Tab on "From domain") offers no such row: its own name read
        // back as search words ("Search all mail for “From @linea…”", main107 d1ffe36c clause 1) said something the search does not do; Return still runs the
        // pill (SearchMacActs.enter, the field's launch). Round 19.
        if !plain.isEmpty {
            add(.search, "search", plain, words: SearchQuery.text(chips: chips, draft: plain), group: 0)
        }
        guard let last = typedWords.last else {
            for (i, r) in source.recents.prefix(recentMax).enumerated() { add(.recent, "recent.\(i)", r, words: r, group: 2) }
            return out
        }
        let previous = typedWords.count > 1 ? typedWords[typedWords.count - 2].lowercased() : ""
        // SPEC A153 (the drawing's second row group, right under "Search all mail for …"): a domain the typed letters start; Tab makes the pill
        let typedLetters = DomainRead.letters(last)
        for d in source.domains.prefix(2) where !typedLetters.isEmpty && d.domain.lowercased().hasPrefix(typedLetters) {
            add(.domain, "domain.\(d.domain)", d.domain, bold: 0..<typedLetters.count, detail: DomainRead.peopleWords(Int(d.people)),
                chip: SearchChip(kind: .domain, value: d.domain, source: ""), replaces: last, group: 1)
        }
        for p in source.people.prefix(4) {
            let value = p.address   // a pill from a person is sent as the address: it names exactly one person, whatever the name says
            let bold = boldRange(p.name, typed: last)
            if p.fromCount > 0 {
                add(.from, "from.\(p.address)", p.name, bold: bold, detail: p.address, chip: SearchChip(kind: .from, value: value, source: ""),
                    replaces: previous == "from" ? "from " + last : last, group: 1)
            }
            if p.toCount > 0 {
                add(.to, "to.\(p.address)", p.name, bold: bold, detail: p.address, chip: SearchChip(kind: .to, value: value, source: ""),
                    replaces: previous == "to" ? "to " + last : last, group: 1)
            }
        }
        let subject = source.subject(plain) ?? plain
        let subjectBold = subject.range(of: plain, options: [.caseInsensitive, .diacriticInsensitive]).map { r in
            subject.distance(from: subject.startIndex, to: r.lowerBound)..<subject.distance(from: subject.startIndex, to: r.upperBound)
        }
        add(.subject, "subject", subject, bold: subjectBold, chip: SearchChip(kind: .subject, value: subject, source: ""), replaces: plain, group: 1)
        add(.attachment, "attname", plain, bold: 0..<plain.count, chip: SearchChip(kind: .attname, value: plain, source: ""), replaces: plain, group: 1)
        for (i, f) in filterRows(typedWords).enumerated() {
            add(.filter, "filter.\(i)", f.chip.title, chip: f.chip, replaces: f.phrase, group: 1)
        }
        for l in source.labels.prefix(40) {
            guard let hit = match(l, typed: typedWords), out.filter({ $0.kind == .label }).count < 2 else { continue }
            add(.label, "label.\(l)", l, bold: hit.range, chip: SearchChip(kind: .label, value: l, source: ""), replaces: hit.word, group: 1)
        }
        for f in source.folders {
            guard let hit = match(f.name, typed: typedWords), out.filter({ $0.kind == .folder }).count < 2 else { continue }
            add(.folder, "folder.\(f.name)", f.name, bold: hit.range, detail: Words.text("Smart folder"), words: f.rule, group: 1)
        }
        let low = typedWords.map { $0.lowercased() }
        let near = source.recents.filter { r in low.contains { r.lowercased().contains($0) } }
        for (i, r) in (near.isEmpty ? Array(source.recents.prefix(2)) : Array(near.prefix(3))).enumerated() { add(.recent, "recent.\(i)", r, words: r, group: 2) }
        return out
    }
}
