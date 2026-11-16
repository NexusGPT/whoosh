import Foundation
import Observation

/// Light focus (round 5, owner 2026-11-08 10:51: "navigation with arrows is WAYYYY TOO SLOW AND HEAVY").
/// A passing focus in the list (J, K, the arrows) costs the reader nothing: the pane keeps what it shows until
/// FOCUS_SLIDE_MS has gone by with no further move, and only then builds the new thread (and, in an open reader,
/// loads the body) and cross-fades to it. The list's own state changes on the key's frame as before; this class
/// only decides WHEN the reader follows. An open (Enter, a click), and a shown message that left the list (archived,
/// deleted), follow at once. The pane's body never reads the list's focus index: that is the whole saving.
@MainActor @Observable
final class ReaderFocus {
    static let shared = ReaderFocus()

    /// The message the pane shows (observed: the pane re-evaluates when this changes, and not on a passing focus).
    private(set) var shownId: String?

    /// What the selected search result hands the reader for the mail the pane shows (A152): the mail that holds the passage, the passage, its source. Set when the
    /// pane settles (never on a passing focus, so J and K cost the pane nothing more), nil when the search is closed or the mail shown is not a result.
    private(set) var passage: ReaderPassage?

    @ObservationIgnored private var watching = false
    @ObservationIgnored private var wasOpen = false
    @ObservationIgnored private var lastFocus: String?
    @ObservationIgnored private var wasResults = false   // search results were showing at the last change: entering or leaving them is not a passing focus
    @ObservationIgnored private var deadline: ContinuousClock.Instant?
    @ObservationIgnored private var timer: Task<Void, Never>?
    @ObservationIgnored private(set) var moves = 0
    @ObservationIgnored private(set) var settles = 0

    /// A passing focus is waiting for its quiet time.
    var pending: Bool { deadline != nil }

    /// The message the reader should draw now. Before the observer runs it is the focused row, as it always was.
    /// An open of the reader (the surface flipped, `changed` not yet run) shows the focused message at once.
    func target(_ m: AppModel) -> String? {
        let shown = shownId   // read first: the pane's body then observes it
        guard watching else { return m.shownMessageId }
        if m.surface == ReaderState.surface && !wasOpen { return m.shownMessageId }
        return shown
    }

    /// Starts watching the list's focus. Called from the pane's `onAppear` and from every probe; idempotent.
    func ensure(_ m: AppModel) {
        guard !watching else { return }
        watching = true
        wasOpen = m.surface == ReaderState.surface
        lastFocus = m.shownMessageId
        shownId = lastFocus
        wasResults = SearchMacModel.shared.showsResults
        passage = ReaderPassage.current(m)
        track(m)
    }

    /// An open settles the pane on the focused message in the same turn as the surface change.
    func settleForOpen(_ m: AppModel) {
        ensure(m)
        wasOpen = true
        lastFocus = m.shownMessageId
        settle(lastFocus, m)
        freshOpen()
    }

    /// A reader that opens starts without marks, expansions or a link ring, even on the thread it showed before.
    /// perf-archive r3: the held body is NOT dropped here. `settle` (always called just before) drops it when another message is
    /// shown; when the open is of the mail the resting pane already showed, the held body is that same mail's body, and dropping it
    /// made the pane's next draw (the turn after Enter's commit) read the body from the core again and rebuild its content (regex
    /// passes) for an equal result. A body that lands later still goes through `ReaderBodies.forget` (body-r1).
    private func freshOpen() {
        ReaderState.shared.resetNavigation()
        ReaderLinks.shared.reset()
    }

    /// A probe or a handler that needs the state of "now" runs the pending bookkeeping first.
    func catchUp(_ m: AppModel) {
        ensure(m)
        changed(m)
    }

    private func track(_ m: AppModel) {
        withObservationTracking {
            _ = m.shownMessageId
            _ = m.surface
        } onChange: { [weak self] in
            // onChange fires before the new value is stored; the next main-queue turn sees it.
            DispatchQueue.main.async { MainActor.assumeIsolated {
                let t0 = InteractionProbe.phaseStart()
                self?.changed(m)
                InteractionProbe.phaseEnd(.readerSettle, t0)
                self?.track(m)
            } }
        }
    }

    private func changed(_ m: AppModel) {
        let open = m.surface == ReaderState.surface
        let opening = open && !wasOpen
        wasOpen = open
        if SearchMacModel.shared.holdsReader { return }   // a search is loading: the pane keeps what it shows (the end of the load runs this again)
        let id = m.shownMessageId
        let showing = SearchMacModel.shared.showsResults
        let switched = showing != wasResults   // a search landed or was left: the pane follows at once, like an open
        wasResults = showing
        if id == lastFocus && !opening { if deadline == nil { syncPassage(m) }; return }
        lastFocus = id
        moves += 1
        let results = SearchMacModel.shared.showsResults ? SearchMacModel.shared.results.map(\.row.messageId) : []
        let gone = shownId.map { s in !m.rows.contains { $0.messageId == s } && !results.contains(s) } ?? true
        if opening || gone || switched || id == nil { settle(id, m); if opening { freshOpen() }; return }
        if id == shownId { deadline = nil; return }
        deadline = ContinuousClock.now + .milliseconds(WhooshConstants.FOCUS_SLIDE_MS)
        if timer == nil { timer = Task { @MainActor in await self.wait(m) } }
    }

    private func wait(_ m: AppModel) async {
        while let d = deadline, ContinuousClock.now < d { try? await Task.sleep(until: d, clock: .continuous) }
        timer = nil
        guard deadline != nil else { return }
        settle(m.shownMessageId, m)
    }

    /// The hand-over follows the settled mail: assigned only when it differs, so an equal value redraws nothing.
    private func syncPassage(_ m: AppModel) {
        let p = ReaderPassage.current(m)
        if p != passage { passage = p }
    }

    private func settle(_ id: String?, _ m: AppModel) {
        deadline = nil
        settles += 1
        ReaderPerf.shared.mark("focus.settle")
        syncPassage(m)
        guard shownId != id else { return }
        shownId = id
        ReaderBodies.shared.clear()
        ReaderState.shared.resetNavigation()
        ReaderLinks.shared.reset()
    }

    /// The thread the pane draws for `resting` or open state, from the shown message.
    func thread(_ m: AppModel, resting: Bool) -> ReaderThread? { ReaderThread.make(m, messageId: target(m), resting: resting) }
}
