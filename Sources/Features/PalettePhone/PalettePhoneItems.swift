import Foundation

/// One line of the phone palette: a command by name, or a place to go.
struct PalettePhoneItem: Identifiable, Equatable {
    enum Group: Int, CaseIterable { case suggested, commands, goTo }
    enum Act: Equatable {
        case mail(SwipeAction)                     // archive, snooze, pin, read: the swipe's own path, undo capsule included
        case undo
        case screen(String, needsMail: Bool)       // the phone screen another lane draws (overlay name)
        case mailbox(String), account(String), tab(String), label(String)
    }

    let id: String
    let title: String
    var detail = ""
    let symbol: String
    let group: Group
    let act: Act
    var accountId: String?

    var isJump: Bool {
        switch act { case .mailbox, .account, .tab, .label: true; default: false }
    }
    /// What the fuzzy match reads: the title, and for a jump what sits beside it (the Mac palette's rule).
    var haystack: String { isJump && !detail.isEmpty ? title + " " + detail : title }
    var headerKey: String { ["suggested", "commands", "go-to"][group.rawValue] }
}

/// The phone's mailboxes: the Mailboxes screen's names, the contract's `Mailbox` behind each (nil = every inbox).
enum PalettePhoneMailboxes {
    static let all: [(key: String, title: String, symbol: String, box: Mailbox?)] = [
        ("all-inboxes", "All Inboxes", "tray", nil), ("vips", "VIPs", "star", .vips), ("pinned", "Pinned", "pin", .pinned),
        ("snoozed", "Snoozed", "clock", .snoozed), ("drafts", "Drafts", "doc", .drafts), ("sent", "Sent", "paperplane", .sent),
        ("junk", "Junk", "xmark.bin", .junk), ("trash", "Trash", "trash", .trash), ("archive", "Archive", "archivebox", .archive),
        ("starred", "Starred", "star.circle", .starred), ("muted", "Muted", "bell.slash", .muted), ("all-mail", "All mail", "tray.2", .allMail),
    ]
}

/// The pool the phone palette ranks, built once when it opens. Commands keep the Mac's ids and titles (`Commands`);
/// the places to go come from the Mac palette's own catalogue (accounts, split tabs, labels at any depth).
/// Left out on purpose: every command that exists only for the Mac's window, notch or key list (docs/lanes/palettephone.md).
@MainActor
enum PalettePhoneCatalog {
    private struct Spec {
        let id: String, symbol: String, leads: String, act: PalettePhoneItem.Act
    }

    /// The words that open first when the field is empty: the four things a phone owner does most.
    private static let suggestedIds: Set<String> = ["archive", "snooze", "label", "compose"]

    static func pool(_ app: AppModel) -> [PalettePhoneItem] { commands() + places(app) }

    private static func commands() -> [PalettePhoneItem] {
        let specs = [
            Spec(id: "archive", symbol: "archivebox", leads: PalettePhoneWords.kindMail, act: .mail(.archive)),
            Spec(id: "snooze", symbol: "clock", leads: PalettePhoneWords.kindMail, act: .mail(.snooze)),
            Spec(id: "label", symbol: "tag", leads: PalettePhoneWords.kindMail, act: .screen("iphone-label-picker", needsMail: true)),
            Spec(id: "pin", symbol: "pin", leads: PalettePhoneWords.kindMail, act: .mail(.pin)),
            Spec(id: "unread", symbol: "envelope.open", leads: PalettePhoneWords.kindMail, act: .mail(.read)),
            Spec(id: "undo", symbol: "arrow.uturn.backward", leads: PalettePhoneWords.kindMail, act: .undo),
            Spec(id: "compose", symbol: "square.and.pencil", leads: PalettePhoneWords.kindComposer, act: .screen("iphone-composer", needsMail: false)),
            Spec(id: "search", symbol: "magnifyingglass", leads: PalettePhoneWords.kindSearch, act: .screen("iphone-search", needsMail: false)),
        ]
        var seen = Set<String>(), out: [PalettePhoneItem] = []
        for s in specs {
            let raw = PaletteCatalog.display(Commands.title(s.id) ?? s.id)
            let title = Words.text(raw)
            guard seen.insert(title.lowercased()).inserted else { continue }   // a title is listed once
            out.append(PalettePhoneItem(id: "command:\(s.id)", title: title, detail: s.leads, symbol: s.symbol,
                                        group: suggestedIds.contains(s.id) ? .suggested : .commands, act: s.act))
        }
        out.append(PalettePhoneItem(id: "settings", title: PalettePhoneWords.settings, symbol: "gear", group: .commands,
                                    act: .screen("iphone-settings", needsMail: false)))
        return out
    }

    private static func places(_ app: AppModel) -> [PalettePhoneItem] {
        var out = PalettePhoneMailboxes.all.map { b in
            PalettePhoneItem(id: "mailbox:\(b.key)", title: Words.text(b.title), detail: PalettePhoneWords.kindMailbox, symbol: b.symbol,
                             group: .goTo, act: .mailbox(b.key))
        }
        for j in PaletteCatalog.jumps(app) {
            switch j.act {
            case .jumpAccount(let id):
                out.append(PalettePhoneItem(id: j.id, title: j.title, detail: j.detail, symbol: "person.crop.circle", group: .goTo, act: .account(id), accountId: id))
            case .jumpTab(let id):
                out.append(PalettePhoneItem(id: j.id, title: j.title, detail: PalettePhoneWords.kindTab, symbol: "rectangle.split.3x1", group: .goTo, act: .tab(id)))
            case .jumpLabel(let id):
                out.append(PalettePhoneItem(id: j.id, title: j.title, detail: PalettePhoneWords.kindLabel, symbol: "tag", group: .goTo, act: .label(id)))
            default: continue
            }
        }
        return out
    }
}
