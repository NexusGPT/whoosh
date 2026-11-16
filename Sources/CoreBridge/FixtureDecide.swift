import Foundation

/// `fixture decide <page> <message> <label> <confidence>` in the test engine (hid-ai): ONE message of the sample below goes through the list
/// the page holds now (the same `PreviewEntry` list the preview sends), as docs/DECISION-MODEL.md v2 §1-2 says a list decides: the first ticked
/// entry that answers wins; a rule answers when its condition holds; an "ask the model" entry answers only with the engine on, at or above its
/// trust level (inclusive); the closing default always answers. The engine is the saved choice (`decision_model/provider`): Off (`rules`) skips
/// every model entry; Local answers with the verb's own label and confidence (the on-device model is scripted here); Jev is asked only after the
/// owner's consent, over HTTP at the endpoint a check named (`fixture provider-endpoint jev <url>`, on this Mac only: `RigNet`), with the key in
/// the Authorization header and only the fields §2 lists, the body cut at DECISION_BODY_CHARS. Nothing is written: the answer goes back to the
/// page, which shows why the message was decided as it was.
struct FixtureDecideMessage {
    let from: String
    let subject: String
    let bodyLength: Int
    var listHeader = false, fromMe = false, needsReply = false
}

/// What one decision answered: the entry (1-based) that decided, the outcome, who decided in the page's words, and whether a model was asked.
public struct FixtureDecision: Sendable {
    public let entry: Int?
    public let outcome: String
    public let decidedBy: String
    public let modelCalled: Bool
}

enum FixtureDecideSample {
    /// The owner's VIP list in the sample (the VIPs page's first rule reads it).
    static let vipList: Set<String> = ["orders@linea-supply-example.com"]

    /// The messages a check names by key (fixtures/tests-j5.json `decision.messages`, the same senders, subjects and facts).
    static let messages: [String: FixtureDecideMessage] = [
        "vip": .init(from: "orders@linea-supply-example.com", subject: "Quote for the Q1 print run: 12,000 units", bodyLength: 300),
        "list": .init(from: "noreply@github-example.com", subject: "[whoosh] Pull request #42", bodyLength: 200, listHeader: true),
        "promo": .init(from: "news@marathon-example.run", subject: "Brussels 20k: bib pickup times", bodyLength: 400),
        "ask": .init(from: "jonas@orbital-example.be", subject: "Can we move Thursday to 15:00?", bodyLength: 250, needsReply: true),
        "mine": .init(from: "me@assem-example.com", subject: "Notes to self", bodyLength: 90, fromMe: true),
        "code": .init(from: "security@service-example.com", subject: "Your verification code is 482913", bodyLength: 120),
        "welcome": .init(from: "hello@service-example.com", subject: "Welcome aboard", bodyLength: 500),
        "invoice": .init(from: "noreply@hostcloud-example.net", subject: "Invoice 2026-11 is available", bodyLength: 300),
        "long": .init(from: "bart@tenders-example.be", subject: "Tender AQP: long body", bodyLength: 2500),
        "mid": .init(from: "karim@capital-example.lu", subject: "Term sheet v2, redlines", bodyLength: 1500),
    ]

    /// A rule's match key (`DecisionEntry.match`) against one message.
    static func matches(_ key: String, _ m: FixtureDecideMessage) -> Bool {
        let subject = m.subject.lowercased()
        switch key {
        case "vip": return vipList.contains(m.from.lowercased())
        case "list": return m.listHeader
        case "me": return m.fromMe
        case "code": return ["code", "verification", "sign in"].contains { subject.contains($0) }
        case "reply": return m.needsReply
        default:
            if key.hasPrefix("from:") { return m.from.lowercased().contains(String(key.dropFirst(5)).lowercased()) }
            if key.hasPrefix("subject:") { return subject.contains(String(key.dropFirst(8)).lowercased()) }
            return false
        }
    }

