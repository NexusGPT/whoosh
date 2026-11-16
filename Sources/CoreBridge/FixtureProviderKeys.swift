import Foundation

/// A pasted provider key, the way the REAL core takes it (engine/src/ai_settings.rs, docs/lanes/ai-settings-core.md): `SaveProviderKey`
/// answers at once; the provider's answer lands later on `ai_provider/<id>/state` ("checking", then "connected" or "refused"; Jev:
/// "not checked"), `ai_provider/<id>/refusal` (the provider's words) and `ai_provider/<id>/models`. The fixture's check runs on its own
/// queue and lands when the FIXTURE CLOCK passes `keyCheckMs` after the save, so `clock advance` drives it in a check, and with no advance
/// it lands by real time on the next read. Nothing is sent anywhere except to the endpoint a check named (`fixture provider-endpoint`). On a test
/// copy that endpoint must be on this Mac (127.0.0.1, ::1, localhost, `RigNet`): the verb refuses any other, and `keyCheckURL` refuses it again
/// before the POST, so a pasted key never leaves this Mac.
struct FixtureKeyCheck {
    enum Phase { case checking, connected, refused, saved }
    enum Verdict { case accepted([String]), refused(String) }
    var phase: Phase = .checking
    var token = 0                 // the newest save of this provider; an older answer is dropped
    var dueMs: Int64 = 0          // the fixture clock time the answer may land
    var verdict: Verdict?         // the queue's answer, kept until the clock reaches `dueMs`
    var refusal: String?
    var key: String?              // hid-ai: Jev's saved key, held in memory as the test engine's Keychain (`fixture decide` sends it in the header); never served by a read
}

/// Fixture-only: how long (fixture clock) a provider takes to answer a key. Not a product constant, so not in docs/CONSTANTS.md.
let FIXTURE_KEY_CHECK_MS: Int64 = 600

private final class KeyTokens: @unchecked Sendable {
    private let lock = NSLock(); private var n = 0
    func next() -> Int { lock.withLock { n += 1; return n } }
}

/// What a provider's key check answered; filled on the URL session's queue, read after the semaphore.
private final class ProviderAnswer: @unchecked Sendable {
    var status = 0
    var body = Data()
}

extension FixtureCore {
    /// The fixture's own queue for key checks (serial: a verdict is always in before a later settle that drained it).
    private static let keyQueue = DispatchQueue(label: "whoosh.fixture.key-check")
    private static let tokens = KeyTokens()

    /// SaveProviderKey, with the lock held: records "checking" and starts the verdict; answers at once.
    func beginKeyCheckLocked(_ id: String, key raw: String) throws {
        let key = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if key.isEmpty { throw CoreError.Invalid(field: "key", reasonWords: "Paste the key first.") }
        var c = FixtureKeyCheck()
        c.token = Self.tokens.next()
        c.dueMs = clock.nowMs() + FIXTURE_KEY_CHECK_MS
        if id == "jev" {      // the decision model's key is saved, never checked
            c.phase = .saved
            c.key = key
            agents.credentialStored.insert(id)
            agents.keyChecks[id] = c
            return
        }
        agents.keyChecks[id] = c
        let name = agents.provider(id)?.name ?? id
        let seed = agents.provider(id)?.models ?? []
        let endpoint = providerEndpoints[id]
        let token = c.token
        Self.keyQueue.async { [weak self] in
            let v = Self.verdict(name: name, seed: seed, endpoint: endpoint, key: key)
            self?.lock.withLock {
                guard let self, self.agents.keyChecks[id]?.token == token else { return }
                self.agents.keyChecks[id]?.verdict = v
            }
        }
    }

    /// Lands every check whose verdict is in and whose time has come. Called with the lock held, before any AI read.
    func settleKeyChecksLocked() {
        let now = clock.nowMs()
        for (id, c) in agents.keyChecks where c.phase == .checking {
            guard let v = c.verdict, now >= c.dueMs else { continue }
            switch v {
            case .accepted(let models):
                agents.keyChecks[id]?.phase = .connected
                agents.keyChecks[id]?.refusal = nil
                agents.credentialStored.insert(id)
                if let i = agents.providers.firstIndex(where: { $0.id == id }) {
                    agents.providers[i].connected = true
                    agents.providers[i].models = models
                    agents.providers[i].keychain = KeychainRef(service: "whoosh.provider", account: id)
                }
            case .refused(let words):      // nothing is kept: a key held from before stays as it was
                agents.keyChecks[id]?.phase = .refused
                agents.keyChecks[id]?.refusal = words
            }
        }
    }

