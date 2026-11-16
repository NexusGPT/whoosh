import Foundation
import Testing
@testable import WhooshUI

// A hidden test copy (a rig) never opens the sign-in sheet and never asks a sign-in server outside this Mac (owner: "nothing leaves this
// Mac except the relay"; "on all tests, EVER, i want you to stop the preview opening EVER"). Found 2026-11-11 (rig network audit, gap 2):
// the welcome key started the real Microsoft page in ASWebAuthenticationSession and exchanged the code at the real token endpoint.
// `RigNet` is the one rule. No test opens a socket or a window: the sheet below is a stand-in that only counts calls.

private let rigEnv = ["WHOOSH_RIG_HIDDEN": "1"]
private let ownerEnv: [String: String] = [:]

private final class CountingSheet: AuthorizationUI, @unchecked Sendable {
    private let lock = NSLock(); private var n = 0
    var calls: Int { lock.withLock { n } }
    func authorize(_ url: URL, callbackScheme: String) async throws -> URL {
        lock.withLock { n += 1 }
        throw SignInError.cancelled
    }
}

private func service(_ sheet: CountingSheet, env: [String: String], endpoints: OAuthEndpoints) -> SignInService {
    SignInService(vault: MemoryVault(), ui: sheet, endpoints: { _ in endpoints },
                  reaches: { RigNet.allows($0, env: env) }, refusal: { RigNet.refusedWords })
}

@Test func aRigRefusesTheRealMicrosoftSignInBeforeAnySheetOrRequest() async {
    let sheet = CountingSheet()
    let s = service(sheet, env: rigEnv, endpoints: OAuthPresets.microsoft(clientId: "stand-in"))
    #expect(s.refusesAddress(.microsoft))
    await #expect(throws: SignInError.words(RigNet.refusedWords)) { _ = try await s.signIn(.microsoft, name: "someone@outlook.com") }
    #expect(sheet.calls == 0)
}

@Test func aRigRefusesEveryPresetRoute() {
    for e in [OAuthPresets.microsoft(clientId: "x"), OAuthPresets.fastmail(clientId: "x"), OAuthPresets.google(clientId: "x"), OAuthPresets.chatgpt(clientId: "x")] {
        #expect(service(CountingSheet(), env: rigEnv, endpoints: e).refusesAddress(.microsoft))
    }
}

@Test func aRigKeepsALoopbackAuthority() async {
    let local = OAuthEndpoints(authorize: URL(string: "http://127.0.0.1:8765/authorize")!, token: URL(string: "http://127.0.0.1:8765/token")!,
                               clientId: "stand-in", scopes: ["offline_access"])
    let sheet = CountingSheet()
    let s = service(sheet, env: rigEnv, endpoints: local)
    #expect(!s.refusesAddress(.microsoft))
    await #expect(throws: SignInError.cancelled) { _ = try await s.signIn(.microsoft, name: "someone@example.com") }
    #expect(sheet.calls == 1)   // the route went on to the (stand-in) sheet: a loopback authority is allowed
}

// The live service (`SignInController.liveService`) is built by the app only: building it here made the owner's real sheet
// (WebAuthSheet, an AuthenticationServices presenter) off the main thread and trapped the test process (lead's run at fc900042).
// The sheet choice is asked as the pure function liveService uses; the refusal is proven on a service built from stand-ins.

@Test func aRigChoosesNoSheetAndItsSheetRefusesWithoutOpening() async {
    #expect(SignInController.usesRigSheet(env: rigEnv))
    #expect(SignInController.usesRigSheet(env: ["WHOOSH_SOCKET_DIR": "/nonexistent/rig-run"]))
    await #expect(throws: SignInError.words(RigNet.refusedWords)) {
        _ = try await RigNoSheet().authorize(URL(string: "https://login.microsoftonline.com/common/oauth2/v2.0/authorize")!, callbackScheme: "whoosh")
    }
}

@Test func theOwnersCopyKeepsTheRealSignIn() {
    #expect(!SignInController.usesRigSheet(env: ownerEnv))   // liveService then builds WebAuthSheet, as before
    let s = service(CountingSheet(), env: ownerEnv, endpoints: OAuthPresets.microsoft(clientId: "stand-in"))
    #expect(!s.refusesAddress(.microsoft))
    #expect(!s.refusesAddress(.chatgpt))
}
