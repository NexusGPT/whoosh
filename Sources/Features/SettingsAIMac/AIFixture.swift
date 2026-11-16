import Foundation

/// The fixture doors J5 drives, as the notifications the shell posts for the verbs it serves (`whoosh.fixture.<verb>`, userInfo
/// `args` and `text`; Sources/ControlSocket/FixtureEvents.swift): `fixture paste <name>`, `fixture speech-fail [words]`,
/// `fixture reset`, and `fixture provider-endpoint <provider> <url>` (provider `laya`, `model:<line id>`, `model-hub` or a provider id; this verb is NOT in FixtureEvents.served at this commit:
/// the lead adds `"provider-endpoint": 2...2,` there, see docs/lanes/settingsaimac-r3.md). The click targets
/// `mac-settings-ai.fixture.*` drive the same doors.
@MainActor
enum AIFixture {
    static let paste = Notification.Name("whoosh.fixture.paste")
    static let speechFail = Notification.Name("whoosh.fixture.speech-fail")
    static let reset = Notification.Name("whoosh.fixture.reset")
    static let providerEndpoint = Notification.Name("whoosh.fixture.provider-endpoint")
    /// hid-ai: `fixture decide <page> <message> <label> <confidence>`, `fixture add-rule <page> <words> <outcome>`, `fixture consent jev yes|no`
    /// (served in FixtureEvents.swift). Taken on the posting thread (FixtureEvents is on the main actor), so the page holds the answer before the verb replies.
    static let decide = Notification.Name("whoosh.fixture.decide")
    static let addRule = Notification.Name("whoosh.fixture.add-rule")
    static let consent = Notification.Name("whoosh.fixture.consent")
    static let corrections = Notification.Name("whoosh.fixture.corrections")
    static let examples = Notification.Name("whoosh.fixture.examples")
    static let chatgpt = Notification.Name("whoosh.fixture.chatgpt")

    /// The journey fixture's named keys (docs/JOURNEYS.md J5.1); the key text never reaches a log or the disk.
    static let anthropicKey = "sk-test-anthropic-0001"
    static func key(named name: String) -> String? {
        switch name {
        case "anthropic-key": anthropicKey
        case "bad-key": "sk-bad"
        default: nil
        }
    }

    /// The next speech download fails, in the words given (the core's default words when none).
    static func failSpeech(_ words: String = "") {
        _ = CoreHost.shared.fixture?.failNext("speech-download", reason: words)
    }

    private static var tokens: [NSObjectProtocol] = []

    static func listen(_ s: AISettingsStore) {
        guard tokens.isEmpty else { return }
        let c = NotificationCenter.default
        tokens = [
            c.addObserver(forName: paste, object: nil, queue: .main) { n in
                let name = ((n.userInfo?["args"] as? [String]) ?? []).first ?? ""
                MainActor.assumeIsolated { if let k = key(named: name) { s.paste(k, into: "anthropic") } }
            },
            c.addObserver(forName: speechFail, object: nil, queue: .main) { n in
                let words = ((n.userInfo?["args"] as? [String]) ?? []).joined(separator: " ")
                MainActor.assumeIsolated { failSpeech(words) }
            },
            c.addObserver(forName: reset, object: nil, queue: .main) { _ in MainActor.assumeIsolated { s.resetFromFixture() } },
            c.addObserver(forName: decide, object: nil, queue: nil) { n in
                let a = ((n.userInfo?["args"] as? [String]) ?? [])
                guard a.count == 4 else { return }
                MainActor.assumeIsolated { s.decideFromFixture(page: a[0], message: a[1], label: a[2], confidence: Double(a[3]) ?? 0) }
            },
            c.addObserver(forName: addRule, object: nil, queue: nil) { n in
                let a = ((n.userInfo?["args"] as? [String]) ?? [])
                guard a.count == 3 else { return }
                MainActor.assumeIsolated { s.addRule(page: a[0], words: a[1], outcome: a[2]) }
            },
            c.addObserver(forName: consent, object: nil, queue: nil) { n in
                let a = ((n.userInfo?["args"] as? [String]) ?? [])
                guard a.count == 2, a[0] == "jev" else { return }
                MainActor.assumeIsolated { s.settleAll(); s.setConsent(a[1] == "yes") }
            },
            c.addObserver(forName: corrections, object: nil, queue: nil) { n in
                let a = ((n.userInfo?["args"] as? [String]) ?? [])
                guard a.count == 3, let k = Int(a[1]) else { return }
                FixtureTeaching.setCorrections(page: a[0], count: k, shared: a[2] == "mixed" ? nil : a[2])
            },
            c.addObserver(forName: chatgpt, object: nil, queue: nil) { n in
                let a = ((n.userInfo?["args"] as? [String]) ?? [])
                guard a.count == 2 else { return }
                MainActor.assumeIsolated { _ = SignInController.shared.rigChatGPT(approve: a[0] == "approve", base: a[1]) }
            },
            c.addObserver(forName: examples, object: nil, queue: nil) { n in
                guard let k = ((n.userInfo?["args"] as? [String]) ?? []).first.flatMap({ Int($0) }) else { return }
                FixtureTeaching.setExamples(k)
            },
            c.addObserver(forName: providerEndpoint, object: nil, queue: .main) { n in
                let a = ((n.userInfo?["args"] as? [String]) ?? [])
                guard a.count == 2 else { return }
                MainActor.assumeIsolated {
                    if a[0] == "laya" { LayaOnDevice.shared.useRigBase(a[1]); s.layaStateChanged() }   // the weights' download server of a check (rig only)
                    else if a[0] == "model-hub" { ModelHub.useRigBase(a[1]) }   // the hub the line "Other model" asks: a loopback server of the check (rig only)
                    else if a[0].hasPrefix("model:") { DecisionModels.shared.useRigBase(id: String(a[0].dropFirst(6)), a[1]); s.layaStateChanged() }   // the same for a catalogue line
                    else { CoreHost.shared.fixture?.setProviderEndpoint(a[0], url: a[1]) }
                }
            },
        ]
    }
}
