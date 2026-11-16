import Foundation

/// The four filters under the phone's search field: account, sender, date, attachment.
/// Sender, date and attachment are the Mac's own chips (`SearchChip`: `from:"…"`, `before June`, `has attachment`), written the
/// way the Mac's field writes them, so the engine reads them with the one parser and a saved folder's rule reads the same on both
/// devices. The account has no chip on the Mac (rows carry their account): it narrows the rows after the engine answers.
struct SearchPhoneFilters: Equatable, Sendable {
    var accountId: String?
    var sender: String?
    /// a month name of the current year ("June"): the rows before the first of that month
    var beforeMonth: String?
    var attachment = false
    /// the sender's domain without its @ (SPEC A153): every mail whose sender's address is at it; written `from:@<domain>`, the pill "From @<domain>"
    var domain: String?

    var isEmpty: Bool { accountId == nil && sender == nil && beforeMonth == nil && !attachment && domain == nil }

    /// The filter words the engine reads, the sender included or left out (the sender's options are read from the rows that pass
    /// every OTHER filter, so the picker still lists the people a pick would hide).
    func words(includingSender: Bool) -> String {
        var parts: [String] = []
        if includingSender, let s = sender { parts.append("from:\"\(s)\"") }
        if let d = domain { parts.append("from:@\(d)") }
        // SPEC A142: only the colon forms are filters; "before June" and "has attachment" typed as words stay words. The month is the first day of
        // that month this year (main97 iPhone save-keeps-the-search: the folder's rule held the words and listed six mails, not thr-01)
        if let m = beforeMonth, let i = Self.months.firstIndex(where: { $0.caseInsensitiveCompare(m) == .orderedSame }) {
            let year = Calendar(identifier: .gregorian).component(.year, from: Date())
            parts.append(String(format: "before:%04d-%02d-01", year, i + 1))
        }
        if attachment { parts.append("has:attachment") }
        return parts.joined(separator: " ")
    }

    /// The month names a Date pick offers: the calendar's twelve, in order (the parser's own English list, `SearchQuery.months`).
    static var months: [String] { SearchQuery.english.monthSymbols }
}

/// One line of an open filter picker.
struct SearchPhoneOption: Equatable, Sendable {
    /// the id the socket row and the click carry: an account id, a month in lower case, a sender's index
    let id: String
    let title: String
    let on: Bool
    /// the account an option stands for (accounts only): the tile is drawn from it
    var accountId: String?
}

/// Which picker a filter chip opens. Attachment has none: it is a switch.
enum SearchPhoneMenu: String, CaseIterable, Sendable { case account, sender, date }

/// One chip under the field: its kind (the ax-id part), its label at rest, and the value it holds (nil = no filter of this kind).
struct SearchPhoneChip: Identifiable, Equatable, Sendable {
    let kind: String
    let label: String
    let value: String?
    var id: String { kind }
}
