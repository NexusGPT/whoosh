import Foundation
import Testing
@testable import WhooshUI

// Written from docs/JOURNEYS.md, docs/CONSTANTS.md and the fixture, never from a built screen. A test of a feature
// that is still a stub FAILS on purpose: the failure is the honest state until the lane lands.
// Run with WHOOSH_FIXTURE_CORE=1 so the shared AppModel serves the design fixture.
// MUTANTS: each test names, in its trailing comment, the mutation of the target that must turn it red.

@MainActor private func model() throws -> AppModel {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()   // launch state once per test (Tests/Support/SharedAppFresh.swift)
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
    return m
}
@MainActor private func rows(_ ax: String) throws -> [ControlRow] {
    let src = try #require(Registrations.rowSource(ax), "no rows are registered at \(ax) yet")
    return try src(try model())
}
private func fixture() throws -> [String: Any] {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    let data = try Data(contentsOf: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"))
    return try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
}
private func list(_ key: String) throws -> [[String: Any]] { try #require(try fixture()[key] as? [[String: Any]]) }

@MainActor @Test func theReaderRegionsExistAndAgreeWithTheOpenThread() throws {
    let t = try #require(try fixture()["open_thread"] as? [String: Any])
    let subject = try rows("mac-main.reader.subject")
    #expect(subject.first?["subject"] as? String == t["subject"] as? String)       // mutant: previous thread's subject
    let thread = try rows("mac-main.reader.thread")
    #expect(thread.count == (t["messages"] as? [Any])?.count)                       // mutant: only the last message
}
// swift-tests lane: SKIPPED, not passed. The row reads `MessageBodyProbe.shared.current` (Sources/Features/MessageBody/MessageBodyFeature.swift:38-39), which the message body VIEW sets when it
// is drawn; a test process has no window and draws nothing, so the count cannot be judged here (main76: ControlFailure "no message body is on screen"). It is judged on the running
// rig by checks/messagebody-check.py (J1.6).
@MainActor @Test(.disabled("needs a drawn message body: MessageBodyProbe.current is set by the view and a test process has no window; judged by checks/messagebody-check.py on the rig")) func trackersBlockedShowAsACount() throws {
    // swift-tests lane: the trackers row says "no message body is on screen" until a thread is open (main76 ControlFailure); open the focused one first, as Enter does.
    let open = try #require(Registrations.click("mac-main.reader.open"), "mac-main.reader.open is not registered")
    _ = open(try model())
    defer { _ = try? model().press("Escape") }
    let t = try rows("mac-main.reader.trackers")
    #expect(t.first?["count"] as? Int == 0)        // the fixture thread is clean; mutant: a fake count
}
@MainActor @Test func openingAThreadMarksItReadInTheCore() throws {
    let m = try model()
    #expect(m.press("Enter") == true)              // mutant: Enter is unbound
    #expect(m.core.state(path: "message/\(m.focusedMessageId ?? "")/read") == "true")
    _ = m.press("Escape")
}
