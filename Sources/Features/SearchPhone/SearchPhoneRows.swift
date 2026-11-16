import Foundation

/// What the socket verb `rows` reads on the phone search screen (ax ids `iphone-search.<part>`): the facts the screen draws,
/// in the order it draws them. Every read settles the search first, so a `type` followed at once by `rows` reads the rows
/// that text finds.
@MainActor
enum SearchPhoneRows {
    static var s: SearchPhoneModel { .shared }

    /// Where results sit under the header: the Mac's own split, the first three are "Top results", the rest "All results".
    static func section(_ i: Int) -> String { i < SearchResultsList.topCount ? "top" : "all" }

    static func menuOptions(_ m: SearchPhoneMenu) -> [SearchPhoneOption] {
        switch m {
        case .account:
            let all = [SearchPhoneOption(id: "all", title: Words.text("All accounts"), on: s.filters.accountId == nil)]
            return all + PhoneAccounts.all(s.app).map { SearchPhoneOption(id: $0.id, title: $0.name, on: s.filters.accountId == $0.id, accountId: $0.id) }
        case .sender:
            return s.senderNames.enumerated().map { SearchPhoneOption(id: "\($0.offset)", title: $0.element, on: s.filters.sender == $0.element) }
        case .date:
            return SearchPhoneFilters.months.map { SearchPhoneOption(id: $0.lowercased(), title: Words.text("Before %@", $0), on: s.filters.beforeMonth == $0) }
        }
    }

    /// The four chips under the field, each with its label as drawn and whether it holds a value.
    static var chips: [SearchPhoneChip] {
        let f = s.filters
        var all = [
            SearchPhoneChip(kind: "account", label: Words.text("Account"), value: f.accountId.map { PhoneAccounts.of($0, in: s.app).name }),
            SearchPhoneChip(kind: "sender", label: Words.text("Sender"), value: f.sender),
            SearchPhoneChip(kind: "date", label: Words.text("Date"), value: f.beforeMonth),
            SearchPhoneChip(kind: "attachment", label: Words.text("Attachments"), value: f.attachment ? Words.text("Has attachment") : nil),
        ]
        // the domain pill "From @<domain>" stands under the field only while a domain is chosen (SPEC A153): a fifth kind with no picker
        if let d = f.domain { all.insert(SearchPhoneChip(kind: "domain", label: Words.text("From %@", "@" + d), value: "@" + d), at: 2) }
        return all
    }

    static func chipLabel(_ c: SearchPhoneChip) -> String {
        guard let v = c.value else { return c.label }
        switch c.kind {
        case "sender", "domain": return Words.text("From %@", v)
        case "date": return Words.text("Before %@", v)
        default: return v
        }
    }

