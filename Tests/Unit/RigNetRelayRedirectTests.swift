import Foundation
import Testing
@testable import WhooshUI

// A hidden test copy (a rig) never follows a relay's redirect to a host outside this Mac (owner: "nothing leaves this Mac except the
// relay"). Found 2026-11-11 (rig network audit, gap 7): the loopback check ran on the base address only, and URLSession.shared follows
// redirects. `RigNet` is the one rule. No test opens a socket: the redirect decision is asked directly.

private let rigEnv = ["WHOOSH_RIG_HIDDEN": "1"]
private let ownerEnv: [String: String] = [:]
// No RelayClient is built here: its init takes URLSession.shared by default. The redirect rule is asked on the guard itself, and the
// init's choice (`RigNet.rig(env:) ? RigRedirectGuard(env:) : nil`, RelayClient.swift) on the shared rule.

@Test func aRigsRelayRedirectGuardRefusesARedirectOffThisMac() {
    #expect(RigNet.rig(env: rigEnv))
    let guardian = RigRedirectGuard(env: rigEnv)
    #expect(guardian.next(URLRequest(url: URL(string: "https://relay.example.com/v1/registrations/a")!)) == nil)
    #expect(guardian.next(URLRequest(url: URL(string: "http://127.0.0.1@relay.example.com/x")!)) == nil)
}

@Test func aRigsRelayRedirectGuardFollowsARedirectOnThisMac() {
    let local = URLRequest(url: URL(string: "http://localhost:8787/v1/registrations/a")!)
    #expect(RigRedirectGuard(env: rigEnv).next(local) == local)
}

@Test func theOwnersRelayClientGetsNoRedirectGuard() {
    #expect(!RigNet.rig(env: ownerEnv))   // the init then passes no delegate, as before
    let remote = URLRequest(url: URL(string: "https://relay.example.com/v1/registrations/a")!)
    #expect(RigRedirectGuard(env: ownerEnv).next(remote) == remote)
}
