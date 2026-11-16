import Foundation
import Observation

/// Holds the iPhone's `PhoneNav` and keeps it, `AppModel.overlay`/`surface` and the screens' own states saying the same thing.
/// Every door (a row tap, the Mailboxes rows, the bar's Compose, the notification's tap, the socket's clicks) ends in `send`.
/// The reducer is `PhoneNavReducer` (PhoneRoutes.swift); this class adds only the writes the reducer must not make (a pure value
/// cannot touch `AppModel`). It does no work of its own on a tap: one reduce, two string compares.
@MainActor @Observable
final class PhoneRouter {
    static let shared = PhoneRouter()

    private(set) var nav = PhoneNav()
    private static let searchWord = "iphone-search"
    @ObservationIgnored private var resetToken: NSObjectProtocol?

    private var app: AppModel { .shared }

    private init() {
        #if os(iOS)
        PushClientRig.startForPhone()   // the test copy's arrival router with the recording poster (J7.1 lock-screen rows); nothing on a real phone
        #endif
        // `fixture reset` puts the view state back (FixtureEvents.reset clears overlay and surface itself); the stack follows it.
        // queue nil: the block runs on the posting thread, at once. `queue: .main` queued it for a later turn of the run loop, so the doors a check clicks right after
        // `fixture reset` (back, Settings, the Sync row) could land first and the late reset then popped the stack to Mailboxes (phone8: phone-shots sync-broken/light, the
        // screen `iphone-mailboxes` with the state naming `iphone-settings`; the same state passed in dark). FixtureEvents posts on the main actor, so assumeIsolated holds.
        resetToken = NotificationCenter.default.addObserver(forName: FixtureEvents.name("reset"), object: nil, queue: nil) { _ in
            MainActor.assumeIsolated {
                let r = PhoneRouter.shared
                r.send(.reset)
                // `FixtureEvents.reset` has just written overlay nil and surface `mac-main`; `send` returns early when the stack did not move (a reset on the inbox), so the
                // words the socket reads stayed `mac-main` while the screen showed the inbox (phone5: J7.1 `surface=mac-main`, phone-shots `iphone-inbox/list/light`). The stack is the truth.
                if r.app.overlay != r.nav.overlayName { r.app.overlay = r.nav.overlayName }
                if r.app.surface != r.nav.surfaceName { r.app.surface = r.nav.surfaceName }
            }
        }
    }

    // MARK: the one entry

    func send(_ action: PhoneNavAction) {
        let before = nav
        PhoneNavReducer.reduce(&nav, action)
        guard nav != before else { return }
        settle(from: before)
    }

    /// What the reducer cannot do: tell the socket's view state and the screens that a screen left or arrived.
    private func settle(from before: PhoneNav) {
        // A step that marks the search closed (system Back from the inbox, Mailboxes, Back, a notification's deep link) also closes the search itself:
        // else the inbox draws it again on the way back. First, so the surface and overlay written below are this step's own (hid-search round 14 finding).
        if before.searching, !nav.searching, SearchPhoneModel.shared.active { _ = SearchPhoneModel.shared.close() }
        for route in before.path where !nav.path.contains(route) { leave(route) }
        if app.overlay != nav.overlayName { app.overlay = nav.overlayName }
        if nav.sheet == nil, before.path != nav.path || before.searching != nav.searching || before.sheet != nil {
            if app.surface != nav.surfaceName { app.surface = nav.surfaceName }
        }
    }

    /// A screen the stack dropped (system Back, a deep link over it) forgets what it held, as its own Back does.
    private func leave(_ route: PhoneRoute) {
        switch route {
        case .inbox: break
        case .thread: ThreadPhoneState.shared.dismissed()
        case .settings: SettingsPhoneState.shared.path = []
        case .labelsContacts: LabelsContactsPhoneState.shared.path = []
        }
    }

    // MARK: the doors the views call

    /// A tap on the inbox row at `index`: the thread's own door (it freezes the message and sets the overlay), then the push at once.
    @discardableResult
    func openThread(row index: Int) -> Bool {
        guard ThreadPhoneState.shared.open(row: index, in: app) else { return false }
        send(.overlayChanged(ThreadPhoneState.overlayId, focused: ThreadPhoneState.shared.target(app)))
        return true
    }

