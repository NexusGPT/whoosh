import SwiftUI

/// The searchphone lane's registration: the phone search screen's rows, clicks, keys and the surface it draws.
/// ax ids follow `<surface>.<region>[.<element>...][.<index>]` (checks/searchphone-check.py).
@MainActor
public enum SearchPhoneFeature: FeatureModule {
    public static let id = "searchphone"
    public static let surfaces: [String] = ["iphone-search"]
    static var s: SearchPhoneModel { .shared }
    static let slot = SurfaceSlot("iphone-search")

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] { SearchPhoneRows.all }

    /// The screen drawn for the surface word `iphone-search`; the inbox's pill shows it in place of the list (docs/lanes/searchphone.md "Hand-over").
    public static func view(for slot: SurfaceSlot) -> AnyView? {
        slot == Self.slot ? AnyView(SearchPhoneScreen()) : nil
    }

    /// While naming a smart folder the typed text goes to the name, not to the search field.
    public static func typeText(_ text: String, _ m: AppModel) -> Bool {
        guard s.active, s.naming else { return false }
        s.name += text
        return true
    }

    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        [
            "Escape": { _ in
                guard s.active else { return false }
                if s.naming { return s.cancelNaming() }
                if s.menu != nil { return s.closeMenu() }
                return s.close()
            },
            "Enter": { _ in
                guard s.active, !s.naming else { return s.active && s.confirmName() }
                return s.submit()
            },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        s.settle()      // text typed a moment ago has its rows before a click names one of them
        var c: [String: (AppModel) -> Bool] = [
            "iphone-search.open": { _ in s.open() },
            "iphone-search.close": { _ in s.active && s.close() },
            "iphone-search.field.clear": { _ in s.active && s.clearText() },
            "iphone-search.field.submit": { _ in s.active && s.submit() },
            "iphone-search.filter.menu.close": { _ in s.closeMenu() },
            "iphone-search.filter.attachment": { _ in s.toggleAttachment() },
            "iphone-search.save": { _ in s.startSaving() },
            "iphone-search.more": { _ in s.toggleRest() },
            "iphone-search.skeleton.hold": { _ in s.hold() },
            "iphone-search.skeleton.release": { _ in s.release() },
            "iphone-search.save.confirm": { _ in s.confirmName() },
            "iphone-search.save.cancel": { _ in s.cancelNaming() },
            "iphone-search.meaning.download": { _ in s.pressDoor() },
        ]
        for m in SearchPhoneMenu.allCases { c["iphone-search.filter.\(m.rawValue)"] = { _ in s.toggleMenu(m) } }
        for i in s.domainOffers.indices { c["iphone-search.domain.\(i)"] = { _ in s.chooseDomain(i) } }
        for k in ["account", "sender", "date", "attachment", "domain"] { c["iphone-search.filter.\(k).clear"] = { _ in s.clearFilter(k) } }
        c["iphone-search.filter.account.all"] = { _ in s.setAccount(nil) }
        for a in PhoneAccounts.all(AppModel.shared) { c["iphone-search.filter.account.\(a.id)"] = { _ in s.setAccount(a.id) } }
        for i in 0..<s.senderNames.count { c["iphone-search.filter.sender.\(i)"] = { _ in s.setSender(i) } }
        for month in SearchPhoneFilters.months { c["iphone-search.filter.date.\(month.lowercased())"] = { _ in s.setMonth(month) } }
        for i in 0..<s.results.count { c["iphone-search.results.\(i)"] = { _ in s.openResult(i) } }
        for i in 0..<s.recents.count { c["iphone-search.recent.\(i)"] = { _ in s.useRecent(i) } }
        return c
    }
}
