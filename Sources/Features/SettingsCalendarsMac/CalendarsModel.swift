import Foundation
import Observation
#if os(macOS)
import AppKit
#endif

/// The state of the Agendas pane: what macOS says about access, the calendars it gives, and what was handed to the replies.
/// Nothing here waits: every read of macOS answers by `done`, and the view and the socket read what is cached.
@MainActor @Observable
final class CalendarsModel {
    static let shared = CalendarsModel()

    private(set) var state: CalendarAccessState
    private(set) var calendars: [CalendarInfo] = []
    /// A list is being read (shown as "Reading calendars", never as an empty list).
    private(set) var loading = false
    /// The system's window is up and waits for the owner.
    private(set) var asking = false
    /// macOS was asked and showed nothing (the status stayed not determined), or never called back within `askTimeout`.
    private(set) var noPrompt = false
    /// How many times "Open System Settings" was pressed (the rig counts it; it opens no other app).
    private(set) var openedSystemSettings = 0

    @ObservationIgnored let access: CalendarAccessing
    @ObservationIgnored var feed: ReplyCalendarFeed = CoreReplyCalendarFeed()
    @ObservationIgnored private var lastEnabled: Set<String> = []
    @ObservationIgnored private var debounce: Task<Void, Never>?
    @ObservationIgnored private var started = false
    @ObservationIgnored private var timeout: Task<Void, Never>?
    /// The longest the row says "asking": a real prompt waits for the owner, so it is long; the rig's is short.
    @ObservationIgnored private var askTimeout: Duration { CoreHost.shared.kind == .fixture ? .milliseconds(500) : .seconds(45) }

    private init() {
        let a: CalendarAccessing = CoreHost.shared.kind == .fixture || CorePlace.onRig ? FixtureCalendarAccess() : EventKitCalendarAccess()   // probes-r1 R4: any rig copy, whatever its core, never reaches EventKit
        access = a
        state = a.state
    }

    /// Reads the system's state and the calendar list once, and follows changes made outside Whoosh. Safe to call again.
    func start() {
        guard !started else { return }
        started = true
        access.observeChanges { [weak self] in
            MainActor.assumeIsolated { self?.changed() }
        }
        watchEnabled()
        refresh()
    }

    /// Reads the state, then the calendars, then hands both to the replies. Returns at once.
    func refresh() {
        state = access.state
        if state != .notAsked { noPrompt = false }
        CalendarLog.line("calendar access: \(state.rawValue)")
        guard state == .allowed else {
            calendars = []; loading = false
            pushFeed()
            return
        }
        loading = true
        access.loadCalendars { [weak self] list in
            MainActor.assumeIsolated { self?.took(list) }
        }
    }

    /// The one button: asks macOS once. It does nothing in any other state (allowed, refused: the system never asks twice).
    @discardableResult
    func ask() -> Bool {
        guard state == .notAsked, !asking else { return false }
        asking = true
        noPrompt = false
        CalendarLog.line("calendar access asked, status before: \(state.rawValue)")
        timeout?.cancel()
        let wait = askTimeout
        timeout = Task { [weak self] in
            try? await Task.sleep(for: wait)
            guard !Task.isCancelled else { return }
            self?.gaveUp()
        }
        access.requestAccess { [weak self] answer in
            MainActor.assumeIsolated { self?.asked(answer) }
        }
        return true
    }

    /// The refused state's door to the system's own pane. The rig only counts the press.
    @discardableResult
    func openSystemSettings() -> Bool {
        guard state == .refused || noPrompt else { return false }
        openedSystemSettings += 1
        #if os(macOS)
        if CoreHost.shared.kind != .fixture, !CorePlace.onRig, let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars") {
            NSWorkspace.shared.open(url)
        }
        #endif
        return true
    }

    /// A calendar was switched on or off as a source for replies: the events of the enabled ones are read again.
    func enabledSetChanged() { enabledMaybeChanged() }

    // MARK: answers

    private func asked(_ answer: CalendarAccessState) {
        timeout?.cancel()
        asking = false
        state = answer
        noPrompt = answer == .notAsked
        CalendarLog.line("calendar access answer, status after: \(answer.rawValue)" + (noPrompt ? ", macOS showed no prompt" : ""))
        refresh()
    }

    /// macOS never called back: the row stops saying "asking" and says macOS did not ask. A late answer still lands in `asked`.
    private func gaveUp() {
        guard asking else { return }
        asking = false
        noPrompt = true
        CalendarLog.line("calendar access: no answer from macOS in time, status \(access.state.rawValue)")
    }

    private func took(_ list: [CalendarInfo]) {
        calendars = list
        loading = false
        CalendarLog.line("calendars read: \(list.count)")
        pushFeed()
    }

    /// Calendars and events change often in bursts (a sync): one refresh, a second after the last change.
    private func changed() {
        debounce?.cancel()
        debounce = Task { [weak self] in
            try? await Task.sleep(for: .seconds(1))
            guard !Task.isCancelled else { return }
            self?.refresh()
        }
    }

    /// Not asked: nothing is sent (the core keeps what it holds). Refused: the core drops every calendar. Allowed: the list, then the events
    /// of the calendars the owner switched on in Settings > Replies > Sources.
    private func pushFeed() {
        let window = CalendarWindow.around(Date())
        switch state {
        case .notAsked:
            return
        case .refused:
            feed.replaceCalendars([])
            feed.replaceEvents([], window: window)
        case .allowed:
            feed.replaceCalendars(calendars)
            let enabled = feed.enabledCalendarIds()
            lastEnabled = enabled
            let ids = enabled.intersection(calendars.map(\.id))
            guard !ids.isEmpty else { feed.replaceEvents([], window: window); return }
            access.loadEvents(in: window, calendarIds: ids) { [weak self] events in
                MainActor.assumeIsolated { self?.feed.replaceEvents(events, window: window) }
            }
        }
    }

    /// The owner switches a calendar on or off in Settings > Replies > Sources: the pane's store reads again, and the events follow.
    private func watchEnabled() {
        withObservationTracking {
            _ = RepliesStore.shared.sources
        } onChange: { [weak self] in
            Task { @MainActor in self?.enabledMaybeChanged() }
        }
    }

    private func enabledMaybeChanged() {
        watchEnabled()
        guard state == .allowed else { return }
        let now = feed.enabledCalendarIds()
        guard now != lastEnabled else { return }
        pushFeed()
    }

    // MARK: the rig

    /// `fixture calendar-access <word>`: sets the fixture's access state. False for a word that is none of the four, or on the real Mac.
    @discardableResult
    func fixtureSet(_ word: String) -> Bool {
        guard let fx = access as? FixtureCalendarAccess, fx.set(word) else { return false }
        asking = false
        noPrompt = false
        timeout?.cancel()
        refresh()
        return true
    }

    /// `fixture reset`: not asked, nothing handed over, nothing pressed.
    func resetFromFixture() {
        guard let fx = access as? FixtureCalendarAccess else { return }
        fx.reset()
        openedSystemSettings = 0
        asking = false
        noPrompt = false
        timeout?.cancel()
        (feed as? CoreReplyCalendarFeed)?.forget()
        lastEnabled = []
        refresh()
    }
}
