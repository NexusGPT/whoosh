import Foundation
import Observation

/// One outside app the owner can allow. The name is the identity the MCP server reports for the client.
struct McpApp: Equatable, Identifiable {
    let name: String
    var allowed: Bool
    var id: String { name }
    var slug: String { McpWords.slug(name) }
}

/// The pane's state. Every input changes it on the frame it lands (docs/MOTION.md law 1); the views only animate after.
///
/// The agents core reports neither a server state nor the apps it has seen (docs/lanes/mcpnotesmac.md, round 2 gaps), so
/// both start from `Known`. Grants are read from and written through `McpGrantDoor`.
@MainActor @Observable
final class McpSession {
    static let shared = McpSession()

    enum Known {
        static let apps = ["Claude Desktop", "Cursor"]
    }

    /// One request an allowed app made that needs the owner's yes (`McpApprovalNeeded`); answered once, allow or refuse.
    struct Request: Identifiable, Equatable {
        let id: String
        let app: String
        let tool: String
        let words: String
    }

    var serverOn = true
    var apps: [McpApp] = Known.apps.map { McpApp(name: $0, allowed: false) }
    /// The requests waiting for the owner, oldest first.
    private(set) var requests: [Request] = []
    private var subscription: UInt64?
    /// The reason the last grant could not be saved, in words; nil when it could.
    var failure: String?

    /// Allowed apps first, each group in its first-seen order: allowing an app moves its row up (ROW_INSERT_MS).
    var ordered: [McpApp] { apps.filter(\.allowed) + apps.filter { !$0.allowed } }
    var allowedCount: Int { apps.filter(\.allowed).count }

    @discardableResult
    func set(_ name: String, allowed: Bool) -> Bool {
        guard let i = apps.firstIndex(where: { $0.name == name }) else { return false }
        if apps[i].allowed == allowed { return true }   // already as asked: the click reached its goal
        if let reason = McpGrantDoor.write(app: name, allowed: allowed) { failure = reason } else { failure = nil }
        apps[i].allowed = allowed
        syncFromCore()
        return true
    }

    @discardableResult
    func start() -> Bool {
        if serverOn { return true }
        _ = AppModel.shared.perform(.setMcpServer(on: true))
        serverOn = true
        return true
    }

    /// Listens for the approvals the core asks for. Once is enough; a second call changes nothing.
    func subscribe() {
        guard subscription == nil else { return }
        subscription = AppModel.shared.core.subscribe(stream: .agent, sink: McpApprovalSink { [weak self] r in
            guard let self, !self.requests.contains(where: { $0.id == r.id }) else { return }
            self.requests.append(r)
        })
    }

    /// The owner's answer to one request: allow it once or refuse it. The request leaves the list when the core took the answer.
    @discardableResult
    func answer(_ id: String, allowed: Bool) -> Bool {
        guard requests.contains(where: { $0.id == id }) else { return false }
        if AppModel.shared.perform(.answerMcpApproval(requestId: id, allowed: allowed)) {
            failure = nil
            requests.removeAll { $0.id == id }
            return true
        }
        failure = AppModel.shared.lastFailure?.reasonWords ?? McpWords.answerFailed
        return false
    }

    /// Interim fixture verb: the server goes off, as the core will report it when it stops.
    func stopForFixture() -> Bool { guard serverOn else { return false }; serverOn = false; return true }

    /// Believe the core where it serves a grant (a restart, or another window, changed it).
    func syncFromCore() {
        subscribe()
        for g in AppModel.shared.core.mcpGrants() where !apps.contains(where: { $0.name == g.appName }) {
            apps.append(McpApp(name: g.appName, allowed: g.allowed))
        }
        if let json = AppModel.shared.core.state(path: "mcp_server/on"),
           let on = try? JSONSerialization.jsonObject(with: Data(json.utf8), options: [.fragmentsAllowed]) as? Bool, on != serverOn { serverOn = on }
        for i in apps.indices { if let g = McpGrantDoor.read(app: apps[i].name), g != apps[i].allowed { apps[i].allowed = g } }
    }

    func app(slug: String) -> McpApp? { apps.first { $0.slug == slug } }
}

/// Where a grant is written: the core's `SetMcpGrant`. A refusal reads in the core's words; the old session-only notice stays
/// only for a core that answers without words. The core is read first (`mcp_grant/<app>`), so a
/// core that serves the path is believed over the session.
enum McpGrantDoor {
    /// nil when the core saved it; otherwise the reason, in words.
    @MainActor static func write(app: String, allowed: Bool) -> String? {
        if AppModel.shared.perform(.setMcpGrant(appName: app, allowed: allowed)) { return nil }
        return AppModel.shared.lastFailure?.reasonWords ?? McpWords.grantUnsaved
    }

    /// The core's answer for one app: true, false, or nil when it does not serve the path.
    @MainActor static func read(app: String) -> Bool? {
        guard let json = AppModel.shared.core.state(path: "mcp_grant/\(app)"),
              let v = try? JSONSerialization.jsonObject(with: Data(json.utf8), options: [.fragmentsAllowed]) as? Bool else { return nil }
        return v
    }
}


/// Receives the agent stream and keeps the approvals the owner is asked for.
final class McpApprovalSink: EventSink, @unchecked Sendable {
    private let handle: @MainActor (McpSession.Request) -> Void
    init(_ handle: @escaping @MainActor (McpSession.Request) -> Void) { self.handle = handle }
    func onEvent(event: CoreEvent) {
        guard case .mcpApprovalNeeded(let id, let app, let tool, let words) = event else { return }
        let r = McpSession.Request(id: id, app: app, tool: tool, words: words)
        if Thread.isMainThread { MainActor.assumeIsolated { handle(r) } } else { Task { @MainActor in handle(r) } }
    }
}
