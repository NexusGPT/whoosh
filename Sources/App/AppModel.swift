import Foundation
import Observation

/// The UI state the shell owns: which surface, which tab, which row has focus. Data comes from
/// the core only (`CoreHost.shared.core`); this model never holds a second copy of a count.
@MainActor @Observable
public final class AppModel {
    public static let shared = AppModel()

    public let host = CoreHost.shared
    public var core: WhooshCore { host.core }
    public var surface = "mac-main"
    public var tab = "primary"
    public var accountFilter: String?
    /// Which mailbox the list shows (sidebar click, G then a key). nil = the inbox with its tab strip; a value ignores `tab`
    /// (InboxQuery.mailbox, delta 9). Found 2026-11-09: the sidebar highlighted Starred, Drafts or Sent while the list kept
    /// the inbox rows, because nothing wrote this field.
    public var mailbox: Mailbox? { didSet { if mailbox != oldValue { focusIndex = 0; refresh() } } }
    /// Which saved smart folder the list shows (sidebar click). nil = none; a value shows that folder's results and ignores
    /// `tab` and `mailbox` (contract delta 14 adds InboxQuery.smart_folder_id; until then the list filters by the saved query).
    public var smartFolderId: String? { didSet { if smartFolderId != oldValue { focusIndex = 0; refresh() } } }
    /// The unpinned label the list is filtered by (sidebar click, `LabelFilter`, docs/PRD.md A107). nil = none. The core reads the rows by it
    /// (`InboxQuery.labelId`, on the All view), so the list holds every row the label carries, not one page of the All view filtered here.
    public var labelId: String? { didSet { if labelId != oldValue { focusIndex = 0; refresh() } } }
    public var overlay: String?
    /// the Settings window's page (⌘,); `shot mac-settings-…` moves it
    public var settingsPage = SurfaceSlot.macSettingsGeneral
    public var focusIndex = 0
    /// The shared text buffer of the fields that do not own their text (search, palette, the labels screen's names).
    /// A field that owns its text (the quick reply's draft) takes `type` itself (FeatureModule.typeText).
    public var typed = ""
    public var lastFailure: ActionFailed? { didSet { if lastFailure != nil { failures += 1 } } }
    /// Counts failed actions, so the key chain can tell "not mine" (pass on) from "tried and failed" (stop).
    public private(set) var failures = 0
    public private(set) var rows: [InboxRow] = []
    /// How many rows the list asks the core for. It starts at one page (LIST_PAGE_ROWS) and gains a page each time the owner nears the
    /// end of the rows it holds (`wantMoreRows`); another tab, account, mailbox, label or smart folder starts again at one page.
    /// Owner's store, 14 Nov 2026: a tab of 178 conversations showed 50, said "End of All", and Down stopped on the 50th.
    public private(set) var rowBudget = UInt32(WhooshConstants.LIST_PAGE_ROWS)
    @ObservationIgnored private var rowScope = ""
    public private(set) var counts = Counts(tabs: [], mailboxes: [:])
    /// Moves when a refresh changed the list's rows or the counts. A view that draws figures it reads from the core itself (a label's unread) reads this,
    /// so it is drawn again when the mail changes: the iPhone's labels pages. (The Mac sidebar is told one turn later, off the key's frame: `mailChanged`.)
    public private(set) var mailRevision = 0
    public var launchMs: Double?
    /// Process start to the first presented frame, in ms: taken on the main queue's next turn after the first
    /// RootView appears (the frame the appearance produced has been handed over by then). `measure mac-main.first-frame`.
    public var firstFrameMs: Double?

    private init() {
        // The app draws its first frame from an empty model: the first read and the change subscription run in `settle()`, on the
        // main queue's turn after the first frame (RootView, AppLaunch). A unit test has no window and so no such turn: it keeps
        // the eager read, which is what its assertions were written against.
        if ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil { settle() }
        LaunchClock.trace("appmodel-init-end")
    }

