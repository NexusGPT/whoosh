import Foundation

/// State paths the composer's journey (J3) reads from the fixture core: the AI draft's job note and the per-recipient delivery count.
/// Both answer from what the fixture holds (the outbox entries, the agent roster, the job the composer reported), never from a constant.
///
/// `ai_job/last/agent` and `ai_job/last/voice_profile_address`: the frozen contract has no command for an AI draft, so the composer's draft
/// runs app-side; when it succeeds the composer reports the job here (`noteAIJob`), and the agent is the roster's built-in Writer (the name the
/// agent roster serves), the voice profile the address the draft was written for. Nothing is served before a draft ran, and `fixture reset` takes the note away (`aiJobNote`, a stored property reset in `restoreSeedLocked`).
/// `server/<address>/inbox/count`: the sent outbox entries that go to the address (To, Cc, Bcc; a quick-send reply goes to its thread's sender) (a cancelled, waiting or scheduled entry
/// is not delivered; the clock settles what is due first).
struct FixtureAIJobNote: Equatable, Sendable { var agent: String; var address: String }

extension FixtureCore {
    /// The composer's AI draft ran for `address` (the account the message goes out from).
    public func noteAIJob(voiceProfileAddress address: String) {
        let agent = lock.withLock { agents.agents.first { $0.id == "writer" }?.name ?? "" }
        lock.withLock { aiJobNote = FixtureAIJobNote(agent: agent == "Writer" ? "built-in Writer" : agent, address: address) }
    }

    /// Who an entry goes to: its To, Cc and Bcc; a quick send of a reply (`quickSend` keeps no To) goes to the sender of the thread's latest incoming message.
    func recipientsLocked(_ d: Draft) -> [String] {
        var all = d.to + d.cc + d.bcc
        if d.to.isEmpty, let t = d.replyToThreadId {
            let thread = threadOf(t)
            if let last = messages.last(where: { $0.outgoing != true && threadOf($0.id) == thread }) { all.append(last.from) }
        }
        return all
    }

    /// nil = not one of these paths; a value of nil inside = no such value yet (the socket answers "no core value").
    func stateComposerServed(_ path: String) -> TestsServed? {
        if let rich = stateRichServed(path) { return rich }   // gap-composer-rich: sent/last/* (FixtureCoreRich.swift)
        func json(_ v: Any) -> String? { (try? JSONSerialization.data(withJSONObject: v, options: [.fragmentsAllowed])).flatMap { String(data: $0, encoding: .utf8) } }
        switch path {
        case "ai_job/last/agent": return TestsServed(value: lock.withLock { aiJobNote }.flatMap { json($0.agent) })
        case "ai_job/last/voice_profile_address": return TestsServed(value: lock.withLock { aiJobNote }.flatMap { json($0.address) })
        default: break
        }
        let pre = "server/", post = "/inbox/count"
        guard path.hasPrefix(pre), path.hasSuffix(post), path.count > pre.count + post.count else { return nil }
        let address = String(path.dropFirst(pre.count).dropLast(post.count)).lowercased()
        return TestsServed(value: lock.withLock { () -> String? in
            settleOutboxLocked()
            let n = entries.filter { e in
                guard case .sent = e.state else { return false }
                return recipientsLocked(e.draft).contains { $0.lowercased() == address }
            }.count
            return json(n)
        })
    }
}