    /// `clock advance` reached the fixture: let the queue's verdicts in, then land what is due.
    func settleKeyChecksAfterClock() {
        Self.keyQueue.sync { }
        lock.withLock { settleKeyChecksLocked() }
    }

    /// RemoveProviderKey: the key, the connection and any check in flight go.
    func forgetKeyLocked(_ id: String) {
        agents.keyChecks[id] = nil
        agents.credentialStored.remove(id)
        if let i = agents.providers.firstIndex(where: { $0.id == id }) { agents.providers[i].connected = false; agents.providers[i].keychain = nil }
    }

    /// `ai_provider/<id>/state|refusal|models`: the same JSON kinds and words as the real core (string, string, strings).
    func aiProviderStateLocked(_ id: String, _ leaf: String) -> Any? {
        settleKeyChecksLocked()
        let known = id == "local" || FixtureSettingsBook.credentialIds.contains(id)
        guard known else { return nil }
        let c = agents.keyChecks[id]
        switch leaf {
        case "state":      // the words are the contract's (ai_provider_state.rs, delta 24)
            if id == "local" { return aiProviderStateKey(s: .connected) }
            switch c?.phase {
            case .checking?: return aiProviderStateKey(s: .checking)
            case .refused?: return aiProviderStateKey(s: .refused)
            case .saved?: return aiProviderStateKey(s: .notChecked)
            case .connected?: return aiProviderStateKey(s: .connected)   // hid-ai: Jev's key after a Test the endpoint answered (no provider row holds Jev)
            default: return aiProviderStateKey(s: agents.provider(id)?.connected == true ? .connected : .notConnected)
            }
        case "refusal": return c?.phase == .refused ? c?.refusal : nil
        case "models": return agents.provider(id).map { $0.connected ? $0.models : [] } ?? []
        default: return nil
        }
    }

    /// Where a key is asked: `<endpoint>/validate`. On a test copy only when the host is this Mac (`RigNet.allows`); nil means refused,
    /// and nothing is sent. `env` nil is this process.
    static func keyCheckURL(_ base: String, env: [String: String]? = nil) -> URL? {
        guard let url = URL(string: base + "/validate"), RigNet.allows(url, env: env) else { return nil }
        return url
    }

    /// `fixture provider-endpoint` and the pane's reset: the endpoint names where a key is asked.
    private static func verdict(name: String, seed: [String], endpoint: String?, key: String) -> FixtureKeyCheck.Verdict {
        let fallback = "\(name) did not accept that key"
        guard let base = endpoint else {
            return key.count >= 8 && !key.contains(where: \.isWhitespace) ? .accepted(seed) : .refused(fallback)
        }
        guard URL(string: base + "/validate") != nil else { return .refused(fallback) }
        guard let url = keyCheckURL(base) else { return .refused(RigNet.refusedWords) }   // a test copy, a host off this Mac: nothing is sent
        var req = URLRequest(url: url, timeoutInterval: 3)
        req.httpMethod = "POST"
        req.setValue("Bearer " + key, forHTTPHeaderField: "Authorization")
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = Data("{}".utf8)
        FixtureProviderRequests.note()   // hid-ai F2: a key check sent off this process counts as a provider request
        let box = ProviderAnswer()
        let done = DispatchSemaphore(value: 0)
        URLSession.shared.dataTask(with: req) { data, response, _ in
            box.status = (response as? HTTPURLResponse)?.statusCode ?? 0
            box.body = data ?? Data()
            done.signal()
        }.resume()
        guard done.wait(timeout: .now() + 4) == .success else { return .refused("\(name) did not answer") }
        let json = (try? JSONSerialization.jsonObject(with: box.body)) as? [String: Any]
        guard box.status == 200 else { return .refused((json?["error"] as? String) ?? fallback) }
        return .accepted((json?["models"] as? [String]) ?? seed)
    }
}
