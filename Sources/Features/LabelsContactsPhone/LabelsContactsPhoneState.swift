import Foundation
import Observation

/// One page of the labels-and-contacts stack: a label's page, the mail in that label, or one person.
/// The contact page is keyed by one address of the person; the page reads the whole person from the core.
enum PhonePage: Hashable, Sendable {
    case label(String)
    case mail(String)
    case contact(String)
}

/// The screen's one state object: the pushed pages and the failure of the last pin. Mail, labels and contacts stay in the core.
/// The doors into this screen (Mailboxes' Labels row, a sender's name in a thread) call `openList`, `openLabel`
/// and `openContact`; the stack, Back and the pin go through here so the socket and the real controls share one path.
@MainActor @Observable
final class LabelsContactsPhoneState {
    static let shared = LabelsContactsPhoneState()
    /// The surface id and the value of `AppModel.overlay` while this screen is up.
    static let surface = "iphone-labels-contacts"

    var path: [PhonePage] = []
    private(set) var pinFailure: String?
    /// Moves when a write lands, so the pages read the core again.
    private(set) var revision = 0

    private init() {}

    /// The label the open pages are about: the last label or mail page of the stack.
    var currentLabelId: String? {
        for page in path.reversed() {
            switch page {
            case .label(let id), .mail(let id): return id
            case .contact: continue
            }
        }
        return nil
    }

    var currentContactAddress: String? {
        if case .contact(let a)? = path.last { return a }
        return nil
    }

    func openList(in m: AppModel) { path = []; pinFailure = nil; m.overlay = Self.surface }

    @discardableResult
    func openLabel(_ id: String, in m: AppModel) -> Bool {
        guard m.core.labels().contains(where: { $0.id == id }) else { return false }
        path = [.label(id)]; pinFailure = nil; m.overlay = Self.surface
        return true
    }

    @discardableResult
    func openContact(address: String, in m: AppModel) -> Bool {
        guard m.core.contact(address: address) != nil else { return false }
        path = [.contact(address)]; m.overlay = Self.surface
        return true
    }

    func push(_ page: PhonePage) { pinFailure = nil; path.append(page) }

    /// Back pops one page; on the first page it leaves the screen.
    @discardableResult
    func back(_ m: AppModel) -> Bool {
        if path.isEmpty { m.overlay = nil } else { path.removeLast(); pinFailure = nil }
        return true
    }

    /// The only write on this screen. A refusal (a view is not a label, a core that says no) stays in place in the core's words.
    @discardableResult
    func setPinned(_ id: String, _ on: Bool, _ m: AppModel) -> Bool {
        if let words = ViewRefusal.words(for: .pinLabel(labelId: id, pinned: on)) { pinFailure = words; return false }
        guard m.perform(.pinLabel(labelId: id, pinned: on)) else {
            pinFailure = m.lastFailure?.reasonWords ?? Words.text("That label is gone.")
            return false
        }
        pinFailure = nil; revision += 1
        return true
    }

    /// A message of a list opens its thread, the way the inbox row does.
    func openThread(_ messageId: String, in m: AppModel) -> Bool {
        m.focus(messageId: messageId); m.overlay = "iphone-thread"
        return true
    }

    func touch() { revision += 1 }
}
