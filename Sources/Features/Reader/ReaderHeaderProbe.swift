import Foundation

/// Probe regions for round 6: `mac-main.reader.header` (one row per message: open, the tooltip, the rows the expanded
/// header shows, and the thread's account as the meta line draws it: id, colour and icon from the account book, the
/// same RowAccount the list row serves), `mac-main.reader.keytips` (round 9) and `mac-main.reader.unsubscribe` (the confirm, the method, what it says, the note after).
@MainActor
enum ReaderHeaderProbe {
    static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "mac-main.reader.header": { m in
                let t = try ReaderProbe.thread(m)
                let facts = t.earlier + [t.open]
                let a = RowAccount.of(t.accountId, in: m)
                let account: ControlRow = ["account": a.id, "account_colour": a.colour, "colour": a.colour,
                                           "account_icon": a.iconId ?? NSNull()]
                return facts.enumerated().map { i, f in
                    guard let f else { return (["index": i, "id": NSNull(), "open": false, "chevron": false] as ControlRow).merging(account) { x, _ in x } }
                    let open = ReaderState.shared.headerIsOpen(f.id)
                    let row: ControlRow = ["index": i, "id": f.id, "message_id": f.id, "open": open, "chevron": true,
                            "tooltip": ReaderWords.headerTooltip(open: open),
                            "rows": open ? ReaderWords.headerRows(f).map { ["label": $0.label, "value": $0.value] } : [],
                            "time": f.time, "attachments": f.attachments.count,
                            "motion_ms": ReaderHeaderMotion.ms, "motion": ReaderHeaderMotion.properties,
                            "cc_served": f.cc != nil, "cc_reason": f.cc == nil ? ReaderWords.ccNotServed : NSNull()]
                    return row.merging(account) { x, _ in x }
                }
            },
            "mac-main.reader.keycaps": ReaderHoldCaps.rows,
            // round 9: the tooltips of the controls the keys also reach (the rule that unfolds, the link chips); each carries its key
            "mac-main.reader.keytips": { _ in
                [["id": "earlier", "tooltip": ReaderWords.earlierTooltip, "key": ReaderKeyLabel.chord(for: "expand-all-messages")],
                 ["id": "links", "tooltip": ReaderWords.linkTooltip, "key": ReaderKeyLabel.chord(for: "cycle-links")]]
            },
            "mac-main.reader.unsubscribe": { m in
                let s = ReaderState.shared
                var row: ControlRow = ["pending": s.unsubscribe != nil, "note": s.actionNote ?? NSNull(), "sending": ReaderListActions.sending ?? NSNull(), "opened": ReaderListActions.opened, "calls": ReaderListActions.calls]
                if let u = s.unsubscribe {
                    row["message"] = u.messageId
                    row["says"] = ReaderListActions.consequence(u.method)
                    switch u.method { case .oneClick: row["method"] = "one_click"; case .mail: row["method"] = "mail"; case .link: row["method"] = "link" }
                }
                return [row]
            },
        ]
    }
}
