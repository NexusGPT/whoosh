import Foundation

/// The ONE account directory the Mac list, the sidebar and the phone read (inboxphone gap 3), instead of
/// each lane decoding the fixture file. Every fact comes from the core: `WhooshCore.accounts()` (name,
/// address, provider, colour, icon pick, unread, health), in the core's order.
@MainActor
public final class AccountBook {
    public static let shared = AccountBook()

    public struct Entry: Identifiable, Equatable, Sendable {
        public let id: String
        public let name: String
        public let address: String
        public let provider: MailProvider
        /// "#RRGGBB", or "" when the source gives none (draw the neutral ring)
        public let colour: String
        /// the per-account glyph pick; nil draws the provider default
        public let iconId: String?
        public let unread: Int
        /// the sync error in words, nil while healthy
        public let health: String?
    }

    public func accounts(core: WhooshCore = CoreHost.shared.core) -> [Entry] {
        core.accounts().map { a in
            Entry(id: a.id, name: AccountNames.display(a), address: a.address, provider: a.provider, colour: a.colour,
                  iconId: a.iconId, unread: Int(a.unread), health: a.health?.reasonWords)
        }
    }

    public func account(_ id: String, core: WhooshCore = CoreHost.shared.core) -> Entry? {
        accounts(core: core).first { $0.id == id }
    }

    /// The two facts every surface's mark draws and its probe rows serve: the account's icon pick (nil = provider default)
    /// and colour ("" = neutral ring). One helper for the sidebar, the search results and the notch rows (probes-r1):
    /// read the book once per probe answer, then call `mark(of:in:)` per row.
    public struct Mark: Sendable {
        public let icon: String?
        public let colour: String
        /// the icon as a probe row value: the pick, or null
        public var iconValue: Any { icon ?? NSNull() }
    }

    public static func mark(of accountId: String, in book: [Entry]) -> Mark {
        let a = book.first { $0.id == accountId }
        return Mark(icon: a?.iconId, colour: a?.colour ?? "")
    }
}
