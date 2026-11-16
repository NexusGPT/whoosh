import Foundation
import Observation

/// One account as the pane shows it. Name, address, provider, colour, icon id, group, paused and backfill are the core's
/// `AccountSummary` fields; a pick made in this pane sits over them until the core can keep it (see `AccountsDoor`).
struct AccountEntry: Identifiable, Equatable {
    let id: String
    var name: String
    var address: String
    var provider: String
    var colour: String
    var iconId: String?
    var imagePath: String?
    var group: String?
    var paused: Bool = false
    /// The share of older mail read so far (0 to 1); nil when the history has not started.
    var backfill: Double?
    var signature = ""
    /// The mail service the core's route puts the account under (nil: none of the five the pane reports on).
    var service: MailService?
    var historyDone: Bool { (backfill ?? 0) >= 1 }
}

/// The pane's state. Every input lands on its own frame (docs/MOTION.md law 1); the views only animate after.
///
/// Reads: `core.accounts()` (group, paused, backfill, health words) and `core.syncStates()` (the server's own words).
/// Writes: sign in again goes through the core (`reauth` on the fixture core, the signin lane on the real one). The
/// other four (notification, icon, image, colour) have no core command yet: `AccountsDoor` names what is missing and the
/// change lives for this session. Docs: docs/lanes/settingsaccountsmac.md "## Round 2".
@MainActor @Observable
final class AccountsSession {
    static let shared = AccountsSession()

    private(set) var accounts: [AccountEntry] = []
    private(set) var states: [String: SyncState] = [:]
    /// The account whose icon and colour are open in the picker.
    var editing: String?
    /// What the owner typed in the find field; empty shows every account.
    var query = ""
    /// A short message under one account's row ("That image is too big").
    private(set) var notes: [String: String] = [:]
    /// The reason the last change could not be kept, in words; nil when it could.
    private(set) var failure: String?
    /// The Add account sheet is open over the Settings window (⌘N, the header button, the socket).
    var adding = false
    /// The Import mail sheet is open (the button in the header, the socket).
    var importing = false
    /// The rig's stand-in for opening System Settings at Full Disk Access (a test never opens it): set by the door, read by `rows ...import.sheet`.
    var fdaOpened = false
    /// Open the sheet on a clean scan of this Mac; it shares the first start's model.
    func openImport() { ImportModel.shared.openFromSettings(); importing = true }
    func closeImport() { _ = ImportModel.shared.leave(AppModel.shared); importing = false }
    /// The account whose removal is being reviewed (Remove account… opens the review, nothing goes before it is confirmed).
    var removing: String?
    /// The text field the socket's `type` verb writes to: set by the click door on a field (a real keystroke goes to the field itself).
    enum Field: Equatable { case name(String), signature(String), find }
    var fieldFocus: Field?
    /// What is typed in an account page's name and signature fields and not yet saved; a page drops its drafts when it closes.
    private(set) var nameDrafts: [String: String] = [:]
    private(set) var signatureDrafts: [String: String] = [:]

    init() { reload() }

    /// True while the Accounts pane is on screen: only then does a core change re-read the list (nothing is read for a pane nobody sees).
    var shown = false

    /// The core changed (`AppModel.refresh`, which the core's own events reach through `CoreChanges` and the fixture verbs through `m.refresh()`): when the pane is on screen
    /// and the core lists other accounts than the pane holds, the pane reloads. No timer, no polling: the event is the trigger.
    func followCore() {
        guard shown else { return }
        let ids = CoreHost.shared.core.accounts().map(\.id)
        if ids != accounts.map(\.id) { reload() }
    }

    func reload() {
        accounts = CoreHost.shared.core.accounts().map { a in
            AccountEntry(id: a.id, name: AccountNames.display(a), address: a.address, provider: AccountsWords.provider(a.provider),
                         colour: a.colour, iconId: a.iconFile != nil ? nil : a.iconId, imagePath: a.iconFile, group: a.group,
                         paused: a.paused, backfill: a.backfill, signature: a.signature,
                         service: MailService.of(provider: a.provider, address: a.address))
        }
        refreshStates()
    }

    func refreshStates() {
        states = Dictionary(uniqueKeysWithValues: CoreHost.shared.core.syncStates().map { ($0.accountId, $0) })
    }

