import Foundation
import Testing
@testable import WhooshUI

// fn-pushclient-ios (proof file checks/proofs/g2/acefe421.json): the REAL RelayClient and the REAL PushRegistrar against a local fake of the
// shipped relay on the loopback address (127.0.0.1:<port>). The fake answers exactly the rows of relay/src/routes/mod.rs:33-35 and the rules of
// relay/src/routes/registrations.rs:18-63 and relay/src/registry/devices.rs:25-52, and 404 or 405 for every other path or method, which is what
// the old PUT /v1/registrations/{account_id} client got from the real relay.
// The fake is a URLProtocol on a URLSession, not a listening socket: the request, its method, path and JSON body are what the client really builds.
// MUTANTS (red-first broken copies):
//   theClientSpeaksTheRowsTheShippedRelayServes        : RelayClient.swift `request.httpMethod = "POST"` -> `"PUT"`
//   theClientDeletesByRoutingIdAndTokenAndSendsNoAccountId : RelayClient.swift `.appendingPathComponent(routingId).appendingPathComponent(deviceToken)` -> `.appendingPathComponent(routingId)`
//   thirtyAccountsHoldOneRowEachOnTheRelayAndNoneAfterRemoval : PushRegistrar.swift `try await sweepStale()` in setToken removed
//   theClientRefusesAnAnswerThatIsNotTheRelaysOk       : RelayClient.swift `ok: [200, 201]` -> `ok: [200, 201, 500]`

private final class RelayFake: URLProtocol, @unchecked Sendable {
    struct World { var rows = Set<String>(); var log: [String] = []; var bodies: [String] = []; var forceStatus: Int? }
    nonisolated(unsafe) static var worlds: [Int: World] = [:]
    static let lock = NSLock()
    static func with<T>(_ port: Int, _ body: (inout World) -> T) -> T { lock.withLock { body(&worlds[port, default: World()]) } }

    override class func canInit(with request: URLRequest) -> Bool { request.url?.host == "127.0.0.1" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func stopLoading() {}

    private func bodyData() -> Data {
        if let b = request.httpBody { return b }
        guard let s = request.httpBodyStream else { return Data() }
        s.open(); defer { s.close() }
        var out = Data(); var buf = [UInt8](repeating: 0, count: 4096)
        while s.hasBytesAvailable { let n = s.read(&buf, maxLength: buf.count); if n <= 0 { break }; out.append(buf, count: n) }
        return out
    }
    private static func hex(_ s: String) -> Bool { !s.isEmpty && s.allSatisfy { $0.isHexDigit } }
    private static func routing(_ s: String) -> Bool { !s.isEmpty && s.allSatisfy { $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "-" || $0 == "_") } }

    override func startLoading() {
        let url = request.url!, port = url.port ?? 0, method = request.httpMethod ?? "GET"
        let parts = url.path.split(separator: "/").map(String.init)
        let body = bodyData()
        var status = 404
        Self.with(port) { w in
            w.log.append("\(method) \(url.path)")
            if let forced = w.forceStatus { status = forced; return }
            if parts == ["v1", "registrations"] {
                guard method == "POST" else { status = 405; return }
                w.bodies.append(String(data: body, encoding: .utf8) ?? "")
                guard let obj = try? JSONSerialization.jsonObject(with: body) as? [String: Any],
                      let tok = obj["device_token"] as? String, let rid = obj["routing_id"] as? String, let env = obj["environment"] as? String,
                      Self.hex(tok), Self.routing(rid), env == "sandbox" || env == "production" else { status = 400; return }
                let key = "\(rid)/\(tok.lowercased())"
                status = w.rows.contains(key) ? 200 : 201
                w.rows.insert(key)
            } else if parts.count == 4, parts[0] == "v1", parts[1] == "registrations" {
                guard method == "DELETE" else { status = 405; return }
                guard Self.routing(parts[2]), Self.hex(parts[3]) else { status = 400; return }
                status = w.rows.remove("\(parts[2])/\(parts[3].lowercased())") != nil ? 204 : 404
            }
        }
        let response = HTTPURLResponse(url: url, statusCode: status, httpVersion: "HTTP/1.1", headerFields: ["Content-Type": "application/json"])!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data("{}".utf8))
        client?.urlProtocolDidFinishLoading(self)
    }
}

private func client(port: Int) throws -> RelayClient {
    let cfg = URLSessionConfiguration.ephemeral
    cfg.protocolClasses = [RelayFake.self]
    return try RelayClient(baseURL: URL(string: "http://127.0.0.1:\(port)")!, session: URLSession(configuration: cfg))
}

