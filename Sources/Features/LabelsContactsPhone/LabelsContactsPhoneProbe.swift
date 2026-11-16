import Foundation

/// The `rows` answers of the screen: the same facts the pages draw, in the order they draw them. Regions:
/// `iphone-labels-contacts.page` (where the stack stands), `.labels`, `.people`, `.smart-folders` and `.match-count` (the core's smart folders, read only: the rule editor stays on the Mac), `.label` and `.label.cards`, `.mail`, `.contact`, `.contact.messages`.
@MainActor
enum LabelsContactsPhoneProbe {
    static let ax = LabelsContactsPhoneState.surface
    private static var state: LabelsContactsPhoneState { .shared }

    static func page(_ m: AppModel) -> [ControlRow] {
        let kind: String
        switch state.path.last {
        case .label?: kind = "label"
        case .mail?: kind = "mail"
        case .contact?: kind = "contact"
        case nil: kind = m.overlay == ax ? "list" : "closed"
        }
        return [["ax": "\(ax).page", "kind": kind, "depth": state.path.count, "overlay": m.overlay ?? NSNull(),
                 "label_id": state.currentLabelId ?? NSNull(), "contact": state.currentContactAddress ?? NSNull()]]
    }

    static func labels(_ m: AppModel) -> [ControlRow] {
        let rows = m.core.labels()
        let tree = LabelTree(rows: rows)
        let byId = Dictionary(rows.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
        return tree.ordered().compactMap { item in
            byId[item.id].map { l in
                ["ax": "\(ax).list.label.\(l.id)", "id": l.id, "name": l.name, "depth": tree.depth(l.id), "pinned": l.pinned,
                 "unread": Int(l.unread), "colour": l.colour ?? NSNull()] as ControlRow
            }
        }
    }

    /// The core's smart folders in the core's order, each with its rule in words (the core's `ruleWords`) and its match count (`smart_folder/<id>/count`).
    static func smartFolders(_ m: AppModel) -> [ControlRow] {
        m.core.smartFolders().enumerated().map { i, f in
            ["ax": "\(ax).smart-folders.\(i)", "id": f.id, "name": f.name, "rule": f.ruleWords, "parent": f.parentId ?? NSNull(),
             "count": SidebarData.smartFolderCount(f.id, model: m) ?? NSNull()] as ControlRow
        }
    }

    /// One match count per smart folder, 0 included (a rule that matches nothing reads 0, never a missing row).
    static func matchCount(_ m: AppModel) -> [ControlRow] {
        m.core.smartFolders().enumerated().map { i, f in
            ["ax": "\(ax).match-count.\(i)", "id": f.id, "count": SidebarData.smartFolderCount(f.id, model: m) ?? 0] as ControlRow
        }
    }

    static func people(_ m: AppModel) -> [ControlRow] {
        PhoneContactReader.people(m).enumerated().map { i, p in
            ["ax": "\(ax).list.person.\(i)", "id": p.id, "name": p.name, "addresses": p.addresses] as ControlRow
        }
    }

    static func label(_ m: AppModel) -> [ControlRow] {
        guard let id = state.currentLabelId, let l = PhoneLabelReader.read(id, m) else { return [] }
        return [["ax": "\(ax).label", "id": l.id, "name": l.name, "path": l.path, "depth": l.depth, "colour": l.colour ?? NSNull(),
                 "meaning": l.meaning, "pinned": l.pinned, "scope": l.scope, "unread": l.unread, "mail": l.mail,
                 "pin_failure": state.pinFailure ?? NSNull(), "edit_note": Words.text("Rules are edited on the Mac.")]]
    }

    static func cards(_ m: AppModel) -> [ControlRow] {
        guard let id = state.currentLabelId, let l = PhoneLabelReader.read(id, m) else { return [] }
        return l.cards.map { c in
            let chains: [[String: Any]] = c.chains.map { ["lead": $0.lead, "mode": $0.mode ?? "", "lines": $0.lines] }
            return ["ax": "\(ax).label.card.\(c.key)", "card": c.key, "title": c.title, "subtitle": c.subtitle, "count": c.count, "on": c.on,
                    "lines": c.allLines, "chains": chains, "note": c.note ?? NSNull()] as ControlRow
        }
    }

    static func mail(_ m: AppModel) -> [ControlRow] {
        guard case .mail(let id)? = state.path.last else { return [] }
        return PhoneLabelReader.mail(id, m).enumerated().map { i, r in
            ["ax": "\(ax).mail.row.\(i)", "id": r.messageId, "sender": r.senderName, "subject": r.subject, "account": r.accountId] as ControlRow
        }
    }

    static func contact(_ m: AppModel) -> [ControlRow] {
        guard let a = state.currentContactAddress, let c = PhoneContactReader.read(address: a, m) else { return [] }
        return [["ax": "\(ax).contact", "id": c.id, "name": c.name, "addresses": c.addresses, "mark": c.logoId ?? "none",
                 "mail": c.messages.count, "look_alike": c.lookAlike?.address ?? NSNull()]]
    }

    static func contactMessages(_ m: AppModel) -> [ControlRow] {
        guard let a = state.currentContactAddress, let c = PhoneContactReader.read(address: a, m) else { return [] }
        return c.messages.enumerated().map { i, r in
            ["ax": "\(ax).contact.messages.\(i)", "id": r.messageId, "sender": r.senderName, "subject": r.subject, "account": r.accountId,
             "received_ms": Int(r.receivedAtMs)] as ControlRow
        }
    }
}
