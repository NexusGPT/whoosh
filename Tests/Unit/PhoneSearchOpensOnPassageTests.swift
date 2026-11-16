import Foundation
import Testing
@testable import WhooshUI

// Lane fix-rest-app, point 3 (item 9f990234, docs/lanes/gap-phone-search.md findings 1 and 2): a result of the iPhone search opens its thread on the
// result's own mail even when no loaded inbox row holds it, and with the earlier mail that holds the passage unfolded. Each test names its broken copy.
// Run with WHOOSH_FIXTURE_CORE=1.

private let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
@MainActor private func appModel() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    setenv("WHOOSH_FIXTURE_PATH", root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}
/// The corpus with a three-mail thread (thr-01: msg-01 the newest, msg-01a and msg-01b before it), the search open and its answer set to `results`.
@MainActor private func rig(hit: String?, open id: String = "msg-01") throws -> (AppModel, SearchPhoneModel, [String]) {
    let m = try appModel()
    _ = try FixtureVerbs.run(m, ["corpus", root.appendingPathComponent("fixtures/tests-j2.json").path])
    ThreadPhoneState.shared.reset()
    let s = SearchPhoneModel.shared
    _ = s.close(); _ = s.open()
    let row = try #require(SearchMacModel.everyRow(m).first { $0.messageId == id } ?? m.core.inbox(query: InboxQuery(tabId: m.tab, accountId: nil, labelId: nil)).first { $0.messageId == id },
                           "the corpus holds \(id)")
    var r = SearchPhoneModel.wrapped(row)
    r.part = .match
    r.hitMessageId = hit ?? ""
    s.restOpen = false; s.stage = "ranked"
    s.applyParts([r])
    let earlier = (m.core.thread(threadId: row.threadId)?.messageIds ?? []).filter { $0 != id }
    return (m, s, earlier)
}

@MainActor @Test func aResultInNoLoadedRowIsStillAMailTheWindowKnows() throws {
    let m = try appModel()
    let s = SearchPhoneModel.shared
    _ = s.close(); _ = s.open()
    defer { _ = s.close() }
    var row = try #require(m.rows.first)
    row.messageId = "msg-in-no-loaded-row"
    #expect(m.knownRow("msg-in-no-loaded-row") == nil, "before: nothing knows it")
    s.restOpen = false; s.stage = "ranked"
    s.applyParts([SearchPhoneModel.wrapped(row)])
    #expect(m.knownRow("msg-in-no-loaded-row")?.messageId == "msg-in-no-loaded-row")     // mutant: AppModel.knownRow without the SearchPhoneModel line
    _ = s.close()
    #expect(m.knownRow("msg-in-no-loaded-row") == nil, "a closed search leaves no answer behind")
}

@MainActor @Test func aResultOpensItsThreadOnItsOwnMailWhereverItIs() throws {
    let m = try appModel()
    let s = SearchPhoneModel.shared
    _ = s.close(); _ = s.open()
    ThreadPhoneState.shared.reset()
    defer { _ = s.close(); ThreadPhoneState.shared.reset(); m.overlay = nil }
    var row = try #require(m.rows.first)
    let focusedBefore = m.focusedMessageId
    row.messageId = "msg-in-no-loaded-row"
    s.restOpen = false; s.stage = "ranked"
    s.applyParts([SearchPhoneModel.wrapped(row)])
    #expect(s.openResult(0))
    #expect(ThreadPhoneState.shared.target(m) == "msg-in-no-loaded-row", "the screen is on the result's mail, not on the focused inbox row \(String(describing: focusedBefore))")   // mutant: openResult only focuses a loaded row
    #expect(m.overlay == ThreadPhoneState.overlayId)
    #expect(ThreadPhoneState.shared.thread(m)?.messageId == "msg-in-no-loaded-row")
}

@MainActor @Test func theEarlierMailThatHoldsThePassageIsUnfoldedWhenTheThreadAppears() throws {
    let (m, s, earlier) = try rig(hit: "msg-01a")
    defer { _ = s.close(); ThreadPhoneState.shared.reset(); m.overlay = nil }
    let at = try #require(earlier.firstIndex(of: "msg-01a"), "msg-01a is an earlier mail of the thread: \(earlier)")
    #expect(s.openResult(0))
    #expect(ThreadPhoneState.shared.expanded.isEmpty, "nothing unfolds before the screen is up")
    ThreadPhoneState.shared.begin(m)                                                     // the screen's onAppear
    #expect(ThreadPhoneState.shared.expanded == [at], "the unfolded index is the hit's place among the earlier mails: \(ThreadPhoneState.shared.expanded)")   // mutant: begin never applies pendingUnfold
    let other = try #require(earlier.firstIndex(of: "msg-01b"))
    #expect(!ThreadPhoneState.shared.expanded.contains(other), "only the mail that holds the passage")
}

@MainActor @Test func theUnfoldIsAppliedOnceAndTheScreenReappearingDoesNotUnfoldAFoldedMailAgain() throws {
    let (m, s, earlier) = try rig(hit: "msg-01b")
    defer { _ = s.close(); ThreadPhoneState.shared.reset(); m.overlay = nil }
    let at = try #require(earlier.firstIndex(of: "msg-01b"))
    #expect(s.openResult(0))
    ThreadPhoneState.shared.begin(m)
    #expect(ThreadPhoneState.shared.expanded == [at])
    ThreadPhoneState.shared.toggleExpanded(at)                                           // the person folds it
    ThreadPhoneState.shared.begin(m)
    #expect(ThreadPhoneState.shared.expanded.isEmpty, "the second appearance leaves it folded")   // mutant: pendingUnfold is not cleared after it is applied
    #expect(ThreadPhoneState.shared.pendingUnfold == nil)
}

@MainActor @Test func aResultWhoseOwnMailHoldsThePassageOrWhoseHitIsNotInTheThreadUnfoldsNothing() throws {
    for hit in [nil, "msg-01", "msg-not-in-this-thread"] as [String?] {
        let (m, s, _) = try rig(hit: hit)
        #expect(s.openResult(0))
        ThreadPhoneState.shared.begin(m)
        #expect(ThreadPhoneState.shared.expanded.isEmpty, "hit \(String(describing: hit)) unfolds nothing: \(ThreadPhoneState.shared.expanded)")   // mutant: `hit == id` is not excluded, or an unknown hit inserts index 0
        _ = s.close(); ThreadPhoneState.shared.reset(); m.overlay = nil
    }
}

@MainActor @Test func openingAMailTheWindowDoesNotKnowChangesNothing() throws {
    let m = try appModel()
    ThreadPhoneState.shared.reset()
    m.overlay = nil
    #expect(!ThreadPhoneState.shared.open(messageId: "no-such-mail", unfolding: "x", in: m))   // mutant: the knownRow guard removed
    #expect(m.overlay == nil && ThreadPhoneState.shared.messageId == nil && ThreadPhoneState.shared.pendingUnfold == nil)
}