    /// The match key of a rule written in words (`fixture add-rule`): "subject contains Invoice" -> "subject:invoice", "from x" -> "from:x"; nil otherwise.
    static func matchKey(_ words: String) -> String? {
        let lower = words.trimmingCharacters(in: .whitespaces).lowercased()
        for (prefix, key) in [("subject contains ", "subject:"), ("from ", "from:")] where lower.hasPrefix(prefix) {
            return key + lower.dropFirst(prefix.count).trimmingCharacters(in: .whitespaces)
        }
        return nil
    }

    /// The model's label as the page names its outcome: a tab name starts with a capital ("promotions" -> "Promotions"); a yes or no stays as said.
    static func outcome(page: String, label: String) -> String {
        page == "which-tab" ? label.prefix(1).uppercased() + label.dropFirst() : label
    }

    /// Runs the list. `ask` answers a model entry's question (label, confidence 0...1), or nil when the model gives no answer (no consent, no endpoint).
    static func decide(_ m: FixtureDecideMessage, page: String, entries: [PreviewEntry], engineOn: Bool,
                       ask: (_ question: String) -> (String, Double)?) -> FixtureDecision {
        var called = false
        for (i, e) in entries.enumerated() where e.enabled {
            let n = i + 1
            switch e.kind {
            case .rule:
                if matches(e.matchKey, m) { return FixtureDecision(entry: n, outcome: e.outcome, decidedBy: "entry \(n) · rule", modelCalled: called) }
            case .model:
                guard engineOn, !e.question.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, let (label, conf) = ask(e.question) else { continue }
                called = true
                let percent = Int((conf * 100).rounded())
                if percent >= Int(e.trust ?? 100) {
                    let out = outcome(page: page, label: label)
                    return FixtureDecision(entry: n, outcome: out, decidedBy: String(format: "entry %d · the model said %@ (%.2f)", n, out, conf), modelCalled: true)
                }
            case .otherwise:
                return FixtureDecision(entry: n, outcome: e.outcome, decidedBy: "entry \(n) · otherwise", modelCalled: called)
            }
        }
        return FixtureDecision(entry: nil, outcome: "—", decidedBy: "no entry answered", modelCalled: called)
    }
}

/// `provider_request/count` (hid-ai, F2): every request the test engine sends, or stands in for, to an AI provider off this Mac: a key check
/// sent to an endpoint, a question to Jev, an agent's ask on a cloud provider. Counted since launch (a check reads it before and after a step).
public enum FixtureProviderRequests {
    private static let lock = NSLock()
    nonisolated(unsafe) private static var n = 0
    public static var count: Int { lock.withLock { n } }
    static func note() { lock.withLock { n += 1 } }
}

/// The teaching values of docs/DECISION-MODEL.md v2 §3 in the test engine (hid-ai round 3): the owner's corrections per page since a day, the
/// sender, domain or list they share (nil when they share none), the examples the on-device model may be refitted on, and the refits asked.
/// Set by `fixture corrections <page> <n> <shared|mixed>` and `fixture examples <n>`; read through the state paths `decision_teaching/...`
/// (FixtureCore.state), the paths the real engine is asked to serve (docs/lanes/hid-ai.md, finding F5). Forgotten on `fixture reset`.
public enum FixtureTeaching {
    private static let lock = NSLock()
    nonisolated(unsafe) private static var corrections: [String: (count: Int, shared: String?)] = [:]
    nonisolated(unsafe) private static var examplesN = 0
    nonisolated(unsafe) private static var refitsN = 0
    public static func setCorrections(page: String, count: Int, shared: String?) { lock.withLock { corrections[page] = (count, shared) } }
    public static func setExamples(_ n: Int) { lock.withLock { examplesN = n } }
    public static func refit() { lock.withLock { refitsN += 1 } }
    public static func reset() { lock.withLock { corrections = [:]; examplesN = 0; refitsN = 0 } }
    static func count(_ page: String) -> Int { lock.withLock { corrections[page]?.count ?? 0 } }
    static func shared(_ page: String) -> String { lock.withLock { corrections[page]?.shared ?? "" } }
    static var examples: Int { lock.withLock { examplesN } }
    static var refits: Int { lock.withLock { refitsN } }
    /// The examples a refit needs (DECISION-MODEL.md v2 §3, "after 35 examples"; the real core's `refit_examples()`).
    static let refitAt = 35
}

