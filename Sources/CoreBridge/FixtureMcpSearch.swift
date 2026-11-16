import Foundation

/// Contracts delta 13: an outside app's `search_mail` over the fixture core, in the order the real gate keeps
/// (core/crates/mcp usecases/search_mail.rs): the server is on, the app holds a grant, the owner says yes, then the
/// search runs. Nothing is searched before the yes; an app with no grant gets nothing and no prompt.
extension FixtureCore {
    /// `fixture mcp-search <app> [query]`. Returns the request id the owner answers with `AnswerMcpApproval`; the
    /// results are read at `mcp_request/<id>/results` (subjects) once approved. Refused in words when the server is
    /// off or the app has no grant: no request is made and no event is sent.
    public func mcpSearch(app: String, query: String) throws -> String {
        let (id, sinks): (String, [EventSink]) = try lock.withLock {
            guard settings.mcpOn else { throw CoreError.Invalid(field: "server", reasonWords: "the MCP server is off, so no app can search") }
            guard settings.mcpAllowedNow(app) else { throw CoreError.Invalid(field: "app", reasonWords: "\(app) has no grant on the MCP server, so it gets nothing") }
            let id = newId("mcp-req")
            mcpRequests[id] = (app, "search_mail", "pending")
            mcpQueries[id] = query
            return (id, self.sinks.values.filter { $0.0 == .agent }.map(\.1))
        }
        let words = query.isEmpty ? "search your mail" : "search your mail for “\(query)”"
        Self.deliver(sinks.map { ($0, CoreEvent.mcpApprovalNeeded(requestId: id, appName: app, toolName: "search_mail", requestWords: words)) })
        return id
    }

    /// The subjects of the messages still in the mail that carry every word of `query` in their subject, sender or body,
    /// newest first. Called with the lock held, after the owner approved.
    func mcpSearchSubjectsLocked(_ query: String) -> [String] {
        let words = query.lowercased().split(whereSeparator: { !$0.isLetter && !$0.isNumber }).map(String.init)
        return order.compactMap { id -> (String, Int64)? in
            guard !deleted.contains(id), !spam.contains(id), let m = fxMessage(id) else { return nil }
            let text = ([m.subject, m.sender, m.from, m.preview] + (m.body ?? [])).joined(separator: " ").lowercased()
            return words.allSatisfy(text.contains) ? (m.subject, receivedAt[id] ?? 0) : nil
        }.sorted { $0.1 > $1.1 }.map(\.0)
    }
}
