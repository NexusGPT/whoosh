import Foundation

// fn-pushclient-ios. The relay address is the user's setting (RelaySettings.swift); no relay address is built in.
// The calls are the rows the shipped relay serves (relay/src/routes/mod.rs:33-35, relay/src/routes/registrations.rs):
//   POST   /v1/registrations                          body {"device_token","routing_id","environment":"sandbox"|"production"} -> 201 (new) or 200 (same pair again)
//   DELETE /v1/registrations/{routing_id}/{device_token} -> 204, or 404 when none held (both mean "none held")
// The relay keys a row by the pair (device_token, routing_id) (relay/src/registry/devices.rs:25-37); the account id never leaves the phone.
// The registration pattern (device token + opaque routing id, one row per account, delete on unregister) follows
// chatmail/notifiers (Apache-2.0/MIT, research/ios-push.md section 4); no code is copied.

/// The APNs environment the relay signs for (relay/src/routes/registrations.rs:33-37). A debug build gets sandbox tokens, a release build production ones.
public enum RelayEnvironment: String, Codable, Sendable, Equatable {
    case sandbox, production
    public static var forThisBuild: RelayEnvironment {
        #if DEBUG
        return .sandbox
        #else
        return .production
        #endif
    }
}

public struct RelayRegistration: Codable, Sendable, Equatable {
    public var deviceToken: String
    public var routingId: String
    public var environment: RelayEnvironment
    enum CodingKeys: String, CodingKey { case deviceToken = "device_token", routingId = "routing_id", environment }
}

public enum RelayError: Error, Equatable {
    case notLoopback(String)
    case status(Int)
}

public protocol RelayAPI: Sendable {
    func register(_ registration: RelayRegistration) async throws
    func unregister(routingId: String, deviceToken: String) async throws
}

public struct RelayClient: RelayAPI {
    private let base: URL
    private let session: URLSession
    /// A test copy (`RigNet`, `env` nil: this process) follows no redirect to a host off this Mac; nil on the owner's copy, where
    /// URLSession follows redirects as before (rig network audit, gap 7).
    let redirectGuard: RigRedirectGuard?

    /// Nothing leaves this Mac while the owner is away: only a loopback relay is accepted until a later lane lifts that.
    public init(baseURL: URL, session: URLSession = .shared, loopbackOnly: Bool = true, env: [String: String]? = nil) throws {
        if loopbackOnly {
            let host = baseURL.host ?? ""
            guard RigNet.isThisMac(host) else { throw RelayError.notLoopback(host) }
        }
        self.base = baseURL; self.session = session
        self.redirectGuard = RigNet.rig(env: env) ? RigRedirectGuard(env: env) : nil
    }

    private var registrations: URL { base.appendingPathComponent("v1").appendingPathComponent("registrations") }

    public func register(_ registration: RelayRegistration) async throws {
        var request = URLRequest(url: registrations); request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONEncoder().encode(registration)
        try check(try await session.data(for: request, delegate: redirectGuard).1, ok: [200, 201])
    }

    public func unregister(routingId: String, deviceToken: String) async throws {
        let url = registrations.appendingPathComponent(routingId).appendingPathComponent(deviceToken)
        var request = URLRequest(url: url); request.httpMethod = "DELETE"
        try check(try await session.data(for: request, delegate: redirectGuard).1, ok: [204, 404])
    }

    private func check(_ response: URLResponse, ok: Set<Int>) throws {
        let code = (response as? HTTPURLResponse)?.statusCode ?? -1
        guard ok.contains(code) else { throw RelayError.status(code) }
    }
}

/// A test copy's redirect rule: a redirect to a host off this Mac is not followed (`RigNet.allows`), so the task ends on the redirect's own
/// answer (a 3xx, which `check` refuses) and nothing is asked outside this Mac.
final class RigRedirectGuard: NSObject, URLSessionTaskDelegate, @unchecked Sendable {
    let env: [String: String]?
    init(env: [String: String]?) { self.env = env }

    /// The request the redirect may go on with, or nil.
    func next(_ request: URLRequest) -> URLRequest? { RigNet.allows(request.url, env: env) ? request : nil }

    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest) async -> URLRequest? {
        next(request)
    }
}