/// Jev's answer, filled on the URL session's queue and read after the semaphore.
private final class JevAnswer: @unchecked Sendable {
    var status = 0
    var body = Data()
}

extension FixtureCore {
    /// `fixture decide`: the engine, the consent, Jev's endpoint and key are read under the lock; the list runs (and Jev is asked) outside it.
    public func fixtureDecide(page: String, message key: String, entries: [PreviewEntry], label: String, confidence: Double) throws -> FixtureDecision {
        guard let m = FixtureDecideSample.messages[key] else { throw CoreError.Invalid(field: "message", reasonWords: "there is no sample message “\(key)”") }
        let (provider, consent, endpoint, jevKey) = lock.withLock {
            (settings.decision?.provider ?? .local, decisionConsentGiven.contains("jev"), providerEndpoints["jev"], agents.keyChecks["jev"]?.key)
        }
        let engineOn = provider != .rules
        return FixtureDecideSample.decide(m, page: page, entries: entries, engineOn: engineOn) { question in
            switch provider {
            case .jev:
                guard consent, let base = endpoint, let k = jevKey else { return nil }   // no mail reaches Jev before the owner agrees
                return Self.askJev(base: base, key: k, message: m, question: question)
            default:
                return (label, confidence)
            }
        }
    }

    /// `Command.testProviderKey` in the test engine, the same contract as the real core (engine/src/ai_settings.rs `test_provider_key`): Jev only (another
    /// provider is checked when its key is saved); answers at once with `ai_provider/jev/state` "checking"; one fixed sample goes to `<endpoint>/v1/systemone`
    /// with the saved key on its own queue, and the state lands on "connected", or "refused" with words at `ai_provider/jev/refusal`. Called with the lock held.
    func beginJevTestLocked(_ id: String) throws {
        guard id == "jev" else {
            guard let p = agents.provider(id) else { throw CoreError.NotFound(kind: "provider", id: id) }
            throw CoreError.Invalid(field: "provider_id", reasonWords: "\(p.name) is checked when its key is saved: paste the key again to check it.")
        }
        guard let key = agents.keyChecks["jev"]?.key else { throw CoreError.Invalid(field: "key", reasonWords: "Paste the Jev key first.") }
        let endpoint = providerEndpoints["jev"]
        agents.keyChecks["jev"]?.phase = .checking
        agents.keyChecks["jev"]?.verdict = nil
        agents.keyChecks["jev"]?.refusal = nil
        let token = agents.keyChecks["jev"]?.token
        let sample = FixtureDecideMessage(from: "sample@example.com", subject: "Whoosh key test", bodyLength: 0)
        Self.jevTestQueue.async { [weak self] in
            let answer = endpoint.flatMap { Self.askJev(base: $0, key: key, message: sample, question: "Which tab?") }
            self?.lock.withLock {
                guard let self, self.agents.keyChecks["jev"]?.token == token else { return }   // a new save meanwhile: this answer is stale
                if answer != nil { self.agents.keyChecks["jev"]?.phase = .connected }
                else { self.agents.keyChecks["jev"]?.phase = .refused; self.agents.keyChecks["jev"]?.refusal = Self.jevRefusal }
            }
        }
    }
    private static let jevTestQueue = DispatchQueue(label: "whoosh.fixture.jev-test")
    static let jevRefusal = "Jev did not accept that key."