private let tok1 = String(repeating: "ab", count: 32), tok2 = String(repeating: "cd", count: 32)

@Test func theClientSpeaksTheRowsTheShippedRelayServes() async throws {
    let port = 41101, c = try client(port: port)
    let reg = RelayRegistration(deviceToken: tok1, routingId: "r-one", environment: .sandbox)
    try await c.register(reg)                       // 201: the relay answers a new pair
    try await c.register(reg)                       // 200: the same pair again is idempotent
    let (rows, log, bodies) = RelayFake.with(port) { ($0.rows, $0.log, $0.bodies) }
    #expect(log == ["POST /v1/registrations", "POST /v1/registrations"], "the rows the relay serves: \(log)")
    #expect(rows == ["r-one/\(tok1)"], "one row for the pair, not two")
    let json = try #require(try JSONSerialization.jsonObject(with: Data(bodies[0].utf8)) as? [String: String])
    #expect(json == ["device_token": tok1, "routing_id": "r-one", "environment": "sandbox"], "the body is the relay's RegisterBody and says no account id or platform")
}

@Test func theClientDeletesByRoutingIdAndTokenAndSendsNoAccountId() async throws {
    let port = 41102, c = try client(port: port)
    try await c.register(RelayRegistration(deviceToken: tok1, routingId: "r-one", environment: .production))
    try await c.unregister(routingId: "r-one", deviceToken: tok1)   // 204
    try await c.unregister(routingId: "r-one", deviceToken: tok1)   // 404 = none held, also accepted
    let (rows, log) = RelayFake.with(port) { ($0.rows, $0.log) }
    #expect(log.suffix(2) == ["DELETE /v1/registrations/r-one/\(tok1)", "DELETE /v1/registrations/r-one/\(tok1)"], "\(log)")
    #expect(rows.isEmpty, "none after removal")
}

@Test func theClientRefusesAnAnswerThatIsNotTheRelaysOk() async throws {
    let port = 41103, c = try client(port: port)
    RelayFake.with(port) { $0.forceStatus = 500 }
    await #expect(throws: RelayError.status(500)) { try await c.register(RelayRegistration(deviceToken: tok1, routingId: "r", environment: .sandbox)) }
    await #expect(throws: RelayError.status(500)) { try await c.unregister(routingId: "r", deviceToken: tok1) }
    RelayFake.with(port) { $0.forceStatus = 405 }
    await #expect(throws: RelayError.status(405)) { try await c.register(RelayRegistration(deviceToken: tok1, routingId: "r", environment: .sandbox)) }
}

@Test func thirtyAccountsHoldOneRowEachOnTheRelayAndNoneAfterRemoval() async throws {
    let port = 41104
    let sink = NoopSink(), store = MemoryRoutingStore()
    let reg = PushRegistrar(relay: try client(port: port), sink: sink, store: store, environment: .sandbox)
    let accounts = (1...30).map { "acct-\($0)" }
    for a in accounts { try await reg.register(accountId: a) }
    try await reg.setToken(Data(repeating: 0xab, count: 32))
    var rows = RelayFake.with(port) { $0.rows }
    #expect(rows.count == 30, "after J4.10: one row for each of the 30 accounts (\(rows.count))")
    #expect(Set(rows.map { String($0.split(separator: "/")[0]) }).count == 30)
    try await reg.setToken(Data(repeating: 0xcd, count: 32))
    rows = RelayFake.with(port) { $0.rows }
    #expect(rows.count == 30 && rows.allSatisfy { $0.hasSuffix("/\(tok2)") }, "a new token leaves one row per account, none under the old token")
    let gone = try #require(store.routingId(for: "acct-9"))
    try await reg.unregister(accountId: "acct-9")
    rows = RelayFake.with(port) { $0.rows }
    #expect(rows.count == 29 && !rows.contains { $0.hasPrefix("\(gone)/") }, "none for the removed account")
    try await reg.setToken(Data(repeating: 0xab, count: 32))
    rows = RelayFake.with(port) { $0.rows }
    #expect(rows.count == 29 && !rows.contains { $0.hasPrefix("\(gone)/") }, "a later token change does not bring it back")
}

private final class NoopSink: PushSink, @unchecked Sendable {
    func registerRouting(accountId: String, routingId: String) throws {}
    func unregisterRouting(accountId: String) throws {}
}
