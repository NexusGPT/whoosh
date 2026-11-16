import Foundation
import Testing
@testable import WhooshUI

// The measures perf/thirty-one.py reads through the control socket's `measure` verb (Sources/ControlSocket/WindowVerbs.swift, PerfMeasures):
// `app.memory` and `mac-search.first-screen`. Each test goes through the real verb (ControlVerbs.handle), not the enum.
// BROKEN COPY (red first): delete the `if let served = try PerfMeasures.answer(ax)` line in WindowVerbs.swift: both verbs then answer
// "no measurement at <id>" and both tests below fail.
// A measure costs nothing when nobody reads it: PerfMeasures holds no timer, no observer and no stored state (a test below pins the id map).

@MainActor private func armed() throws {
    setenv("WHOOSH_FIXTURE_CORE", "1", 1)
    let repo = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    setenv("WHOOSH_FIXTURE_PATH", repo.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let m = SharedApp.begin()
    try #require(m.host.kind == .fixture, "run the unit tests with WHOOSH_FIXTURE_CORE=1")
}

private func reply(_ json: String) throws -> [String: Any] {
    try #require(try JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any], "not a JSON object: \(json)")
}

@MainActor @Test func measureAppMemoryAnswersThePhysicalFootprintInMegabytes() throws {
    try armed()
    let r = try reply(ControlVerbs.handle(#"{"verb":"measure","args":["app.memory"]}"#))
    #expect(r["ok"] as? Bool == true, "\(r)")
    let mb = try #require(r["mb"] as? Double, "no mb in \(r)")
    #expect(mb > 10 && mb < 100_000, "a running test process holds some MB, never 0: \(mb)")
    let rss = try #require(r["rss_mb"] as? Double, "no rss_mb in \(r)")
    #expect(rss > 10 && rss < 100_000, "\(rss)")
}

@MainActor @Test func measureMacSearchFirstScreenIsServedAndNeverAnInvented() throws {
    try armed()
    _ = ControlVerbs.handle(#"{"verb":"key","args":["j"]}"#)
    let text = ControlVerbs.handle(#"{"verb":"measure","args":["mac-search.first-screen"]}"#)
    #expect(!text.contains("no measurement at"), "the id is not served: \(text)")
    let r = try reply(text)
    if r["ok"] as? Bool == true {
        let ms = try #require(r["ms"] as? Double, "no ms in \(r)")
        #expect(ms > 0, "a 0 ms reading is not a reading: \(r)")
    } else {
        #expect((r["error"] as? String ?? "").contains("not judged"), "a frame that did not present is not judged, never a number: \(r)")
    }
}

@MainActor @Test func theFirstScreenReadsTheInteractionFrameAndNothingElseIsServed() {
    #expect(PerfMeasures.probeId(for: "mac-search.first-screen") == "mac-main.interaction.frame")
    #expect(PerfMeasures.probeId(for: "mac-search.first-screen.x") == nil)
    #expect(PerfMeasures.probeId(for: "app.memory") == nil)       // served directly, not through the interaction probe
    #expect(PerfMeasures.probeId(for: "mac-main.frame-max") == nil)
}
