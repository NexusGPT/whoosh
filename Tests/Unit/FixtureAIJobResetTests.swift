import Foundation
import Testing
@testable import WhooshUI

// fix-screens (finding of gap-composer): the AI job the composer reported is a stored property of the fixture core, so `fixture reset`
// takes it away like every other state.
// MUTANTS (red first, one edit each):
//   theAIJobNoteIsServedAndAResetTakesItAway : FixtureReset.swift `aiJobNote = nil` removed
//   noJobIsServedBeforeADraftRan             : FixtureCore.aiJobNote starts as a note instead of nil

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func fixtureCore() throws -> FixtureCore {
    try FixtureCore(url: repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: FixtureClock())
}

@Test func noJobIsServedBeforeADraftRan() throws {
    let core = try fixtureCore()
    #expect(core.state(path: "ai_job/last/agent") == nil)
    #expect(core.state(path: "ai_job/last/voice_profile_address") == nil)
}

@Test func theAIJobNoteIsServedAndAResetTakesItAway() throws {
    let core = try fixtureCore()
    core.noteAIJob(voiceProfileAddress: "me@example.com")
    #expect(core.state(path: "ai_job/last/voice_profile_address") == "\"me@example.com\"")
    #expect(core.state(path: "ai_job/last/agent") != nil)
    core.restoreSeed()
    #expect(core.state(path: "ai_job/last/voice_profile_address") == nil, "the reset must clear the job note")
    #expect(core.state(path: "ai_job/last/agent") == nil)
}
