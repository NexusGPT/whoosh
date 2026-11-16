import Foundation
import Testing
@testable import WhooshUI

// Lane fix-rest-app, point 1 and 4: the test-engine verbs the search checks need (`fixture index-progress`, `fixture seed-corpus`), the rig-only
// hand-over delay (WHOOSH_RIG_EMBED_HANDOVER_DELAY_MS) and the sync failure the fixture core now answers. Each test names its broken copy in a trailing comment.
// Run with WHOOSH_FIXTURE_CORE=1 (the shared AppModel serves the design fixture).

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
@MainActor private func model() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    setenv("WHOOSH_FIXTURE_PATH", repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}
@MainActor private func readCount(_ m: AppModel, _ path: String) -> Int? {
    guard let text = m.core.state(path: path), let data = text.data(using: .utf8) else { return nil }
    return (try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])) as? Int
}

// MARK: fixture index-progress

@Test @MainActor func theIndexProgressVerbPutsItsTwoFiguresOnTheModelAndRefusesWhatIsNotTwoNumbers() throws {
    let m = try model()
    defer { IndexProgressModel.shared.take(.init()) }
    let r = try FixtureVerbs.run(m, ["index-progress", "40", "60"])
    #expect(IndexProgressModel.shared.status.indexed == 40)                     // mutant: indexed and queued swapped in the verb (`s.indexed = queued`)
    #expect(IndexProgressModel.shared.status.queued == 60)
    #expect(IndexProgress.isBuilding(IndexProgressModel.shared.status))
    #expect((r["index_progress"] as? [Int]) == [40, 60])                        // mutant: the reply names the figures in the other order
    _ = try FixtureVerbs.run(m, ["index-progress", "100", "0"])
    #expect(!IndexProgress.isBuilding(IndexProgressModel.shared.status))        // mutant: the verb ignores `0` queued (the line would stay)
    for bad in [["index-progress"], ["index-progress", "5"], ["index-progress", "a", "1"], ["index-progress", "-1", "0"], ["index-progress", "1", "2", "3"]] {
        #expect(throws: ControlFailure.self) { _ = try FixtureVerbs.run(m, bad) }   // mutant: the guard on Int() or on >= 0 removed
    }
    #expect(IndexProgressModel.shared.status.queued == 0, "a refused call changed nothing")
}

// MARK: fixture seed-corpus

@Test @MainActor func theSeedCorpusVerbLeavesExactlyThatManyMailsAndRefusesNoneOrTooMany() throws {
    let m = try model()
    defer { CoreHost.shared.fixture?.restoreSeed() }
    for n in [11, 1, 40] {
        let r = try FixtureVerbs.run(m, ["seed-corpus", String(n)])
        #expect(readCount(m, "index/message_count") == n)                       // mutant: the cut to N removed (`made.rows.count > n`), so the count is a multiple of the accounts
        #expect(r["messages"] as? Int == n)
    }
    #expect(throws: ControlFailure.self) { _ = try FixtureVerbs.run(m, ["seed-corpus", "0"]) }                                   // mutant: guard n >= 1 removed
    #expect(throws: ControlFailure.self) { _ = try FixtureVerbs.run(m, ["seed-corpus", String(FixtureCore.seedCorpusMax + 1)]) } // mutant: the upper bound removed
    #expect(throws: ControlFailure.self) { _ = try FixtureVerbs.run(m, ["seed-corpus", "many"]) }
    #expect(throws: ControlFailure.self) { _ = try FixtureVerbs.run(m, ["seed-corpus"]) }
    #expect(readCount(m, "index/message_count") == 40, "a refused call changed nothing")
}

@Test @MainActor func aResetBringsTheSampleBackAfterASeededCorpus() throws {
    let m = try model()
    let sample = readCount(m, "index/message_count")
    defer { CoreHost.shared.fixture?.restoreSeed() }
    _ = try FixtureVerbs.run(m, ["seed-corpus", "5"])
    #expect(readCount(m, "index/message_count") == 5)
    CoreHost.shared.fixture?.restoreSeed()
    #expect(readCount(m, "index/message_count") == sample)                      // mutant: seedCorpus keeps the mail in a store restoreSeed does not reset
}

// MARK: the sync failure (`fixture sync broken|ok`)

