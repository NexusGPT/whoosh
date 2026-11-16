import Foundation

/// The words of the why tag, through the catalogue (Words.text, 14 languages). The AI is never named after its model: "AI" only.
@MainActor
enum WhyTagWords {
    /// `listName` and `accountName` are asked only when the card needs them (a fallback names its list; an account rule names the account).
    static func make(_ r: WhyRead, listName: () -> String, accountName: (String) -> String) -> WhyTag {
        switch r.card {
        case .entry: return WhyTag(card: Words.text("Rule"), detail: r.held.first.map { words($0, accountName: accountName) })
        case .ai: return WhyTag(card: ai, detail: nil)
        case .exit: return WhyTag(card: ai, detail: r.held.first.map { words($0, accountName: accountName) })
        case .fallback: return WhyTag(card: Words.text("Fallback"), detail: listName())
        case .hand: return WhyTag(card: Words.text("By hand"), detail: nil)
        }
    }

    private static var ai: String { "\u{2726} " + Words.text("AI") }

    /// "Unanswered: yes", "From: laura.meier@nordlicht.example".
    static func words(_ h: WhyHeld, accountName: (String) -> String) -> String {
        switch h {
        case .fact(let key, let wanted): return Words.text("%@: %@", fact(key), wanted ? Words.text("yes") : Words.text("no"))
        case .text(let kind, let value): return Words.text("%@: %@", kindName(kind), value)
        case .account(let id): return Words.text("%@: %@", Words.text("Account"), accountName(id))
        }
    }

    private static func fact(_ key: String) -> String {
        switch key {
        case "unread": return Words.text("Unread")
        case "unanswered": return Words.text("Unanswered")
        case "has_attachment": return Words.text("Attachment")
        case "starred": return Words.text("Starred")
        case "vip_sender": return Words.text("VIP")
        case "cc_me": return Words.text("In copy")
        case "mailing_list": return Words.text("List")
        case "replied_by_me": return Words.text("Replied")
        default: return key
        }
    }

    private static func kindName(_ kind: String) -> String {
        switch kind {
        case "from": return Words.text("From")
        case "to": return Words.text("To")
        case "cc": return Words.text("Cc")
        case "domain": return Words.text("Domain")
        case "subject": return Words.text("Subject")
        case "body": return Words.text("Body")
        case "list_header": return Words.text("List")
        default: return kind
        }
    }
}