    static var all: [String: (AppModel) throws -> [ControlRow]] { [
        "iphone-search.state": { _ in
            s.settle()
            let x = s.meaning
            return [["ax": "iphone-search.state", "state": s.phase.rawValue, "active": s.active, "text": s.text, "count": s.results.count, "matches": s.matchedCount, "related": s.relatedCount, "rest": s.restCount, "tail_pending": s.tailPending,
                     "filters": s.filters.isEmpty ? 0 : 1, "meaning": x.usable ? "ready" : x.state.rawValue,
                     "meaning_missing": s.meaningLineVisible, "naming": s.naming, "menu": s.menu?.rawValue ?? "",
                     "opened": s.openedMessageId ?? NSNull(), "opened_hit": s.openedHitMessageId ?? NSNull(), "says": s.notes.joined(separator: " · ")]]
        },
        "iphone-search.field": { _ in
            s.settle()
            return [["ax": "iphone-search.field", "text": s.text, "placeholder": Words.text("Search"), "empty": s.isIdleInput]]
        },
        "iphone-search.filters": { _ in
            s.settle()
            return chips.map { c in
                ["ax": "iphone-search.filter.\(c.kind)", "kind": c.kind, "label": chipLabel(c), "on": c.value != nil, "value": c.value ?? NSNull(),
                 "menu_open": s.menu?.rawValue == c.kind, "clear": "iphone-search.filter.\(c.kind).clear"]
            }
        },
        "iphone-search.filter.menu": { _ in
            guard let m = s.menu else { return [] }
            return menuOptions(m).map { o in
                ["ax": "iphone-search.filter.\(m.rawValue).\(o.id)", "menu": m.rawValue, "id": o.id, "title": o.title, "on": o.on,
                 "account": o.accountId ?? NSNull()]
            }
        },
        "iphone-search.results": { _ in
            s.settle()
            let m = s.app
            return s.results.enumerated().map { i, r in
                let a = PhoneAccounts.of(r.row.accountId, in: m)
                return ["ax": "iphone-search.results.\(i)", "id": r.row.messageId, "thread": r.row.threadId, "account": r.row.accountId,
                        "account_name": a.name, "mark_colour": a.colour, "mark_icon": a.markIcon,
                        "sender": r.row.senderName, "subject": r.row.subject, "passage": SearchPhoneWords.passage(r), "unread": r.row.unread,
                        "attachment": r.attachmentName ?? NSNull(), "attachment_says": r.attachmentSays ?? NSNull(),
                        "matched_in": r.matchedIn, "part": r.part.rawValue, "section": section(i),
                        "highlights": SearchPhoneWords.markedWords(r, in: s.engineQuery(includingSender: true)),
                        "found": SearchPhoneFound.source(r), "found_line": SearchPhoneFound.line(r), "bold": SearchPhoneFound.boldWords(r),
                        "hit_message": SearchPhoneModel.holder(of: r),
                        "opened": r.row.messageId == s.openedMessageId]
            }
        },
        "iphone-search.header": { _ in
            s.settle()
            return s.phase == .results || s.phase == .none ? [["ax": "iphone-search.header", "title": Words.text("Results"), "count": s.countLine, "matches": s.matchedCount, "related": s.relatedCount, "rest": s.restCount, "stage": s.stage]] : []
        },
        // the thin line above the related mail (or the ranking line): absent when the engine has nothing to say there
        "iphone-search.divider": { _ in
            s.settle()
            guard let t = s.dividerTitle else { return [] }
            return [["ax": "iphone-search.divider", "title": t, "before": s.matchedCount]]
        },
        // "From domain <domain>" rows the typed word offers (SPEC A153)
        "iphone-search.domain": { _ in
            s.settle()
            return s.domainOffers.enumerated().map { i, o in
                ["ax": "iphone-search.domain.\(i)", "domain": o.chip?.value ?? o.title, "title": Words.text("From domain"), "people": o.detail, "chip": o.chip?.title ?? ""]
            }
        },
        "iphone-search.sections": { _ in
            s.settle()
            guard s.upperCount > 0 else { return [] }
            let titles = s.upperCount > SearchResultsList.topCount ? [SearchResultsList.topTitle, SearchResultsList.allTitle] : [SearchResultsList.topTitle]
            return titles.enumerated().map { i, t in ["ax": "iphone-search.sections.\(i)", "title": t] }
        },
        // the closed line "All other mail · N" (SPEC A150): its count, whether it is open and how many rows it lists; absent while the rest is not known or empty
        "iphone-search.more": { _ in
            s.settle()
            guard s.showsMore else { return [] }
            return [["ax": "iphone-search.more", "title": SearchResultsList.moreTitle, "count": s.restCount, "open": s.restOpen, "listed": s.results.count - s.upperCount]]
        },
        "iphone-search.footer": { _ in
            s.settle()
            return s.results.isEmpty ? [] : [["ax": "iphone-search.footer", "text": SearchResultsList.footerLine]]
        },
        "iphone-search.skeleton": { _ in
            s.settle()
            return [["ax": "iphone-search.skeleton", "loading": s.phase == .searching && s.results.isEmpty]]
        },
        "iphone-search.empty": { _ in
            s.settle()
            return s.showsNoMatch ? [["ax": "iphone-search.empty", "title": Words.text("No match"), "line": Words.text("No mail holds these words, and none is close to them.")]] : []
        },
        "iphone-search.recent": { _ in s.recents.enumerated().map { i, q in ["ax": "iphone-search.recent.\(i)", "text": q] } },
        "iphone-search.meaning": { _ in
            s.settle()
            guard s.meaningLineVisible else { return [] }
            return [["ax": "iphone-search.meaning", "state": s.meaning.state.rawValue, "text": s.meaningWords, "button": s.meaningButton,
                     "door": "iphone-search.meaning.download", "taps": s.doorTaps]]
        },
        "iphone-search.save": { _ in
            [["ax": "iphone-search.save", "open": s.naming, "text": s.name, "says": s.fieldSays, "saved": s.savedAs ?? NSNull(),
              "enabled": (s.phase == .results || s.phase == .none) && !s.ruleWords.isEmpty, "rule": s.ruleWords]]
        },
    ] }
}
