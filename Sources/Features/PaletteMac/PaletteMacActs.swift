import Foundation

/// What a chosen line does. The palette closes first, so the command or the jump meets the inbox, never the palette.
extension PaletteMacModel {
    @discardableResult
    func perform(_ item: PaletteItem) -> Bool {
        switch item.act {
        case .moveAccount:
            guard app.shownMessageId != nil else { return false }
            app.typed = ""; selected = 0; step = .accounts
            refreshPools()
            return true
        case .markVip(let vip):
            guard let id = app.shownMessageId else { return false }
            close()
            return app.perform(.markVip(messageId: id, vip: vip))
        case .command("palette"):
            app.typed = ""; selected = 0
            return true
        case .command(let id):
            close()
            return CommandRegistry.handler(id)?(app) ?? false
        case .chord(let key):
            close()
            return app.press(key)
        case .resultTarget(let slug):
            close()
            return Registrations.click("mac-palette.result.\(slug)")?(app) ?? false
        case .jumpAccount(let id):
            close()
            app.accountFilter = id
            return landOnFirstRow()
        case .jumpTab(let id):
            close()
            app.tab = id
            return landOnFirstRow()
        case .jumpLabel(let id):
            close()
            return Registrations.click("mac-main.sidebar.label.\(id).jump")?(app) ?? false
        case .openSettings(let slot):
            close()
            return WindowBridge.open(SurfaceSlot(slot), in: app)
        case .none:
            return false
        }
    }

    /// The jump changed the list's query; the list's first row is the one it focuses.
    private func landOnFirstRow() -> Bool {
        app.focusIndex = 0
        app.refresh()
        return true
    }

    /// Moves the focused message to the account at `i` of the picker and closes. The core refuses an account that needs sign-in.
    @discardableResult
    func pick(_ i: Int) -> Bool {
        let list = accounts
        guard list.indices.contains(i), let id = app.shownMessageId else { return false }
        let to = list[i].id
        close()
        return app.perform(.moveToAccount(messageIds: [id], accountId: to))
    }
}
