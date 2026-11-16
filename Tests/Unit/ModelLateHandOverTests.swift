import Foundation
import Testing
@testable import WhooshUI

// model-late (SPEC A141): at launch the engine hears that the model's answer is coming BEFORE the look for the files, so a search typed
// before the hand-over shows the word list and is ranked when the model comes, never "the model is not on this Mac" meanwhile.

private final class ModelLateEngine: BackgroundEngine, @unchecked Sendable {
    let l = NSLock(); var calls: [String] = []
    func setConditions(batteryPercent: Int, charging: Bool, thermalSerious: Bool) {}
    func ownerActed() {}
    func expectEmbedder() { l.withLock { calls.append("expect") } }
    func setEmbedder(_ e: (any Embedder)?) { l.withLock { calls.append(e == nil ? "none" : "model") } }
    var seen: [String] { l.withLock { calls } }
}

/// Red first: BackgroundWork.swift `EmbedderHandOver.start`, the line `engine.expectEmbedder()` removed: the calls are ["model"].
@Test func theEngineHearsTheModelIsComingBeforeItIsHandedOver() async throws {
    let engine = ModelLateEngine()
    let hand = EmbedderHandOver(engine: engine, state: BackgroundState(), find: { FixtureEmbedder() })
    hand.start()
    try await Task.sleep(nanoseconds: 300_000_000)
    #expect(engine.seen == ["expect", "model"], "said first, then the model: \(engine.seen)")
}

/// Red first: the same line removed: the calls are ["none"].
@Test func theEngineHearsTheModelIsComingBeforeItIsToldThereIsNone() async throws {
    let engine = ModelLateEngine()
    let hand = EmbedderHandOver(engine: engine, state: BackgroundState(), find: { nil })
    hand.start()
    try await Task.sleep(nanoseconds: 300_000_000)
    #expect(engine.seen == ["expect", "none"], "said first, then no model: \(engine.seen)")
}