    /// The accounts the find field lets through, in the core's order.
    var visible: [AccountEntry] {
        let q = query.trimmingCharacters(in: .whitespaces).lowercased()
        return q.isEmpty ? accounts : accounts.filter { $0.name.lowercased().contains(q) || $0.address.lowercased().contains(q) }
    }

    /// The visible accounts under their group headers, in the order each group first appears; ungrouped last.
    /// With no group at all there is one section and no header.
    var sections: [(title: String?, accounts: [AccountEntry])] {
        let v = visible
        var order: [String] = []
        for a in v { if let g = a.group, !order.contains(g) { order.append(g) } }
        if order.isEmpty { return v.isEmpty ? [] : [(nil, v)] }
        var out: [(title: String?, accounts: [AccountEntry])] = order.map { g in (g, v.filter { $0.group == g }) }
        let rest = v.filter { $0.group == nil }
        if !rest.isEmpty { out.append((AccountsWords.otherGroup, rest)) }
        return out
    }

    func entry(_ id: String) -> AccountEntry? { accounts.first { $0.id == id } }
    /// The account's notification setting as the core keeps it; an account with none notifies, on both the notch and the system.
    func setting(_ id: String) -> NotificationSetting {
        CoreHost.shared.core.notificationSettings().first { $0.accountId == id } ?? NotificationSetting(accountId: id, enabled: true, delivery: .both)
    }
    func isMuted(_ id: String) -> Bool { !setting(id).enabled }
    func state(_ id: String) -> SyncState? { states[id] }
    /// The "Mail services" rows: this Mac's own accounts per service, read from the same states as the rest of the pane.
    var serviceRows: [ServiceStatusRow] { ServiceStatusRow.rows(accounts: accounts, states: states) }
    var failing: [AccountEntry] { accounts.filter { states[$0.id]?.lastError != nil } }

    /// What the row says about the account's mail: the server's words when it is failing.
    func syncLine(_ id: String) -> String {
        guard let s = states[id] else { return AccountsWords.neverSynced }
        // sync-words: a status or failure that carries a sync code reads as its catalogue sentence; the code tail never shows raw
        if let e = s.lastError { return SyncStatusWords.failure(code: e.code, words: e.reasonWords)?.sentence ?? AccountsWords.sentence(e.reasonWords) }
        if !s.historyFetched { return AccountsWords.syncing }
        if let st = SyncStatusWords.status(s.providerStatusWords) { return st.sentence }
        return AccountsWords.sentence(s.providerStatusWords ?? "synced")
    }

    // MARK: writes (each is a core command; a refusal reads in the core's words)

    /// Runs one command; `failure` holds the core's words when it refused, nil when it went through. The list is read again either way.
    @discardableResult
    private func run(_ command: Command) -> Bool {
        let ok = AppModel.shared.perform(command)
        failure = ok ? nil : (AppModel.shared.lastFailure?.reasonWords ?? Words.text("Not saved"))
        reload()
        AccountMarkRevision.shared.bump()   // the list's account directory re-reads the core in this same update (RowFacts.swift)
        return ok
    }

    @discardableResult
    func toggleNotifications(_ id: String) -> Bool {
        guard entry(id) != nil else { return false }
        var st = setting(id)
        st.enabled.toggle()
        run(.setNotificationSetting(setting: st))
        return true
    }

    @discardableResult
    func setDelivery(_ d: NotificationDelivery, for id: String) -> Bool {
        guard entry(id) != nil else { return false }
        var st = setting(id)
        st.delivery = d
        return run(.setNotificationSetting(setting: st))
    }

    @discardableResult
    func pickGlyph(_ glyphId: String, for id: String) -> Bool {
        guard entry(id) != nil else { return false }
        notes[id] = nil
        run(.setAccountIcon(accountId: id, source: .library(iconId: glyphId)))
        return true
    }

    @discardableResult
    func pickColour(_ hex: String, for id: String) -> Bool {
        guard entry(id) != nil else { return false }
        run(.setAccountColour(accountId: id, colour: hex))
        return true
    }

    func setImage(path: String, for id: String) {
        guard entry(id) != nil else { return }
        notes[id] = nil
        if !run(.setAccountIcon(accountId: id, source: .uploaded(filePath: path))), let why = failure { notes[id] = why }
    }

