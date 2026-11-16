import Foundation

/// One person as the contact page shows them: every address that is theirs and every message from any of them, newest first.
struct PhoneContactRead: Equatable, Sendable {
    let id: String
    let name: String
    let addresses: [String]
    /// The core's id of the sender's mark, or nil. No shared component draws a sender's mark yet, so the page draws none.
    let logoId: String?
    let messages: [InboxRow]
    /// A look-alike person the owner may merge into this card: the same name, or an address at one of this person's domains.
    let lookAlike: PhoneLookAlike?
}

struct PhoneLookAlike: Equatable, Sendable {
    let name: String
    let address: String
    let cardId: String
}

@MainActor
enum PhoneContactReader {
    /// The people list is capped: a long address book never builds a page-long list on the phone.
    private static let peopleMax = 60   // PHONE_PEOPLE_LIST_MAX, listed for the constants ledger in docs/lanes/labelscontactsphone.md

    static func people(_ m: AppModel) -> [Contact] { Array(m.core.contacts().prefix(peopleMax)) }

    static func read(address: String, _ m: AppModel) -> PhoneContactRead? {
        let core = m.core
        guard let card = core.contact(address: address) else { return nil }
        let people = core.contacts()
        let person = people.first { $0.addresses.contains(address.lowercased()) || $0.addresses.contains(address) } ?? card
        var cards: [Contact] = []
        for a in person.addresses {
            if let c = core.contact(address: a), !cards.contains(where: { $0.id == c.id }) { cards.append(c) }
        }
        if cards.isEmpty { cards = [card] }
        var seen = Set<String>()
        let messages = cards.flatMap { core.contactMessages(contactId: $0.id) }
            .filter { seen.insert($0.messageId).inserted }
            .sorted { $0.receivedAtMs > $1.receivedAtMs }
        let domains = Set(person.addresses.compactMap { $0.split(separator: "@").last.map(String.init) })
        let other = people.first { p in
            p.id != person.id && (p.name.caseInsensitiveCompare(person.name) == .orderedSame
                || p.addresses.contains { a in a.split(separator: "@").last.map(String.init).map(domains.contains) ?? false })
        }
        let alike = other.flatMap { o in
            o.addresses.first.flatMap { a in core.contact(address: a).map { PhoneLookAlike(name: o.name, address: a, cardId: $0.id) } }
        }
        return PhoneContactRead(id: card.id, name: person.name, addresses: person.addresses, logoId: person.logoId ?? card.logoId,
                                messages: messages, lookAlike: alike)
    }

    /// Folds the look-alike into this card through `mergeContacts`; the page reads the core again.
    static func merge(_ r: PhoneContactRead, _ m: AppModel) -> Bool {
        guard let other = r.lookAlike else { return false }
        return m.perform(.mergeContacts(keepId: r.id, mergeId: other.cardId))
    }
}
