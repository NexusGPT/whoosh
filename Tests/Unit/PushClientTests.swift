import Foundation
import Testing
@testable import WhooshUI

// Written from plan item fn-pushclient-mac and docs/CAPABILITY-CONTRACT.md C-J6.1 / C-J7.1, never from a built file.
// Acceptance: a muted account posts no notification and no notch drop; an unmuted one posts once per message.
// That behaviour has no seam a test can reach today (the stub exposes no arrival entry and no notification poster):
// it is the design finding in docs/lanes/tests-j6.md, covered end to end by checks/notch-check.py (J6.1 muted, resync).
// MUTANTS: each test names, in its trailing comment, the mutation of the target that must turn it red.

@MainActor @Test func thePushClientIsRegisteredExactlyOnce() {
    #expect(FeatureRegistry.all.filter { $0.id == "pushclient" }.count == 1)                 // mutant: the registry lists it twice
}

@MainActor @Test func thePushClientRegistersNoSurfaceOfItsOwn() {
    // It feeds the notch and the system notification centre; a surface of its own would be a second screen nobody designed.
    #expect(PushClientFeature.surfaces.isEmpty)                                              // mutant: it claims mac-notch
}