    @discardableResult
    func togglePause(_ id: String) -> Bool {
        guard let e = entry(id) else { return false }
        run(.pauseAccount(accountId: id, paused: !e.paused))
        return true
    }

    @discardableResult
    func rename(_ id: String, to name: String) -> Bool {
        let n = name.trimmingCharacters(in: .whitespaces)
        guard entry(id) != nil, !n.isEmpty else { return false }
        return run(.renameAccount(accountId: id, name: n))
    }

    @discardableResult
    func setSignature(_ id: String, to text: String) -> Bool {
        guard entry(id) != nil else { return false }
        return run(.setSignature(accountId: id, text: text))
    }

    /// What removing the account takes with it, in the core's words (read before anything goes).
    func removalWords(_ id: String) -> String {
        CoreHost.shared.core.accountRemoval(accountId: id)?.words ?? AccountsWords.removeLine
    }

    /// Asks for the removal review; the account is not touched until `confirmRemove`.
    @discardableResult func askRemove(_ id: String) -> Bool { guard entry(id) != nil else { return false }; removing = id; return true }
    @discardableResult
    func confirmRemove() -> Bool {
        guard let id = removing, entry(id) != nil else { return false }
        removing = nil
        let ok = run(.removeAccount(accountId: id))
        if ok, SettingsWindowModel.shared.accountPage == id { _ = SettingsWindowModel.shared.closeAccount() }
        return ok
    }

    func setNameDraft(_ text: String, for id: String) { nameDrafts[id] = text }
    func setSignatureDraft(_ text: String, for id: String) { signatureDrafts[id] = text }
    /// The page closed: its drafts go, and so does a focus on its own fields. The find field belongs to the list, which the page's closing
    /// (a frame later than the click that closed it) must not take focus from.
    func clearDrafts() {
        nameDrafts = [:]; signatureDrafts = [:]
        switch fieldFocus { case .name?, .signature?: fieldFocus = nil; case .find?, nil: break }
    }

    /// Return in the name field: saves what was typed there (nothing typed is nothing to save).
    @discardableResult func submitName(_ id: String) -> Bool {
        guard let text = nameDrafts[id] else { return false }
        return rename(id, to: text)
    }
    /// Return in the signature field.
    @discardableResult func submitSignature(_ id: String) -> Bool {
        guard let text = signatureDrafts[id] else { return false }
        return setSignature(id, to: text)
    }

    /// Text the socket's `type` verb sends while a door has focused one of the fields: the field takes it in the same turn.
    /// False when no field is focused, so the text goes on to the next feature and the shared buffer.
    func typeIntoField(_ text: String) -> Bool {
        switch fieldFocus {
        case .name(let id)?: nameDrafts[id] = (nameDrafts[id] ?? entry(id)?.name ?? "") + text
        case .signature(let id)?: signatureDrafts[id] = (signatureDrafts[id] ?? entry(id)?.signature ?? "") + text
        case .find?: query += text
        case nil: return false
        }
        return true
    }

    func refuse(_ words: String, for id: String) { notes[id] = words }
    /// The refusal under an account's row, if any (the picker shows it beside the upload).
    func refusal(_ id: String) -> String? { notes[id] }
    func clearNote(_ id: String) { notes[id] = nil }

    /// The row's one-line note: a refusal first, else "no notifications" when muted, else empty.
    func note(_ id: String) -> String { notes[id] ?? (isMuted(id) ? AccountsWords.noNotifications : "") }

    /// Sign in again. On the fixture core this stands for a successful sign-in (`reauth`); on the real core it
    /// opens the provider's sign-in through the signin lane.
    @discardableResult
    func signInAgain(_ id: String, model: AppModel) -> Bool {
        guard let e = entry(id) else { return false }
        if let fixture = CoreHost.shared.fixture {
            let ok = fixture.reauth(accountId: id)
            reload()
            return ok
        }
        SignInController.shared.setAddress(e.address)
        let provider: SignInProvider? = switch SignInController.provider(forAddress: e.address) {
            case .microsoft: .microsoft
            case .fastmail: .fastmail
            case .google: .google
            default: nil }
        guard let provider else { model.surface = "mac-welcome-accounts"; return true }
        return SignInController.shared.beginMailSignIn(provider, model: model)
    }
}
