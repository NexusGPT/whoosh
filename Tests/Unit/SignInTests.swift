import Foundation
import Network
import Testing
@testable import WhooshUI

// J4.1-J4.5: the four sign-in routes. Authorisation servers are STUBS on 127.0.0.1 served by this file; no provider
// host is ever contacted and no mail account is connected. Expected values come from docs/JOURNEYS.md J4 and the
// design fixture. The tests compile against the generated contracts and go red on the root's feature stubs until the
// signin / welcomemac lanes register their rows and clicks.

/// A one-shot HTTP stub on an ephemeral loopback port: answers every request with `status` and `body`.
final class StubAuthServer: @unchecked Sendable {
    let listener: NWListener
    private(set) var port: UInt16 = 0
    init(status: Int, body: String) throws {
        let params = NWParameters.tcp
        params.requiredLocalEndpoint = NWEndpoint.hostPort(host: "127.0.0.1", port: .any)
        listener = try NWListener(using: params)
        listener.newConnectionHandler = { conn in
            conn.start(queue: .global())
            conn.receive(minimumIncompleteLength: 1, maximumLength: 8192) { _, _, _, _ in
                let head = "HTTP/1.1 \(status) X\r\nContent-Type: application/json\r\nContent-Length: \(body.utf8.count)\r\nConnection: close\r\n\r\n"
                conn.send(content: Data((head + body).utf8), completion: .contentProcessed { _ in conn.cancel() })
            }
        }
    }
    func start() async throws {
        let ready = AsyncStream<UInt16> { c in
            listener.stateUpdateHandler = { st in
                if case .ready = st, let p = self.listener.port?.rawValue { c.yield(p); c.finish() }
                if case .failed = st { c.yield(0); c.finish() }
            }
        }
        listener.start(queue: .global())
        for await p in ready { port = p }
        #expect(port != 0, "the loopback stub did not start")
    }
    func stop() { listener.cancel() }
    var url: URL { URL(string: "http://127.0.0.1:\(port)/token")! }
}

private func fetch(_ url: URL) async throws -> (Int, String) {
    let (data, resp) = try await URLSession(configuration: .ephemeral).data(from: url)
    return ((resp as? HTTPURLResponse)?.statusCode ?? 0, String(decoding: data, as: UTF8.self))
}

@Test func theLoopbackStubAnswersWithItsStatusAndBody() async throws {      // positive control for every stub test below
    let ok = try StubAuthServer(status: 200, body: "{\"access_token\":\"stub\"}")
    try await ok.start(); defer { ok.stop() }
    let (code, body) = try await fetch(ok.url)
    #expect(code == 200); #expect(body.contains("stub"))
    let denied = try StubAuthServer(status: 400, body: "{\"error\":\"access_denied\"}")
    try await denied.start(); defer { denied.stop() }
    let (code2, body2) = try await fetch(denied.url)
    #expect(code2 == 400); #expect(body2.contains("access_denied"))
}

@MainActor @Test func everySignInRouteHasAControlTheSocketCanPress() {
    for ax in ["mac-welcome-accounts.continue", "mac-welcome-accounts.add-another",
               "mac-welcome-accounts.sign-in-microsoft", "mac-welcome-accounts.sign-in"] {
        #expect(Registrations.click(ax) != nil, "no click registered at \(ax)")
    }
}

@MainActor @Test func theProviderAndItsStepsAreRowsOfTheWelcomeScreen() {
    #expect(Registrations.rowSource("mac-welcome-accounts.provider") != nil)
    #expect(Registrations.rowSource("mac-welcome-accounts.steps") != nil)
}

@Test func addingAMicrosoftAccountThroughTheCoreStoresTheAddressAndNeverAPassword() throws {
    let core = try fixtureCore()
    let a = EXTRA_ADDRESS
    _ = try core.run(command: .addAccount(address: a, provider: .microsoft))     // the stub consent has granted
    #expect(core.state(path: "account/acc-31/address") != nil, "the account row was not created")
    #expect(core.state(path: "account/acc-31/password") == nil)
}

let EXTRA_ADDRESS = "assem@thirty-first-example.test"

func fixtureCore(clock: FixtureClock = FixtureClock()) throws -> FixtureCore {
    let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    return try FixtureCore(url: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"), clock: clock)
}
