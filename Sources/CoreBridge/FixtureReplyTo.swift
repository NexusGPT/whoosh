import Foundation

// fix-replydoors: the test engine answers the two Reply-To state paths the real engine serves (compose_cmds.rs `state`), in the same shape.
// The fixture file has no Reply-To field (FixtureFile.Message), so `message/<id>/reply_to` is always "[]" and `message/<id>/reply_recipients`
// is the plain recipients of the engine's rule with no Reply-To: the sender; a mail the owner wrote goes to its recipients minus the owner.
// Wiring: FixtureCore.state(path:) must call `stateReplyToServed` (one line, next to `stateTestsServed`; FixtureCore.swift is not this lane's).
extension FixtureCore {
    /// nil = not one of these paths; a value of nil inside = no such message.
    func stateReplyToServed(_ path: String) -> TestsServed? {
        let p = path.split(separator: "/", omittingEmptySubsequences: false).map(String.init)
        guard p.count == 3, p[0] == "message", p[2] == "reply_to" || p[2] == "reply_recipients" else { return nil }
        return TestsServed(value: lock.withLock { () -> String? in
            guard let m = fxMessage(p[1]) else { return nil }
            if p[2] == "reply_to" { return "[]" }
            let own = ownAddressesLocked()
            let isOwn: (String) -> Bool = { a in own.contains { $0.caseInsensitiveCompare(a) == .orderedSame } }
            var out: [String] = []
            for a in isOwn(m.from) ? (m.to ?? []) : [m.from] where !isOwn(a) && !out.contains(where: { $0.caseInsensitiveCompare(a) == .orderedSame }) { out.append(a) }
            let data = try? JSONSerialization.data(withJSONObject: out, options: [])
            return data.flatMap { String(data: $0, encoding: .utf8) }
        })
    }
}
