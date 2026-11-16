import Foundation

/// One quoted message of a reply: who wrote it, when, and its plain text.
struct ComposerQuote: Equatable, Identifiable {
    let from: String
    let time: String
    let text: String
    var id: String { from + time }
}

/// A person the To field can offer: a name and an address.
struct ComposerSuggestion: Equatable, Identifiable {
    let name: String
    let address: String
    var id: String { address }
}

/// What the composer reads from the core: accounts (the shell's AccountBook), the message a reply answers
/// (`message(id)` and `body(id)`), contacts (`contact(address)`), and the signature behind a guard.
@MainActor
enum ComposerContext {
    /// the surface window the shell opens for the composer (`WindowBridge.open`)
    static let slot = SurfaceSlot(ComposerIds.surface)

    struct Reply: Equatable {
        let messageId: String
        let threadId: String
        let accountId: String
        let subject: String
        let toAddress: String
        let toName: String
        let quoted: [ComposerQuote]
        /// Every address the reply goes to, the first being `toAddress` (a Reply-To can name several).
        let toAddresses: [String]
    }

    /// The account's signature, read from the core (`WhooshCore.signature(address:)`, delta 7); "" when it has none.
    static func signature(address: String) -> String {
        guard !address.isEmpty else { return "" }
        return AppModel.shared.core.signature(address: address)?.text ?? ""
    }

    static func accounts() -> [Account] {
        AccountBook.shared.accounts().enumerated().map { i, a in
            Account(id: a.id, address: a.address, provider: a.provider, displayName: a.name, colour: a.colour,
                    order: UInt32(i), paused: false, group: nil, iconId: a.iconId)
        }
    }

    /// The thread a reply answers: from the account that received it, to its sender, with the message's text quoted.
    static func reply(messageId: String, core: WhooshCore) -> Reply? {
        guard let m = core.message(messageId: messageId) else { return nil }
        let text = core.body(messageId: messageId)?.plainText ?? m.preview
        let quote = ComposerQuote(from: m.senderName, time: time(m.receivedAtMs), text: text)
        // fix-replydoors: the one place every reply door (composer, quick reply box, notch, phone) takes its recipient from
        let who = ReplyToRule.plain(messageId: messageId, fromAddress: m.fromAddress, senderName: m.senderName, state: { core.state(path: $0) })
        return Reply(messageId: messageId, threadId: m.threadId, accountId: m.accountId, subject: m.subject,
                     toAddress: who.addresses.first ?? m.fromAddress, toName: who.name, quoted: [quote], toAddresses: who.addresses)
    }

    /// The name the core knows for an address, nil for a stranger.
    static func contactName(_ address: String, core: WhooshCore) -> String? {
        core.contact(address: address)?.name
    }

    /// Suggestions for the To field: the person being answered, and any typed address the core has a card for.
    static func suggestions(for typed: String, replyTo: ComposerSuggestion?, core: WhooshCore) -> [ComposerSuggestion] {
        let p = typed.trimmingCharacters(in: .whitespaces).lowercased()
        guard !p.isEmpty else { return [] }
        var out: [ComposerSuggestion] = []
        if let r = replyTo, r.name.lowercased().contains(p) || r.address.lowercased().contains(p) { out.append(r) }
        if p.contains("@"), let c = core.contact(address: p), let a = c.addresses.first(where: { $0.lowercased() == p }) ?? c.addresses.first,
           !out.contains(where: { $0.address == a }) {
            out.append(ComposerSuggestion(name: c.name, address: a))
        }
        // The people of all the mail received and sent, by the first letters of a name, of a word of it or of the address (the engine's read the search
        // field uses, `people/prefix/<letters>`): those the owner wrote to come first. Lead, 13 Nov 2026 (picture of a new message on main157): "ni"
        // in To offered nobody; a name was completed only for the person a reply answers and for a whole address typed out.
        // An address nobody answers (noreply@, mailer-daemon@) is offered only when the owner wrote to it before (probe on main159: "n" listed two
        // noreply addresses among five).
        let people = PeopleRead.people(core: core, prefix: p)
        for person in people.filter({ $0.toCount > 0 }) + people.filter({ $0.toCount == 0 && !isMachineSender($0.address) })
        where !out.contains(where: { $0.address.lowercased() == person.address.lowercased() }) {
            // the name the chip will show (the contact's), so the line and the chip it becomes read the same (picture on main159: a line "Karim Capital"
            // became the chip "Karim Haddad")
            out.append(ComposerSuggestion(name: core.contact(address: person.address)?.name ?? person.name, address: person.address))
        }
        return out
    }

