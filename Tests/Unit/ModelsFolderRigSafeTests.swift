import Foundation
import Testing
@testable import WhooshUI

// A hidden test copy (a rig) never asks a model file outside this Mac (owner: "nothing leaves this Mac except the relay").
// Found 2026-11-11: with no WHOOSH_RIG_LAYA_BASE a rig copy fell back to the real host for the decision model.
// `ModelsFolder.rigSafe` is the one rule; LayaModelStore and FileFetcher (the other models) both pass their address through it.

private let rigEnv = ["WHOOSH_RIG_HIDDEN": "1"]
private let ownerEnv: [String: String] = [:]
private let refusing = URL(string: "http://127.0.0.1:1/")!

@Test func aRigCopyReplacesARealHostByAnAddressThatRefuses() {
    let real = URL(string: "https://huggingface.co/convaiinnovations/laya/resolve/main/model.safetensors")!
    #expect(ModelsFolder.rigSafe(real, env: rigEnv) == refusing)
    #expect(ModelsFolder.rigSafe(LayaModelStore.defaultBase, env: rigEnv) == refusing)
}

@Test func aRigCopyKeepsAnAddressOnThisMac() {
    for text in ["http://127.0.0.1:8123/laya/", "http://localhost:9000/x", "http://[::1]:7000/y"] {
        let local = URL(string: text)!
        #expect(ModelsFolder.rigSafe(local, env: rigEnv) == local)
    }
}

@Test func aRigCopyRefusesAnAddressWithNoHost() {
    #expect(ModelsFolder.rigSafe(URL(string: "file:///tmp/model")!, env: rigEnv) == refusing)
}

@Test func theOwnersCopyKeepsTheRealHost() {
    // bundleId and arguments are the test process's own: neither names a rig, so only the environment decides here.
    let real = LayaModelStore.defaultBase
    #expect(ModelsFolder.rigSafe(real, env: ownerEnv) == real)
}