    func openSettings() { send(.openSettings); SettingsPhoneState.shared.path = [] }

    func openLabelsContacts() { LabelsContactsPhoneState.shared.openList(in: app); send(.openLabelsContacts) }

    func compose() { ComposerPhoneFeature.compose(app); send(.openComposer) }

    func reply(messageId: String) { ComposerPhoneFeature.reply(app, messageId: messageId); send(.openComposer) }

    func openSearch() { SearchPhoneModel.shared.open(); send(.openSearch) }

    func showInbox() { send(.showInbox) }
    func showMailboxes() { send(.showMailboxes) }

    /// A notification's tap, or a `whoosh://message/<id>` link: the thread opens over the inbox. The message is made the focused row first
    /// (on the tab that holds it), so the thread screen, which reads the focused row, shows this message and no other.
    @discardableResult
    func notificationTapped(messageId: String) -> Bool {
        guard !messageId.isEmpty else { return false }
        ThreadPhoneState.shared.dismissed()
        if !app.rows.contains(where: { $0.messageId == messageId }) {
            app.refresh()
            if !app.rows.contains(where: { $0.messageId == messageId }),
               let tab = app.tabIds.first(where: { t in app.core.inbox(query: InboxQuery(tabId: t, accountId: nil, labelId: nil)).contains { $0.messageId == messageId } }) {
                app.tab = tab; app.accountFilter = nil; app.refresh()
            }
        }
        guard app.rows.contains(where: { $0.messageId == messageId }) else { return false }
        app.focus(messageId: messageId)
        send(.deepLinkThread(messageId))
        return true
    }

    @discardableResult
    func open(url: URL) -> Bool {
        guard let id = PhoneDeepLink.messageId(url) else { return false }
        return notificationTapped(messageId: id)
    }

    // MARK: what the views report

    /// The inbox is the top screen with no sheet over it (where its Compose and Search controls are drawn).
    var onInbox: Bool { nav.sheet == nil && nav.top == .inbox }
    /// Mailboxes is the top screen (the root, where its Settings and Labels rows are drawn).
    var onMailboxes: Bool { nav.sheet == nil && nav.path.isEmpty }

    /// `AppModel.overlay` changed (a screen's own door, the socket).
    func overlayChanged() {
        // The palette's Search command names the search by this word; the search is not an overlay but the inbox's own state, so open it.
        if app.overlay == Self.searchWord { openSearch(); return }
        send(.overlayChanged(app.overlay, focused: app.focusedMessageId))
    }

    /// The in-place search opened or closed by its own model (the socket's `iphone-search.open`, its close).
    /// The view's `onChange` of the search model's `active` reaches the router a turn AFTER the door that opened the search. A value the router already holds is dropped:
    /// `.openSearch` trims the stack to the inbox, so a late one popped the Settings a second door had just pushed over the open search (phone7: Search door, then Settings
    /// door, the palette's close showed `iphone-search`).
    func searchChanged(active: Bool) {
        guard active != nav.searching else { return }
        send(active ? .openSearch : .closeSearch)
    }

    /// The system moved the stack (a swipe back).
    func pathChanged(_ path: [PhoneRoute]) { send(.pathChanged(path)) }

    /// A sheet's own dismissal (a swipe down). The composer closes through its own state, which refuses while a send counts down.
    func sheetDismissed(_ sheet: PhoneSheet) {
        guard nav.sheet == sheet else { return }
        if sheet == .composer, ComposerModel.shared.isCounting { return }   // a send in its undo window keeps its sheet
        switch sheet {
        case .composer: ComposerPhoneState.shared.close(app)
        case .palette: PalettePhoneModel.shared.close()
        }
        send(.dismissSheet)
    }
}

/// `whoosh://message/<id>`: the Mac's link scheme (MacShellLinks.scheme). The id is everything after the last slash.
enum PhoneDeepLink {
    static func messageId(_ url: URL) -> String? {
        guard url.scheme == "whoosh", url.host == "message" else { return nil }
        let id = url.pathComponents.filter { $0 != "/" }.first ?? ""
        return id.isEmpty ? nil : id
    }
}
