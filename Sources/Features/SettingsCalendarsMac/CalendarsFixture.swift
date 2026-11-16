import Foundation

/// The fixture doors, as the notifications the shell posts for the verbs it serves (`whoosh.fixture.<verb>`, userInfo `args`;
/// Sources/ControlSocket/FixtureEvents.swift): `fixture calendar-access <word>` and `fixture reset`.
@MainActor
enum CalendarsFixture {
    static let access = Notification.Name("whoosh.fixture.calendar-access")
    static let reset = Notification.Name("whoosh.fixture.reset")
    private static var tokens: [NSObjectProtocol] = []

    static func listen(_ m: CalendarsModel) {
        guard tokens.isEmpty else { return }
        let c = NotificationCenter.default
        // Each closure takes its Sendable values out of the notification first and never passes the notification on.
        tokens = [
            c.addObserver(forName: access, object: nil, queue: .main) { n in
                let word = ((n.userInfo?["args"] as? [String]) ?? []).first ?? ""
                MainActor.assumeIsolated { _ = m.fixtureSet(word) }
            },
            c.addObserver(forName: reset, object: nil, queue: .main) { _ in
                MainActor.assumeIsolated { m.resetFromFixture() }
            },
        ]
    }
}