    /// An address that takes no mail by its own name: noreply, no-reply, donotreply, do-not-reply, mailer-daemon, bounce, bounces.
    static func isMachineSender(_ address: String) -> Bool {
        let local = (address.split(separator: "@").first.map(String.init) ?? address).lowercased()
        return local.range(of: #"^(no[-_.]?reply|do[-_.]?not[-_.]?reply|mailer[-_.]?daemon|bounces?)([-_.+]|$)"#, options: .regularExpression) != nil
    }

    private static func time(_ ms: Int64) -> String {
        ms > 0 ? Date(timeIntervalSince1970: Double(ms) / 1000).formatted(.dateTime.weekday(.abbreviated).hour().minute()) : ""
    }
}

/// Where a reply goes when the mail has a Reply-To (fix-replydoors). Two local state reads of the core, never a network wait:
/// `message/<id>/reply_to` (JSON `[{"name","address"}]`, `[]` when none) and `message/<id>/reply_recipients` (JSON array of addresses:
/// where a plain Reply goes). A core that answers nothing, junk, or an empty list leaves today's behaviour: the sender.
/// `state` is `core.state(path:)`; a function so a test can stand in for the core.
enum ReplyToRule {
    struct Entry: Equatable { let name: String; let address: String }

    static func same(_ a: String, _ b: String) -> Bool {
        a.trimmingCharacters(in: .whitespaces).caseInsensitiveCompare(b.trimmingCharacters(in: .whitespaces)) == .orderedSame
    }

    /// Each address once (case ignored), trimmed, none blank, none of `own`, in the order given.
    static func unique(_ addresses: [String], not own: [String] = []) -> [String] {
        var out: [String] = []
        for a in addresses {
            let t = a.trimmingCharacters(in: .whitespaces)
            if t.isEmpty || own.contains(where: { same($0, t) }) || out.contains(where: { same($0, t) }) { continue }
            out.append(t)
        }
        return out
    }

    /// The Reply-To entries the core holds for a mail; [] when it holds none or answers nothing.
    static func entries(_ messageId: String, state: (String) -> String?) -> [Entry] {
        guard let text = state("message/\(messageId)/reply_to"),
              let list = (try? JSONSerialization.jsonObject(with: Data(text.utf8))) as? [[String: Any]] else { return [] }
        return list.compactMap { d in
            guard let a = (d["address"] as? String)?.trimmingCharacters(in: .whitespaces), !a.isEmpty else { return nil }
            return Entry(name: (d["name"] as? String) ?? "", address: a)
        }
    }

    /// Plain Reply: the addresses the core says (Reply-To first), else the sender. The name is the Reply-To entry's name when an entry has
    /// the first recipient's address, else the sender's name when the first recipient is the sender, else "".
    static func plain(messageId: String, fromAddress: String, senderName: String, state: (String) -> String?) -> (addresses: [String], name: String) {
        var served: [String] = []
        if let text = state("message/\(messageId)/reply_recipients"),
           let list = (try? JSONSerialization.jsonObject(with: Data(text.utf8))) as? [String] { served = unique(list) }
        guard let first = served.first else { return ([fromAddress], senderName) }
        if let e = entries(messageId, state: state).first(where: { same($0.address, first) }), !e.name.isEmpty { return (served, e.name) }
        return (served, same(first, fromAddress) ? senderName : "")
    }

    /// Reply all: with a Reply-To (the owner's own left out) To = those addresses and Cc = every other To and Cc address of the mail, none of
    /// `own`, none twice, none already in To. Without one: the contract's `replyAllRecipients`, as before.
    static func all(messageId: String, fromAddress: String, to: [String], cc: [String], own: [String], state: (String) -> String?) -> ReplyRecipients {
        let asked = unique(entries(messageId, state: state).map(\.address), not: own)
        guard !asked.isEmpty else { return replyAllRecipients(fromAddress: fromAddress, to: to, cc: cc, ownAddresses: own) }
        let others = unique(to + cc, not: own).filter { a in !asked.contains(where: { same($0, a) }) }
        return ReplyRecipients(to: asked, cc: others)
    }
}
