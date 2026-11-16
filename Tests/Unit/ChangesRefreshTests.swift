import Foundation
import Testing
@testable import WhooshUI

// SPEC (shell-refresh, 2026-11-10): the Mac list re-reads itself when the core stores mail. A burst of core events inside one window
// is ONE refresh; an event after the window is another; the owner mid-key defers a due refresh; a refresh keeps the focused message.
// The expected counts are typed here from the SPEC (one per window), never read back from CoreChanges.
// MUTANTS (each turns the named test red): (1) `note` ignores `pending` and arms every time: the burst test sees 50;
// (2) the sink drops `.inboxChanged` from `relevant`: the sink test sees 0; (3) `refreshKeepingFocus` is plain `refresh()`: the focus test sees another row.
// Not run in the round that wrote them (no build); the mutants are stated, not yet seen red.

// a computed property, not a global `let`: `CoreChanges.coalesceMs` is main-actor isolated, and a global's initial value is not (Swift 6: "main actor-isolated default value in a nonisolated context")
@MainActor private var window: Duration { Duration.milliseconds(CoreChanges.coalesceMs * 3) }   // three windows: long enough for one to end and its refresh to run

@MainActor private func makeChanges(midKey: @escaping @MainActor () -> Bool = { false }) -> (CoreChanges, Counter) {
    let c = CoreChanges()
    let n = Counter()
    c.refresh = { n.value += 1 }
    c.midKey = midKey
    return (c, n)
}

@MainActor private final class Counter { var value = 0 }

@MainActor @Test func aBurstOfFiftyEventsIsOneRefresh() async throws {
    let (c, n) = makeChanges()
    for _ in 0..<50 { c.note() }
    try await Task.sleep(for: window)
    #expect(n.value == 1)                                       // mutant (1): 50
    #expect(c.refreshCount == 1)
}

@MainActor @Test func anEventAfterTheWindowIsAnotherRefresh() async throws {
    let (c, n) = makeChanges()
    c.note()
    try await Task.sleep(for: window)
    c.note(); c.note()
    try await Task.sleep(for: window)
    #expect(n.value == 2)
}

@MainActor @Test func noEventNoRefresh() async throws {
    let (_, n) = makeChanges()
    try await Task.sleep(for: window)
    #expect(n.value == 0)
}

@MainActor @Test func aDueRefreshWaitsWhileTheOwnerIsMidKeyThenRuns() async throws {
    var busy = true
    let (c, n) = makeChanges(midKey: { busy })
    c.note()
    try await Task.sleep(for: .milliseconds(CoreChanges.coalesceMs * 2))
    #expect(n.value == 0)                                       // two windows in, still mid-key: held
    busy = false
    try await Task.sleep(for: window)
    #expect(n.value == 1)
}

@MainActor @Test func midKeyNeverHoldsARefreshForeverAfterTheDeferralCap() async throws {
    let (c, n) = makeChanges(midKey: { true })
    c.note()
    try await Task.sleep(for: .milliseconds(CoreChanges.coalesceMs * (CoreChanges.maxDeferrals + 3)))
    #expect(n.value == 1)
}

@MainActor @Test func theSinkTurnsTheCoresStreamEventsIntoOneRefresh() async throws {
    let (c, n) = makeChanges()
    let sink = ChangesSink()
    sink.owner = c
    sink.onEvent(event: .countsChanged(counts: Counts(tabs: [], mailboxes: [:])))
    sink.onEvent(event: .syncStateChanged(state: SyncState(accountId: "a", historyFetched: true, messagesIndexed: 3, lastError: nil, providerStatusWords: nil, lastSyncedMs: nil)))
    try await Task.sleep(for: window)
    #expect(n.value == 1)                                       // mutant (2): the relevant events are dropped, 0
}

@Test func onlyTheStreamsThatMoveTheListAreRelevant() {
    #expect(CoreChanges.relevant(.countsChanged(counts: Counts(tabs: [], mailboxes: [:]))))
    #expect(!CoreChanges.relevant(.actionFailed(failure: ActionFailed(accountName: "Whoosh", reasonWords: "x", retryable: false))))
}

@MainActor @Test func aRefreshKeepsTheFocusOnTheSameMessageWhenRowsArriveAbove() throws {
    // swift-tests lane: the shared model starts the process with no rows loaded; `SharedApp.begin()` hands it back in its launch state, rows read (main76: before.count was 0).
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    let before = m.rows
    try #require(before.count >= 3)
    m.focus(messageId: before[2].messageId)
    let id = before[2].messageId
    m.refreshKeepingFocus()
    #expect(m.focusedMessageId == id)                           // mutant (3): the index alone is kept, and a new row above shifts it
}