    @ObservationIgnored private var settled = false

    /// The first read of the core and the list's subscription to the core's own changes (Sources/App/CoreChanges.swift). Once.
    /// The app runs it right after `firstFrameMs` is set, in the same main-queue turn and before any control verb can run
    /// (the socket opens after the turn is queued), so a drive never reads the empty first frame as the inbox.
    public func settle() {
        guard !settled else { return }
        settled = true
        refresh()
        LaunchClock.trace("settled-refresh-end")
        CoreChanges.shared.start(core: host.core)
        LaunchClock.trace("settled-changes-end")
    }

    public var tabIds: [String] { host.fixture?.tabIds ?? counts.tabs.map(\.tabId) }

    public var focusedMessageId: String? {
        rows.indices.contains(focusIndex) ? rows[focusIndex].messageId : nil
    }

    /// Results are on the list pane and have answered: the main window is about the selected result, not about an inbox row.
    var resultsShow: Bool {
        let s = SearchMacModel.shared
        return s.showsResults && !s.loading
    }

    /// The message the main window is about: the selected search result while results show, else the focused inbox row.
    /// The reader draws it and every action on "the mail" acts on it (docs/SPEC.md A138, docs/lanes/search-actions.md).
    public var shownMessageId: String? { shownRow?.messageId }

    /// The ONE lookup for "the mail the main window is about": the selected result's own row while results show (a result may not be
    /// among `rows`), else the focused inbox row. Reads what is already in memory; nil when nothing is selected.
    public var shownRow: InboxRow? {
        if resultsShow { return SearchMacModel.shared.selectedRow }
        return rows.indices.contains(focusIndex) ? rows[focusIndex] : nil
    }

    /// The row of a mail the main window may show, by its id (docs/SPEC.md A148): the shown row, else the inbox row, else, while the search is
    /// open, the result's own row (a result is often in no loaded inbox row: another tab, Sent, Archive, past the tab's first page). The reader
    /// builds its thread from this and never from `rows` alone. Reads memory only; nil when the window holds no row with that id.
    public func knownRow(_ messageId: String) -> InboxRow? {
        // The shown row, read WITHOUT `focusIndex` (perf-archive): with no results the shown row is `rows[focusIndex]`, the one row of `rows`
        // with this id, which the line below finds by id; so the reader pane, which builds its thread through here, no longer observes the
        // focus index and is not rebuilt by a passing J or K while ReaderFocus holds the shown mail.
        if resultsShow, let r = SearchMacModel.shared.selectedRow, r.messageId == messageId { return r }
        if let r = rows.first(where: { $0.messageId == messageId }) { return r }
        // the iPhone search's result (another tab, Sent, past the first page) is in no loaded row either: its own row opens its thread (item 9f990234)
        if let r = SearchPhoneModel.shared.everyResult.first(where: { $0.row.messageId == messageId })?.row { return r }
        let s = SearchMacModel.shared
        guard s.showsResults else { return nil }
        return s.results.first { $0.row.messageId == messageId }?.row
    }

    public func refresh() {
        // Assign only what changed: the core's own events (CoreChanges) call this often, and an equal value must not redraw the list.
        let t0 = InteractionProbe.phaseStart()
        let scope = "\(tab)|\(accountFilter ?? "")|\(labelId ?? "")|\(mailbox.map { String(describing: $0) } ?? "")|\(smartFolderId ?? "")"
        if scope != rowScope { rowScope = scope; rowBudget = UInt32(WhooshConstants.LIST_PAGE_ROWS) }
        let fresh = core.inboxRows(query: InboxQuery(tabId: tab, accountId: accountFilter, labelId: labelId, mailbox: mailbox, accountIds: [], filters: [], smartFolderId: smartFolderId), rows: rowBudget)
        var changed = false
        if fresh != rows { rows = fresh; changed = true }
        InteractionProbe.phaseEnd(.refreshInbox, t0)
        let t1 = InteractionProbe.phaseStart()
        let freshCounts = core.counts()
        if freshCounts != counts { counts = freshCounts; changed = true }
        InteractionProbe.phaseEnd(.refreshCounts, t1)
        if changed { mailRevision &+= 1; SidebarState.shared.mailChanged() }   // the label counts follow the mail (they are read from the core untracked)
        if focusIndex >= rows.count { focusIndex = max(0, rows.count - 1) }
        AccountsSession.shared.followCore()   // Settings > Accounts follows an account the core gained or lost while it is open (event-driven: refresh is what the core's events call)
    }

