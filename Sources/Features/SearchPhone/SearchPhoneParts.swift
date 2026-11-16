import Foundation

/// The three parts of the list, the Mac's rule (SPEC A150, docs/lanes/list-p1.md): the matches (the typed letters are in the mail), the related mail
/// (none of the letters, closeness clearly above that search's average: the engine decides, `SearchResult.part`), then one closed line "All other mail"
/// that opens the rest. The engine's order is kept; nothing here re-orders it or scores a mail. The count line reads "N matches · M related".
@MainActor
extension SearchPhoneModel {
    /// Keeps the whole answer in `everyResult` and the rows to draw in `results`. While the first answer waits for its ranking (stage "words") the
    /// other mails are not listed, as on the Mac.
    func applyParts(_ listed: [SearchResult]) {
        tailPending = stage == "words" && listed.contains { $0.part != .match }
        everyResult = listed.filter { !(stage == "words" && $0.part != .match) }
        results = restOpen ? everyResult : everyResult.filter { $0.part != .rest }
    }

    /// How many conversations match (one per thread): the number the count line says.
    public var matchedCount: Int { everyResult.filter { $0.part == .match }.count }
    public var relatedCount: Int { everyResult.filter { $0.part == .related }.count }
    /// How many conversations stand behind the closed line.
    public var restCount: Int { everyResult.filter { $0.part == .rest }.count }
    /// How many rows stand above the closed line.
    public var upperCount: Int { everyResult.filter { $0.part != .rest }.count }
    /// The closed line is on the list: the rest is known (not while the first answer waits for its ranking) and not empty.
    public var showsMore: Bool { stage != "words" && restCount > 0 }

    /// The thin line above the related mail (the Mac's `SearchResultsList.titles`, the same function): "Related" where related mail stands, the ranking
    /// line while the first answer waits for it, "Other mails, by date" with no meaning order; nil when there is none.
    public var dividerTitle: String? {
        SearchResultsList.titles(related: relatedCount, rest: restCount, stage: stage, tailPending: tailPending).first
    }

    /// The list says "No match": no mail holds the letters and none is related (the closed line stays under it when mail stands behind it).
    public var showsNoMatch: Bool { (phase == .results || phase == .none) && upperCount == 0 && !tailPending }

    /// "N matches · M related", "No match" when both are 0 (the Mac's words, `SearchMacModel.countLine`).
    public var countLine: String {
        let line = SearchMacModel.countLine(matches: matchedCount, related: relatedCount)
        return line.isEmpty ? SearchMacModel.countLine(0) : line
    }

    /// A tap on the closed line: the rest opens under it, the same tap closes it. The rows above stay as they are.
    @discardableResult
    public func toggleRest() -> Bool {
        settle()
        guard showsMore else { return false }
        restOpen.toggle()
        results = restOpen ? everyResult : everyResult.filter { $0.part != .rest }
        return true
    }
}
