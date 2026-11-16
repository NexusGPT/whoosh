import Foundation
import Testing
@testable import WhooshUI

// Written from docs/plan.json fn-richtext and docs/JOURNEYS.md J3.2/J3.4. The editor wrapper exposes nothing below the socket
// yet (RichTextFeature is a stub), so the round trip itself is driven by checks/richtext-check.py. These tests hold the
// registrations the check reads and fail on the stub until the lane lands.
// MUTANTS:
//   theRunsRegionTheRoundTripReadsIsRegistered : the lane forgets to register mac-composer.runs
//   richTextIsInTheFeatureRegistry             : the registry stops listing RichTextFeature

@MainActor @Test func theRunsRegionTheRoundTripReadsIsRegistered() {
    #expect(Registrations.rowSource("mac-composer.runs") != nil, "no rows at mac-composer.runs: richtext-check.py cannot read formats")
}

@MainActor @Test func richTextIsInTheFeatureRegistry() {
    #expect(FeatureRegistry.all.contains { $0.id == "richtext" })
}
