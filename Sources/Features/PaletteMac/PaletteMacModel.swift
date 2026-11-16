import Foundation
import Observation

/// The palette's state. The field's text is `AppModel.typed` (one home: the real keyboard and the socket `type`
/// verb both write it). The pools are read from the core when the palette opens; typing only ranks them.
@MainActor @Observable
public final class PaletteMacModel {
    public static let shared = PaletteMacModel()

    enum Step { case commands, accounts }

    var step: Step = .commands
    var selected = 0

    @ObservationIgnored private var commandPool: [PaletteItem] = []
    @ObservationIgnored private var jumpPool: [PaletteItem] = []
    @ObservationIgnored private var accountPool: [AccountBook.Entry] = []
    @ObservationIgnored private var memo: (query: String, commands: [PaletteItem], jumps: [PaletteItem])?

    var app: AppModel { AppModel.shared }
    var query: String { app.typed }
    var isOpen: Bool { app.surface == "mac-palette" }

    /// Where the focused message sits now; the account picker leaves it out.
    var currentAccountId: String? { app.rows.indices.contains(app.focusIndex) ? app.rows[app.focusIndex].accountId : nil }

    /// vip-mark: whether the focused mail's sender is a VIP now. Both engines serve the VIPs view (`Mailbox.vips`): it lists this mail, or
    /// another mail of the same address, when the sender is on the VIP list. Read when the palette builds its lines.
    var focusedSenderIsVip: Bool {
        guard let row = app.shownRow else { return false }
        let sender = row.senderAddress.lowercased()
        let vips = app.core.inbox(query: InboxQuery(tabId: "primary", accountId: nil, labelId: nil, mailbox: .vips, accountIds: [], filters: [], smartFolderId: nil))
        return vips.contains { $0.messageId == row.messageId || (!sender.isEmpty && $0.senderAddress.lowercased() == sender) }
    }

    var commands: [PaletteItem] { ranked().commands }
    var jumps: [PaletteItem] { ranked().jumps }
    /// The lines in the order they are drawn and walked: commands, then jumps.
    var items: [PaletteItem] { step == .commands ? commands + jumps : [] }
    var accounts: [AccountBook.Entry] { PaletteMatch.rank(accountPool, query) { $0.name + " " + $0.address } }
    var count: Int { step == .commands ? commands.count + jumps.count : accounts.count }

    private func ranked() -> (commands: [PaletteItem], jumps: [PaletteItem]) {
        if let m = memo, m.query == query { return (m.commands, m.jumps) }
        let c = PaletteMatch.rank(commandPool, query) { $0.haystack }, j = PaletteMatch.rank(jumpPool, query) { $0.haystack }
        memo = (query, c, j)
        return (c, j)
    }

    func refreshPools() {
        commandPool = PaletteCatalog.commands()
        jumpPool = PaletteCatalog.jumps(app)
        accountPool = PaletteCatalog.accounts(app, excluding: currentAccountId)
        memo = nil
    }

    @discardableResult
    func open() -> Bool {
        if isOpen { return close() }
        app.typed = ""; selected = 0; step = .commands
        refreshPools()
        app.surface = "mac-palette"; app.overlay = "mac-palette"
        return true
    }

    @discardableResult
    func close() -> Bool {
        app.typed = ""; selected = 0; step = .commands
        app.overlay = nil; app.surface = "mac-main"
        return true
    }

    func move(_ delta: Int) -> Bool {
        guard count > 0 else { return false }
        selected = min(max(0, selected + delta), count - 1)
        return true
    }

    func queryChanged() { selected = 0 }

    /// Runs the line at `i` (the highlighted one by default).
    @discardableResult
    func run(_ index: Int? = nil) -> Bool {
        let i = index ?? selected
        if step == .accounts { return pick(i) }
        let list = items
        guard list.indices.contains(i) else { return false }
        return perform(list[i])
    }
}
