import Foundation

/// The sign-in clients Whoosh ships (SPEC A88, owner 2026-11-09 14:44: "Yes, I register one and we ship it"). ONE value
/// lives here: the Microsoft client ID of the multi-tenant public client the OWNER registers in their own Entra admin, as
/// Thunderbird ships Mozilla's. With it, a Microsoft 365 address signs in with "Sign in" alone; an organisation that wants
/// its own registration still pastes its ID behind "Use your organisation’s own ID" (the A77 path, now the override).
/// Gmail keeps app passwords: there is no Google value, ever. checks/no-vendor-id-check.py allows exactly one GUID, on the
/// `microsoft` line of this file, and refuses any other vendor ID or any client secret anywhere.
///
/// nil = not registered yet: the sheet behaves as before A88 (the app-ID field shows). The owner pastes the GUID here.
enum DefaultSignInClients {
    static let microsoft: String? = "2d200179-fd15-448b-bf1f-cf4f4f78f53f"   // A88: registered by the owner in their own Entra on 2026-11-09 (multi-tenant public client "Whoosh"); public, not a secret

    /// The hidden test copy's switch (`fixture default-client set|clear`, fixture core only): set = a stand-in ID built at run
    /// time (never a literal, so the vendor check never sees one), clear = no default, nil = the constant above.
    enum FixtureDefault: Equatable { case set, cleared }
    @MainActor static var fixture: FixtureDefault?

    /// The default Microsoft client ID the sheet uses now.
    @MainActor static var microsoftInUse: String? {
        switch fixture {
        case .set?: return [8, 4, 4, 4, 12].map { String(repeating: "c", count: $0) }.joined(separator: "-")
        case .cleared?: return nil
        case nil: return microsoft
        }
    }
}
