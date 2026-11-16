import Foundation

/// One condition that held for a message, as the core serves it in the `matched` array of `message/<id>/why/<view>`
/// (the store's JSON: {"kind":"fact","fact":"unread","wanted":true}, {"kind":"from","words":"laura"}, {"kind":"has_attachment"}).
enum WhyHeld: Equatable, Sendable {
    case fact(key: String, wanted: Bool)
    case text(kind: String, value: String)
    /// an "Account is" rule: the value is an account id, worded as the account's name when the tag is made
    case account(id: String)

    init?(_ o: [String: Any]) {
        switch o["kind"] as? String {
        case "fact"?:
            guard let k = o["fact"] as? String, let w = o["wanted"] as? Bool else { return nil }
            self = .fact(key: k, wanted: w)
        case "has_attachment"?: self = .fact(key: "has_attachment", wanted: true)
        case "domain"?: self = .text(kind: "domain", value: o["domain"] as? String ?? "")
        case "account"?: self = .account(id: o["account_id"] as? String ?? "")
        case let kind? where Self.wordKinds.contains(kind): self = .text(kind: kind, value: o["words"] as? String ?? "")
        default: return nil
        }
    }

    private static let wordKinds: Set<String> = ["from", "to", "cc", "subject", "body", "list_header"]
}

/// The core's answer to "why here" for one message in one view: which card placed it and which conditions of that card's chain held.
/// The core keeps no sentence (contract delta 17), so no language is mixed: the words are made here, from the keys (WhyTagWords).
struct WhyRead: Equatable, Sendable {
    enum Card: String, Sendable { case entry, ai, exit, fallback, hand }
    let card: Card
    let held: [WhyHeld]

    /// nil when the view does not hold the message (the core answers no value) or the answer is not a why read.
    static func parse(_ json: String?) -> WhyRead? {
        guard let json, let data = json.data(using: .utf8),
              let o = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let card = (o["card"] as? String).flatMap(Card.init(rawValue:)) else { return nil }
        return WhyRead(card: card, held: (o["matched"] as? [[String: Any]] ?? []).compactMap(WhyHeld.init))
    }
}

/// The tag a row draws: the card word first (never cut), then the first condition that held (cut with an ellipsis when the line is short).
struct WhyTag: Equatable, Sendable {
    let card: String
    let detail: String?
    /// "Rule · From: laura", "✦ AI · Unanswered: yes", "Fallback · Primary", "By hand": what the probe and the accessibility label say.
    var line: String { detail.map { card + " · " + $0 } ?? card }
}
