import Foundation

// The iPhone's navigation as a value and a reducer (lane gap-phone-wire). No view, no core, no simulator: the stack the root draws is
// `PhoneNav.path`, the sheet over it is `PhoneNav.sheet`, and every way into a screen is one `PhoneNavAction`. Tests/Unit/PhoneRoutesTests.swift
// drives this file alone.
//
// The screens still name themselves by `AppModel.overlay` (the socket's view state, and what each lane's own door sets). The router
// (PhoneRouter.swift) feeds each such name to the reducer as `overlayChanged` and writes `PhoneNav.overlayName` back after a change,
// so the socket tree and the stack always say the same thing.

/// One screen of the stack. The Mailboxes screen is the stack's root and has no case: an empty path is Mailboxes.
enum PhoneRoute: Hashable, Sendable {
    case inbox
    case thread(String)          // the message the thread was opened on
    case settings
    case labelsContacts

    /// The `AppModel.overlay` word a screen sets while it is up; nil for the inbox (it is the surface, not an overlay).
    var overlayName: String? {
        switch self {
        case .inbox: nil
        case .thread: "iphone-thread"
        case .settings: "iphone-settings"
        case .labelsContacts: "iphone-labels-contacts"
        }
    }

    var surfaceName: String {
        switch self {
        case .inbox: "iphone-inbox"
        case .thread: "iphone-thread"
        case .settings: "iphone-settings"
        case .labelsContacts: "iphone-labels-contacts"
        }
    }
}

/// What opens over the stack as an iOS sheet.
enum PhoneSheet: String, Sendable {
    case composer = "iphone-composer"
    case palette = "iphone-palette"
}

struct PhoneNav: Equatable, Sendable {
    /// What is pushed on Mailboxes. The app launches on the inbox (J4.10 lands on "All Inboxes").
    var path: [PhoneRoute] = [.inbox]
    var sheet: PhoneSheet?
    /// The search is open in place in the inbox (under anything pushed above it).
    var searching = false

    /// The `AppModel.overlay` word for what is on top now: the sheet, else the top route's own word, else none.
    var overlayName: String? { sheet?.rawValue ?? path.last?.overlayName }

    /// The `AppModel.surface` word for what is on top now.
    var surfaceName: String {
        if let sheet { return sheet.rawValue }
        guard let top = path.last else { return "iphone-mailboxes" }
        if top == .inbox, searching { return "iphone-search" }
        return top.surfaceName
    }

    var top: PhoneRoute? { path.last }
}

enum PhoneNavAction: Equatable, Sendable {
    /// Mailboxes: All Inboxes, one account, "Show N selected". The inbox replaces whatever was pushed.
    case showInbox
    /// The inbox's Back: Mailboxes is the root again.
    case showMailboxes
    case openThread(String)
    case openSettings
    case openLabelsContacts
    case openComposer
    case openPalette
    case openSearch
    case closeSearch
    /// The sheet closes by itself (a swipe down, Cancel, the end of "Sent").
    case dismissSheet
    /// Back: the sheet first, then in-place search, then one push.
    case back
    /// A notification's tap: the thread opens over the inbox, whatever was on screen.
    case deepLinkThread(String)
    /// A screen set `AppModel.overlay` to this word (nil = it cleared it). `focused` is the inbox's focused message, which a thread names.
    case overlayChanged(String?, focused: String?)
    /// The system changed the stack by itself (a swipe back).
    case pathChanged([PhoneRoute])
    /// `fixture reset`: back on the inbox with nothing open.
    case reset
}

enum PhoneNavReducer {
    static func reduce(_ s: inout PhoneNav, _ a: PhoneNavAction) {
        switch a {
        case .showInbox:
            s.sheet = nil; s.path = [.inbox]
        case .showMailboxes:
            s.sheet = nil; s.searching = false; s.path = []
        case .openThread(let id):
            s.sheet = nil
            switch s.path.last {
            case nil: s.path = [.inbox, .thread(id)]          // a thread opened from nowhere sits over the inbox, never on Mailboxes
            case .thread?: s.path[s.path.count - 1] = .thread(id)
            default: s.path.append(.thread(id))
            }
        case .openSettings:
            s.sheet = nil
            if s.path.last != .settings { s.path.append(.settings) }
        case .openLabelsContacts:
            s.sheet = nil
            if s.path.last != .labelsContacts { s.path.append(.labelsContacts) }
        case .openComposer:
            s.sheet = .composer                              // from the palette too: the palette's sheet gives way
        case .openPalette:
            if s.sheet != .composer { s.sheet = .palette }
        case .openSearch:
            s.sheet = nil
            if let i = s.path.lastIndex(of: .inbox) { s.path = Array(s.path[...i]) } else { s.path = [.inbox] }
            s.searching = true
        case .closeSearch:
            s.searching = false
        case .dismissSheet:
            s.sheet = nil
        case .back:
            if s.sheet != nil { s.sheet = nil }
            else if s.searching, s.path.last == .inbox { s.searching = false }
            else if !s.path.isEmpty { s.path.removeLast() }
        case .deepLinkThread(let id):
            s.sheet = nil; s.searching = false
            s.path = [.inbox, .thread(id)]
        case .overlayChanged(let word, let focused):
            overlay(&s, word, focused: focused)
        case .pathChanged(let path):
            s.path = path
            if !path.contains(.inbox) { s.searching = false }
        case .reset:
            s = PhoneNav()
        }
    }

    /// A screen's own door set the overlay word. A word that names a route or a sheet becomes it; nil closes the sheet, else pops a screen
    /// that was named by a word (the inbox and Mailboxes are not overlays: a nil there changes nothing). Any other word has no screen here.
    private static func overlay(_ s: inout PhoneNav, _ word: String?, focused: String?) {
        guard let word else {
            if s.sheet != nil { s.sheet = nil } else if let top = s.path.last, top.overlayName != nil { s.path.removeLast() }
            return
        }
        let thread = "iphone-thread", settings = "iphone-settings", labels = "iphone-labels-contacts"
        switch word {
        case thread:
            if let focused { reduce(&s, .openThread(focused)) }
            else if case .thread? = s.path.last { /* already up */ }
            else { reduce(&s, .openThread("")) }
        case settings: reduce(&s, .openSettings)
        case labels: reduce(&s, .openLabelsContacts)
        case PhoneSheet.composer.rawValue: reduce(&s, .openComposer)
        case PhoneSheet.palette.rawValue: reduce(&s, .openPalette)
        default: break
        }
    }
}

/// The first window: the welcome, or the stack. Pure, so a test can run every case.
enum PhoneRoot {
    /// `welcomeDone`: the first-launch flow reached its last step. `midArrival`: the flow is on the arrival or notification step, where the
    /// accounts have just landed and the person has not finished. `testCopy`: the fixture core always has accounts, so a test copy follows the flow alone.
    static func showsWelcome(welcomeDone: Bool, midArrival: Bool, accounts: Int, testCopy: Bool) -> Bool {
        if welcomeDone { return false }
        if testCopy { return true }
        return accounts == 0 || midArrival
    }

    /// At launch: the welcome counts as done only on a real phone that said so AND still holds an account; a test copy always starts the flow.
    static func launchIsDone(flagDone: Bool, accounts: Int, testCopy: Bool) -> Bool {
        !testCopy && flagDone && accounts > 0
    }
}
