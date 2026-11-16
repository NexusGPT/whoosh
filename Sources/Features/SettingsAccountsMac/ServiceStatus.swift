import Foundation

/// The five mail services the Accounts pane reports on, in the order the pane lists them.
enum MailService: String, CaseIterable, Equatable {
    case gmail, outlook, icloud, fastmail, zoho

    var name: String {
        switch self {
        case .gmail: return Words.text("Gmail")
        case .outlook: return Words.text("Outlook.com")
        case .icloud: return Words.text("iCloud")
        case .fastmail: return Words.text("Fastmail")
        case .zoho: return Words.text("Zoho")
        }
    }

    /// The service an account belongs to: the route the core detected for it (`MailProvider`). Zoho has no route of its own, so a Zoho
    /// address that came in as "other mail" (IMAP) is Zoho by its host (zoho.*, zohomail.*). Any other account belongs to none of the five.
    static func of(provider: MailProvider, address: String) -> MailService? {
        switch provider {
        case .google: return .gmail
        case .microsoft: return .outlook
        case .icloud: return .icloud
        case .fastmail: return .fastmail
        case .imap:
            let host = address.split(separator: "@").last.map { String($0).lowercased() } ?? ""
            let labels = host.split(separator: ".").map(String.init)
            return labels.contains("zoho") || labels.contains("zohomail") ? .zoho : nil
        }
    }
}

/// What this Mac's own accounts of one service report. Nothing is fetched from the service: the state is read from the accounts' own sync state.
enum ServiceState: String, CaseIterable, Equatable {
    case noAccount = "no-account", connected, needsSignIn = "needs-sign-in", paused, unreachable

    var words: String {
        switch self {
        case .noAccount: return Words.text("No account on this Mac")
        case .connected: return Words.text("Connected")
        case .needsSignIn: return Words.text("Needs sign-in")
        case .paused: return Words.text("Paused")
        case .unreachable: return Words.text("Cannot reach the server")
        }
    }

    /// One account's state. A paused account stops fetching, so it reads paused whatever its last sync said. A failure that carries a sync
    /// code (the sync engine could not list, fetch, save or hold its push) is the server being out of reach; any other failure is the sign-in.
    static func of(paused: Bool, sync: SyncState?) -> ServiceState {
        if paused { return .paused }
        guard let e = sync?.lastError else { return .connected }
        return SyncStatusWords.failure(code: e.code, words: e.reasonWords) != nil ? .unreachable : .needsSignIn
    }

    /// The service's state from its accounts' states: none is no account; a sign-in that is needed comes first, then a server out of reach;
    /// all paused is paused; anything else is connected.
    static func combine(_ accounts: [ServiceState]) -> ServiceState {
        if accounts.isEmpty { return .noAccount }
        if accounts.contains(.needsSignIn) { return .needsSignIn }
        if accounts.contains(.unreachable) { return .unreachable }
        if accounts.allSatisfy({ $0 == .paused }) { return .paused }
        return .connected
    }
}

/// One row of the pane's "Mail services" group.
struct ServiceStatusRow: Equatable {
    let service: MailService
    let state: ServiceState
    let accounts: Int

    /// The five rows, always in the order Gmail, Outlook.com, iCloud, Fastmail, Zoho.
    static func rows(accounts: [AccountEntry], states: [String: SyncState]) -> [ServiceStatusRow] {
        MailService.allCases.map { service in
            let mine = accounts.filter { $0.service == service }
            return ServiceStatusRow(service: service, state: .combine(mine.map { ServiceState.of(paused: $0.paused, sync: states[$0.id]) }), accounts: mine.count)
        }
    }
}