    /// `Command.refitDecisionModel` in the test engine, the real core's gates in order (engine/src/decision_teaching.rs `run`): Jev and Off are refused in
    /// words (field "provider"), fewer examples than the floor in words (field "examples"); then the ask is counted and refused as not built yet
    /// (code "engine.not_built_yet"): no refit happens on either engine. Called with the lock held.
    func refitLocked() throws {
        switch settings.decision?.provider ?? .local {
        case .jev: throw CoreError.Invalid(field: "provider", reasonWords: "Jev is not refitted: make a rule from these corrections instead.")
        case .rules: throw CoreError.Invalid(field: "provider", reasonWords: "The model is off, so there is nothing to refit: choose Local in Settings, Decisions.")
        case .apple: throw CoreError.Invalid(field: "provider", reasonWords: "Apple Intelligence is not refitted: make a rule from these corrections instead.")
        case .local: break
        }
        let have = FixtureTeaching.examples, need = FixtureTeaching.refitAt
        guard have >= need else { throw CoreError.Invalid(field: "examples", reasonWords: "A refit needs \(need) corrections; you have \(have) so far.") }
        FixtureTeaching.refit()
        throw CoreError.Action(failure: ActionFailed(accountName: "", reasonWords: "Whoosh cannot refit the model on this Mac yet: your corrections are kept for when it can.",
                                                     retryable: false, code: "engine.not_built_yet"))
    }

    /// One POST to `<base>/v1/systemone`: the key in the header, the listed fields in the body, the body text cut at DECISION_BODY_CHARS.
    /// A host off this Mac is never asked (`RigNet`); no answer in 4 s is no answer.
    private static func askJev(base: String, key: String, message m: FixtureDecideMessage, question: String) -> (String, Double)? {
        guard let url = URL(string: base + "/v1/systemone"), RigNet.allows(url, env: nil) else { return nil }
        // The real client's body (decision/src/adapters/jev.rs `decide` + wire.rs `request_body`): {state, model, questions}. The state is one text,
        // "from: ..\nto: ..\ncc: ..\nsubject: ..\nlist header: yes|no\n\n<body>" (wire.rs `state_text_with_thread`, no thread here); a choice question is
        // {"type": "choice", "instructions": <prompt>, "criteria": {<option>: <option>}} under its id (wire.rs `question_json`).
        let text = String(repeating: "x", count: min(m.bodyLength, WhooshConstants.DECISION_BODY_CHARS))
        let state = "from: \(m.from)\nto: \ncc: \nsubject: \(m.subject)\nlist header: \(m.listHeader ? "yes" : "no")\n\n\(text)"
        let tabs = ["Primary", "Updates", "Promotions", "Social"]
        let questions: [String: Any] = ["which_tab": ["type": "choice", "instructions": question, "criteria": Dictionary(uniqueKeysWithValues: tabs.map { ($0, $0) })] as [String: Any]]
        let fields: [String: Any] = ["state": state, "model": "jev-latest", "questions": questions]
        guard let data = try? JSONSerialization.data(withJSONObject: fields) else { return nil }
        var req = URLRequest(url: url, timeoutInterval: 3)
        req.httpMethod = "POST"
        req.setValue("Bearer " + key, forHTTPHeaderField: "Authorization")
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = data
        FixtureProviderRequests.note()
        let box = JevAnswer()
        let done = DispatchSemaphore(value: 0)
        URLSession.shared.dataTask(with: req) { data, response, _ in
            box.status = (response as? HTTPURLResponse)?.statusCode ?? 0
            box.body = data ?? Data()
            done.signal()
        }.resume()
        guard done.wait(timeout: .now() + 4) == .success, box.status == 200,
              let json = (try? JSONSerialization.jsonObject(with: box.body)) as? [String: Any],
              // hid-ai round 5: the real answer (decision/src/adapters/wire.rs `read_answers`): {"model": .., "answers": {<question id>: {"choice": .., "confidence": ..}}},
              // exactly the ids asked (here "which_tab"); any other shape is no answer, as the real client refuses it
              let answers = json["answers"] as? [String: Any], Set(answers.keys) == ["which_tab"],
              let a = answers["which_tab"] as? [String: Any], let choice = a["choice"] as? String,
              let conf = (a["confidence"] as? NSNumber)?.doubleValue else { return nil }
        return (choice, conf)
    }
}
