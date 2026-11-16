import Foundation
import Testing
@testable import WhooshUI

// Written from docs/JOURNEYS.md J3.1-J3.2 and fixtures/tests-j3.json. No microphone is needed: the audio is a recorded fixture.
// Dictation exposes no clean-up function or engine port in Sources yet, so there is nothing below the socket to call; that is
// a hand-back row in docs/lanes/tests-j3.md. These tests hold what is checkable today and fail on the stub for the rest.
// MUTANTS:
//   theAudioFixtureIsARealRecording      : the fixture base64 is cut or the header changed
//   everyUtteranceHasCleanedTextOrIsEmpty: an utterance fixture loses its cleaned text
//   theGlobeKeyPairIsBound               : the lane binds only the press, not the release
//   dictationIsInTheFeatureRegistry      : the registry stops listing DictationFeature

private func fixture() throws -> [String: Any] {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    return try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: root.appendingPathComponent("fixtures/tests-j3.json"))) as? [String: Any])
}

@Test func theAudioFixtureIsARealRecording() throws {
    let a = try #require((try fixture()["dictation"] as? [String: Any])?["audio"] as? [String: Any])
    let b64 = try #require(a["base64"] as? String)
    let data = try #require(Data(base64Encoded: b64))
    #expect(String(decoding: data.prefix(4), as: UTF8.self) == "RIFF")
    #expect(String(decoding: data[8..<12], as: UTF8.self) == "WAVE")
    let rate = data[24..<28].withUnsafeBytes { $0.loadUnaligned(as: UInt32.self) }
    #expect(rate == 8000)
    #expect(data.count > 44 + 1000, "the recording holds samples, not only a header")
}

@Test func everyUtteranceHasCleanedTextOrIsEmpty() throws {
    let us = try #require((try fixture()["dictation"] as? [String: Any])?["utterances"] as? [[String: Any]])
    #expect(us.count == 3)
    for u in us {
        let raw = try #require(u["raw"] as? String), cleaned = try #require(u["cleaned"] as? String)
        #expect(raw.isEmpty == cleaned.isEmpty, "utterance \(u["id"] ?? "?"): raw and cleaned are empty together")
        for f in (u["fillers_in_raw"] as? [String]) ?? [] {
            #expect(raw.contains(f), "the raw text really holds the filler \(f) (the positive control for its removal)")
            #expect(!(" " + cleaned.lowercased() + " ").contains(" \(f) "), "the cleaned text drops the filler \(f)")
        }
    }
}

@MainActor @Test(arguments: ["Fn:down", "Fn:up"])
func theGlobeKeyPairIsBound(chord: String) throws {
    #expect(Registrations.key(chord) != nil, "\(chord) is bound to nothing: holding Globe cannot start or end a dictation")
}

@MainActor @Test func dictationIsInTheFeatureRegistry() {
    #expect(FeatureRegistry.all.contains { $0.id == "dictation" })
}