    /// The owner is near the end of the rows the list holds (the focus, or a row scrolled into view): one more page. A list that holds
    /// fewer rows than it asked for holds everything, and nothing is read.
    public func wantMoreRows() {
        guard rows.count >= Int(rowBudget) else { return }
        rowBudget += UInt32(WhooshConstants.LIST_PAGE_ROWS)
        refresh()
    }

    /// True when row `index` is among the last LIST_ROWS_AHEAD rows the list holds.
    public func nearListEnd(_ index: Int) -> Bool { index >= rows.count - Int(WhooshConstants.LIST_ROWS_AHEAD) }

    /// Runs one command; a failure is kept in words for the status line, never thrown at a view.
    @discardableResult
    public func perform(_ command: Command) -> Bool {
        do {
            if host.kind == .fixture, let reason = FixtureFaults.take(FixtureFaults.op(of: command)) {
                throw CoreError.Action(failure: ActionFailed(accountName: "Whoosh", reasonWords: reason, retryable: true))
            }
            let t0 = InteractionProbe.phaseStart()
            _ = try core.run(command: command)
            InteractionProbe.phaseEnd(.engineRun, t0)
            lastFailure = nil
            refresh()
            let t1 = InteractionProbe.phaseStart()
            SearchMacModel.shared.acted(command)   // results show: a mail that left the mailbox leaves the results too
            InteractionProbe.phaseEnd(.searchActed, t1)
            return true
        } catch CoreError.Action(let failure) {
            lastFailure = failure
        } catch {
            lastFailure = ActionFailed(accountName: "Whoosh", reasonWords: "\(error)", retryable: false)
        }
        refresh()
        return false
    }

    public func moveFocus(_ delta: Int) -> Bool {
        guard !rows.isEmpty else { return false }
        focusIndex = min(max(0, focusIndex + delta), rows.count - 1)
        return true
    }

    public func focus(messageId: String) {
        if let i = rows.firstIndex(where: { $0.messageId == messageId }) { focusIndex = i }
    }

    public func onFocused(_ make: (String) -> Command) -> Bool {
        guard let id = shownMessageId else { return false }
        return perform(make(id))
    }

    public func switchTab(_ delta: Int) -> Bool {
        let ids = tabIds
        guard let i = ids.firstIndex(of: tab), !ids.isEmpty else { return false }
        tab = ids[(i + delta + ids.count) % ids.count]
        focusIndex = 0
        refresh()
        return true
    }

    /// The key path every key takes: the real keyboard and the socket verb `key` both call this.
    @discardableResult
    public func press(_ chord: String) -> Bool {
        let c = Self.normalise(chord)
        guard let action = Registrations.key(c) else { return false }
        return action(self)
    }

    /// "e" and "E" are the same key; named keys keep their name ("Enter", "Escape", "Shift+Tab", "⌘K").
    /// The list's hide-emblems toggle: UserDefaults `whoosh.list.showEmblems`, true when never set.
    public static var showEmblems: Bool {
        UserDefaults.standard.object(forKey: "whoosh.list.showEmblems") as? Bool ?? true
    }

    public static func normalise(_ chord: String) -> String {
        chord.count == 1 ? chord.uppercased() : chord
    }
}
