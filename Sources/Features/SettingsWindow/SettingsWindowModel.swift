import Foundation
import Observation

/// Where the Settings window is: the pane, and the account page inside Accounts. Back and forward walk the places
/// visited (⌘[ and ⌘]). Every input lands on its own frame; the views animate after (docs/MOTION.md law 1).
@MainActor @Observable
final class SettingsWindowModel {
    static let shared = SettingsWindowModel()

    struct Place: Hashable { var pane: SettingsPane; var account: String? = nil; var reply: RepliesPage? = nil }

    private(set) var place = Place(pane: .general)
    private(set) var back: [Place] = []
    private(set) var forward: [Place] = []
    /// How far the last pane change moved, in rows: a short move slides the selection, a long one fades (docs/MOTION.md rule 11).
    private(set) var lastDistance = 0
    /// Réponses lists its six parts only while it is open. Open while Réponses (or one of its parts) is the selected pane, unless the chevron
    /// closed it; the chevron can also open it beside another selected pane. Any move to another pane closes it: it is never remembered.
    private(set) var repliesOpen = false
    /// The height of the title bar band the window draws over its full-size content (read from the window, 0 while none is drawn). The detail
    /// column starts below it: the toolbar row sits under the band otherwise and only its lowest few points show.
    private(set) var titlebarHeight: CGFloat = 0
    func setTitlebar(_ h: CGFloat) { if abs(h - titlebarHeight) > 0.5 { titlebarHeight = h } }
    /// The row a search result points at: ringed in blue until the next move.
    private(set) var ring: String?
    /// What the owner typed in the search field; empty shows the panes.
    private(set) var query = ""
    private(set) var selectedResult = 0
    /// Counts the requests to put the cursor in the search field (⌘F).
    private(set) var searchRequests = 0
    private var searchWanted = false
    private var typedMark = 0

    var pane: SettingsPane { place.pane }
    var accountPage: String? { place.account }
    var canGoBack: Bool { !back.isEmpty }
    var canGoForward: Bool { !forward.isEmpty }
    var results: [SettingsEntry] { SettingsSearch.results(for: query) }
    var searching: Bool { !query.trimmingCharacters(in: .whitespaces).isEmpty }

    // MARK: moves

    @discardableResult
    func go(_ p: Place) -> Bool {
        guard p != place else { return false }
        back.append(place)
        forward.removeAll()
        move(to: p)
        return true
    }
    @discardableResult func select(_ pane: SettingsPane) -> Bool { ring = nil; if pane == .replies { repliesOpen = true }; return go(Place(pane: pane, reply: pane == .replies ? .auto : nil)) }
    /// A part of Réponses (the sidebar's child rows, a link inside a pane). Back and forward walk the parts.
    @discardableResult func openReply(_ p: RepliesPage) -> Bool { ring = nil; repliesOpen = true; return go(Place(pane: .replies, reply: p)) }
    @discardableResult func openAccount(_ id: String) -> Bool { ring = nil; return go(Place(pane: .accounts, account: id)) }
    /// The account page's way home: one step up, not a step back through history.
    @discardableResult func closeAccount() -> Bool { guard place.account != nil else { return false }; return go(Place(pane: .accounts)) }

    @discardableResult
    func goBack() -> Bool {
        guard let p = back.popLast() else { return false }
        forward.append(place)
        ring = nil
        move(to: p)
        return true
    }
    @discardableResult
    func goForward() -> Bool {
        guard let p = forward.popLast() else { return false }
        back.append(place)
        ring = nil
        move(to: p)
        return true
    }

    /// The chevron of the Réponses row: opens or closes its parts without changing the selected pane.
    func toggleReplies() { repliesOpen.toggle() }

    private func move(to p: Place) {
        if p.pane != .replies { repliesOpen = false } else if place.pane != .replies { repliesOpen = true }   // back and forward into Réponses open it
        let all = SettingsPane.allCases
        lastDistance = abs((all.firstIndex(of: p.pane) ?? 0) - (all.firstIndex(of: place.pane) ?? 0))
        place = p
        AccountsSession.shared.fieldFocus = nil   // a field a door focused belongs to the page it was on
        FormFitLog.shared.reset()
        mirrorToApp()
    }

    /// The app model still answers to four page slots (the control socket, `shot`, the palette).
    private func mirrorToApp() {
        let app = AppModel.shared
        if let slot = place.pane.slot { app.settingsPage = slot }
        app.surface = (place.pane.slot ?? app.settingsPage).id
    }

    /// The page a slot names was asked for from outside (the palette, the notch, the status line): show its pane.
    func follow(slot: SurfaceSlot) {
        guard let want = SettingsPane.pane(for: slot), place.pane.slot != slot else { return }
        _ = select(want)
    }

    // MARK: search

    func setQuery(_ q: String) {
        query = q
        selectedResult = 0
        ring = nil
        if searching, let first = results.first { reveal(first) }
    }
    func clearQuery() { query = ""; selectedResult = 0; ring = nil; searchWanted = false }
    func moveSelection(_ delta: Int) {
        let r = results
        guard !r.isEmpty else { return }
        selectedResult = min(max(selectedResult + delta, 0), r.count - 1)
        reveal(r[selectedResult])
    }
    /// Opens the pane of a found setting and rings its row.
    func reveal(_ e: SettingsEntry) {
        _ = go(Place(pane: e.pane))
        ring = e.ax
    }
    /// A click on a result: the pane opens, the row is ringed, the panes come back.
    func open(_ e: SettingsEntry) { reveal(e); query = ""; selectedResult = 0 }
    /// Return in the search field: the pane stays, the panes come back, the row stays ringed.
    func commitSearch() {
        let r = results
        guard !r.isEmpty else { return }
        let e = r[min(selectedResult, r.count - 1)]
        reveal(e)
        query = ""
        selectedResult = 0
    }
    func requestSearch() {
        searchRequests += 1
        searchWanted = true
        AccountsSession.shared.fieldFocus = nil   // the search field takes the typed text now
        typedMark = AppModel.shared.typed.count
    }
    /// The Settings window holds the keys only while one of its surfaces is current; closed, or with the keys elsewhere, it takes nothing typed.
    static func holdsKeys(_ m: AppModel) -> Bool { m.surface.hasPrefix("mac-settings") }
    /// The search field stops being the target of the shared buffer (the window closed, or the keys went to another surface).
    func releaseTyped() { searchWanted = false }
    /// Text the socket's `type` verb added while the search field is the target: it goes into the field.
    func absorbTyped(_ typed: String) {
        guard searchWanted, typed.count > typedMark else { return }
        let added = String(typed.dropFirst(typedMark))
        typedMark = typed.count
        setQuery(query + added)
    }
}
