import Foundation
import Testing
@testable import WhooshUI

// A hidden test copy (a rig) never sends a pasted provider key outside this Mac (owner: "nothing leaves this Mac except the relay").
// Found 2026-11-11 (rig network audit, gap 1): `fixture provider-endpoint <provider> <url>` took any url and the key check POSTed the key
// to it as a Bearer token. `RigNet` (Sources/CoreBridge/CoreHost.swift) is the one rule; the verb and the POST both ask it. No test opens a socket.

private let rigEnv = ["WHOOSH_RIG_HIDDEN": "1"]
private let ownerEnv: [String: String] = [:]

@Test func theSharedRuleKeepsThisMacAndRefusesEveryOtherHostOnARig() {
    for text in ["http://127.0.0.1:8123/x", "http://localhost:9000/x", "http://[::1]:7000/y"] {
        #expect(RigNet.allows(URL(string: text)!, env: rigEnv))
        #expect(RigNet.safe(URL(string: text)!, env: rigEnv) == URL(string: text)!)
    }
    for text in ["https://api.openai.com/v1", "http://127.0.0.1.example.com/", "http://127.0.0.1@example.com/", "https://example.com/banner.png"] {
        #expect(!RigNet.allows(URL(string: text)!, env: rigEnv))
        #expect(RigNet.safe(URL(string: text)!, env: rigEnv) == RigNet.refusing)
    }
    #expect(!RigNet.allows(nil, env: rigEnv))
}

@Test func theSharedRuleChangesNothingOnTheOwnersCopy() {
    let real = URL(string: "https://api.openai.com/v1")!
    #expect(RigNet.allows(real, env: ownerEnv))
    #expect(RigNet.safe(real, env: ownerEnv) == real)
}

@Test func modelsFolderRigSafeIsTheSharedRule() {
    let real = URL(string: "https://huggingface.co/x")!
    #expect(ModelsFolder.rigSafe(real, env: rigEnv) == RigNet.safe(real, env: rigEnv))
    #expect(ModelsFolder.rigSafe(real, env: ownerEnv) == RigNet.safe(real, env: ownerEnv))
}

@Test func aRigRefusesAProviderEndpointOffThisMacInTheVerb() {
    #expect(FixtureVerbs.providerEndpointRefused("https://api.openai.com", env: rigEnv))
    #expect(FixtureVerbs.providerEndpointRefused("http://example.com:8080", env: rigEnv))
    #expect(FixtureVerbs.providerEndpointRefused("not a url", env: rigEnv))
    #expect(!FixtureVerbs.providerEndpointRefused("http://127.0.0.1:8123", env: rigEnv))
    #expect(!FixtureVerbs.providerEndpointRefused("", env: rigEnv))   // clearing the endpoint is always allowed
}

@Test func aRigRefusesAKeyCheckOffThisMacBeforeThePost() {
    #expect(FixtureCore.keyCheckURL("https://api.openai.com", env: rigEnv) == nil)
    #expect(FixtureCore.keyCheckURL("http://127.0.0.1:8123", env: rigEnv) == URL(string: "http://127.0.0.1:8123/validate")!)
}

@Test func theOwnersCopyKeepsEveryProviderEndpoint() {
    #expect(!FixtureVerbs.providerEndpointRefused("https://api.openai.com", env: ownerEnv))
    #expect(FixtureCore.keyCheckURL("https://api.openai.com", env: ownerEnv) == URL(string: "https://api.openai.com/validate")!)
}