@Test @MainActor func aBrokenSyncAnswersTheCoresReasonOkAndAResetTakeItAway() throws {
    let m = try model()
    defer { FixtureVerbs.syncBroken = false; CoreHost.shared.fixture?.restoreSeed() }
    #expect(m.core.state(path: "settings_sync/error") == nil)                   // mutant: the case answers a reason while the flag is off
    #expect(SettingsPhoneSync.read(m).phase == .inSync)
    _ = try FixtureVerbs.run(m, ["sync", "broken"])
    let text = try #require(m.core.state(path: "settings_sync/error"))          // mutant: the case deleted (inert verb, as before)
    #expect(text == "\"\(FixtureCore.syncBrokenReason)\"")
    let read = SettingsPhoneSync.read(m)
    #expect(read.phase == .failing && read.reason == FixtureCore.syncBrokenReason && read.retry)
    _ = try FixtureVerbs.run(m, ["sync", "ok"])
    #expect(m.core.state(path: "settings_sync/error") == nil)
    _ = try FixtureVerbs.run(m, ["sync", "broken"])
    CoreHost.shared.fixture?.restoreSeed()
    #expect(m.core.state(path: "settings_sync/error") == nil)                   // mutant: restoreSeedLocked without `syncBroken = false`
}

// MARK: the rig-only hand-over delay

private let rigEnv = ["WHOOSH_RIG_HIDDEN": "1"]
private func withDelay(_ env: [String: String], _ ms: String) -> [String: String] { env.merging(["WHOOSH_RIG_EMBED_HANDOVER_DELAY_MS": ms]) { $1 } }

@Test func aTestCopyReadsTheHandOverDelayFromItsEnvironment() {
    #expect(EmbedderHandOver.rigDelayMs(env: withDelay(rigEnv, "250"), bundleId: nil, arguments: []) == 250)               // mutant: the function returns 0 always
    #expect(EmbedderHandOver.rigDelayMs(env: withDelay(["WHOOSH_SOCKET_DIR": "/x"], "250"), bundleId: nil, arguments: []) == 250)
    #expect(EmbedderHandOver.rigDelayMs(env: withDelay([:], "250"), bundleId: "app.whoosh.rig", arguments: []) == 250)
    #expect(EmbedderHandOver.rigDelayMs(env: withDelay([:], "250"), bundleId: nil, arguments: ["--rig-hidden"]) == 250)
}

@Test func theOwnersAppNeverReadsTheHandOverDelay() {
    #expect(EmbedderHandOver.rigDelayMs(env: withDelay([:], "250"), bundleId: "app.whoosh", arguments: []) == 0)           // mutant: the isRig guard removed
    #expect(EmbedderHandOver.rigDelayMs(env: withDelay([:], "250"), bundleId: nil, arguments: []) == 0)
    #expect(EmbedderHandOver.rigDelayMs(env: withDelay(["WHOOSH_RIG_HIDDEN": "0"], "250"), bundleId: nil, arguments: []) == 0)
}

@Test func theHandOverDelayIsAbsentEmptyBadOrNegativeMeansNoneAndIsCapped() {
    #expect(EmbedderHandOver.rigDelayMs(env: rigEnv, bundleId: nil, arguments: []) == 0)
    for bad in ["", "abc", "-5", "0", "1.5"] {
        #expect(EmbedderHandOver.rigDelayMs(env: withDelay(rigEnv, bad), bundleId: nil, arguments: []) == 0, "\(bad)")     // mutant: a negative value sleeps (`ms > 0` removed)
    }
    #expect(EmbedderHandOver.rigDelayMs(env: withDelay(rigEnv, "999999999"), bundleId: nil, arguments: []) == EmbedderHandOver.rigDelayMaxMs)   // mutant: cap removed
}

private final class DelayEngine: BackgroundEngine, @unchecked Sendable {
    let l = NSLock(); var calls: [String] = []
    func setConditions(batteryPercent: Int, charging: Bool, thermalSerious: Bool) {}
    func ownerActed() {}
    func setEmbedder(_ e: (any Embedder)?) { l.withLock { calls.append(e == nil ? "none" : "model") } }
    var seen: [String] { l.withLock { calls } }
}

@Test func aDelayedHandOverWaitsAndThenHandsOverAndNoDelayHandsOverAtOnce() async throws {
    let slow = DelayEngine()
    let late = EmbedderHandOver(engine: slow, state: BackgroundState(), find: { FixtureEmbedder() }, delayMs: { 600 })
    late.start()
    late.look()                                                                  // a look while it waits (the app came to the front) must not hand over early
    try await Task.sleep(nanoseconds: 250_000_000)
    #expect(slow.seen.isEmpty, "still waiting: \(slow.seen)")                    // mutant: start() does not sleep (the delay is read and ignored)
    try await Task.sleep(nanoseconds: 800_000_000)
    #expect(slow.seen == ["model"], "handed over after the wait: \(slow.seen)")
    let quick = DelayEngine()
    let now = EmbedderHandOver(engine: quick, state: BackgroundState(), find: { FixtureEmbedder() }, delayMs: { 0 })
    now.start()
    try await Task.sleep(nanoseconds: 250_000_000)
    #expect(quick.seen == ["model"], "no delay, no wait: \(quick.seen)")
}
