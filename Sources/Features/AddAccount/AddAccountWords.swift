import Foundation

/// Every word the Add account sheet shows, in one place. Plain, short, no jargon; each string goes through Words.text with its English
/// key. Computed, not stored: the language picker applies at once, so a word is read when it is drawn.
enum AddAccountWords {
    static let surface = "mac-add-account"

    // the head
    static var title: String { Words.text("Add account") }
    static var leadEntry: String { Words.text("An address and a password. Whoosh finds the servers.") }
    static var leadFound: String { Words.text("Found. Nothing has been sent yet.") }
    static var leadManual: String { Words.text("Type the server settings. Nothing has been sent yet.") }
    static func leadChecking(_ provider: String) -> String { Words.text("Checking with %@. Nothing is saved yet.", provider) }
    static func leadRefused(_ provider: String) -> String { Words.text("%@ refused the password. Nothing was saved.", provider) }
    static var leadServer: String { Words.text("the server") }
    static var added: String { Words.text("Account added") }
    static var welcomeTitle: String { Words.text("Add your mail") }
    static var welcomeLead: String { Words.text("One address at a time. Whoosh finds the servers.") }

    // the two fields
    static var email: String { Words.text("Email") }
    static var password: String { Words.text("Password") }
    static var addressPlaceholder: String { Words.text("name@example.com") }
    static var passwordPlaceholder: String { Words.text("Password or app password") }
    static var showPassword: String { Words.text("Show password") }
    static var hidePassword: String { Words.text("Hide password") }
    static var keychain: String { Words.text("The password stays in this Mac’s Keychain.") }

    // bring your own sign-in (PRD 2026-11-09 "as easy as Thunderbird"): Whoosh ships no vendor ID; the person pastes their own once
    static var appIdCap: String { Words.text("Your organisation’s app ID") }
    static var appIdHint: String { Words.text("Paste it once. It stays in this Mac’s Keychain.") }
    static var appIdSaved: String { Words.text("App ID saved in the Keychain") }
    static var change: String { Words.text("Change") }
    static var signIn: String { Words.text("Sign in") }
    static var microsoftNoPassword: String { Words.text("Microsoft turned off password sign-in for IMAP in 2022, so a password cannot work here. Paste your organisation’s app ID to sign in.") }
    static var guideLink: String { Words.text("How to get your app ID") }
    /// SPEC A87: the link beside the app-ID field
    static var guideLinkShort: String { Words.text("How to get one") }
    /// SPEC A88: over Whoosh's default Microsoft registration, the way to an organisation's own app ID
    static var useOwnId: String { Words.text("Use your organisation’s own ID") }

    // SPEC A87 (owner 14:38 "what does that mean btw ?"): one plain sentence per known refusal cause; the server's own text sits behind Details
    static var detailsShow: String { Words.text("Details") }
    static var detailsHide: String { Words.text("Hide details") }
    /// Total over the contract's causes: a new cause is a build error, never a silent default.
    static func causeSentence(_ c: RefusalCause) -> String {
        switch c {
        case .basicAuthDisabled: return Words.text("Microsoft no longer accepts passwords for mail. Sign in with your organisation’s app ID below.")
        case .badPassword: return Words.text("The password was refused.")
        case .hostUnreachable: return Words.text("The server did not answer.")
        case .tlsFailed: return Words.text("The secure connection failed.")
        case .smtpRefused: return Words.text("The sending server refused the sign-in.")
        case .unknown: return Words.text("The server refused the account. Details shows its own words.")
        }
    }
    /// The cause ids the socket rows and the checks read (the contract's `refusal_cause_id`).
    static func causeId(_ c: RefusalCause) -> String { refusalCauseId(cause: c) }
    /// the guide's bundled copy (Sources/Resources/guides), the same words as docs/guides/bring-your-own-sign-in.md
    static let guideFile = "bring-your-own-sign-in.md"
    static var waitingBrowser: String { Words.text("Waiting for you in the browser…") }
    static var signedIn: String { Words.text("Signed in. Adding the account…") }
    static var needAppId: String { Words.text("Paste your app ID first.") }
    static var signInRefused: String { Words.text("Sign-in refused") }
    static var useAppId: String { Words.text("Sign in with your own app ID below.") }
    /// SEAM(delta 14): the real core has no sign-in door yet; this is what the sheet says until SetOAuthClient / StartOAuthSignIn land.
    static var signInNotYet: String { Words.text("Sign in with your own app ID needs the next core update.") }

    // the step line
    static func stepOf(_ n: Int, _ of: Int) -> String { Words.text("%@ of %@", n, of) }
    static var checkingServer: String { Words.text("Checking the mail server…") }
    static var checkingSending: String { Words.text("Checking sending…") }
    /// The detection steps in the order they run. SEAM(delta 14): core-autoconfig adds the domain's mail record (MX) after the
    /// domain's own file; flip `mxStepLanded` when `DetectionStep` gains it and `stepId` names it.
    static let mxStepLanded = true
    static var stepOrder: [String] {
        mxStepLanded ? ["ispdb", "autoconfig", "srv", "autodiscover", "mx", "guess"] : ["ispdb", "autoconfig", "srv", "autodiscover", "guess"]
    }
    static func stepWords(_ id: String) -> String {
        switch id {
        case "mx": return Words.text("Reading the domain’s mail record…")
        case "ispdb": return Words.text("Asking the Thunderbird provider list…")
        case "autoconfig": return Words.text("Looking for the provider’s own settings file…")
        case "srv": return Words.text("Looking up the mail server in DNS…")
        case "autodiscover": return Words.text("Asking Microsoft Autodiscover…")
        default: return Words.text("Trying imap., mail. and smtp. names on the usual ports…")
        }
    }
    /// What a step that found nothing is called in "Nothing in …".
    static func missed(_ id: String) -> String {
        switch id {
        case "mx": return Words.text("the domain’s mail record")
        case "ispdb": return Words.text("the Thunderbird list")
        case "autoconfig": return Words.text("the domain’s own settings file")
        case "srv": return Words.text("DNS")
        case "autodiscover": return Words.text("Microsoft Autodiscover")
        default: return Words.text("the usual names")
        }
    }
    static func nothingIn(_ places: [String]) -> String {
        guard let last = places.last else { return "" }
        let head = places.dropLast().joined(separator: ", ")
        return Words.text("Nothing in %@.", head.isEmpty ? last : Words.text("%@ or %@", head, last))
    }
    /// The found line's last words: which step found the settings.
    static func foundBy(_ source: DetectionSource) -> String {
        switch source {
        case .ispdb: return Words.text("Thunderbird list")
        case .autoconfig: return Words.text("the domain’s settings file")
        case .srv: return Words.text("DNS")
        case .autodiscover: return Words.text("Microsoft Autodiscover")
        case .guess: return Words.text("the usual names")
        case .mailRecord: return Words.text("the domain’s mail record")
        case .manual: return Words.text("your settings")
        }
    }
    static func source(_ s: DetectionSource) -> String { foundBy(s) }
    static func imapSteps(_ host: String, _ port: String) -> String { Words.text("%@:%@ · IMAP LOGIN, then LIST", host, port) }
    static var details: String { Words.text("Details") }
    static func noSettings(_ domain: String) -> String {
        Words.text("No settings found for %@. The Thunderbird list, the domain’s file, DNS, Autodiscover and the usual names all missed.", domain)
    }
    static var typedSettingsNote: String { Words.text("Type the incoming and the sending server below.") }

    // the footer
    static var manual: String { Words.text("Manual setup") }
    static var hideManual: String { Words.text("Hide manual setup") }
    static var skipManual: String { Words.text("Skip to manual setup") }
    static var serverSettings: String { Words.text("Server settings") }
    static var cancel: String { Words.text("Cancel") }
    static var cont: String { Words.text("Continue") }
    static var finding: String { Words.text("Finding…") }
    static var add: String { Words.text("Add") }
    static var adding: String { Words.text("Adding…") }
    static var retry: String { Words.text("Try again") }
    static var addAnother: String { Words.text("Add another") }
    static var done: String { Words.text("Done") }
    static var stopQuestion: String { Words.text("Stop checking? Nothing is saved yet.") }
    static var keepChecking: String { Words.text("Keep checking") }
    static var stop: String { Words.text("Stop") }
    static func tooltip(_ title: String, _ key: String) -> String { Words.text("%@ (%@)", title, key) }

    // the refusal
    static func refusedBy(_ host: String) -> String { Words.text("%@ refused the password", host) }
    static func didNotAnswer(_ host: String) -> String { Words.text("%@ did not answer", host) }
    static var signInFailed: String { Words.text("The sign-in failed") }
    static func consentRequired(_ domain: String) -> String { Words.text("The admin of %@ must approve Whoosh once. Then you can sign in.", domain) }
    static func consentLinkLine(_ link: String) -> String { Words.text("Approval link: %@", link) }
    static var copy: String { Words.text("Copy") }
    static var consentPathPage: String { Words.text("add-account.engine.oauth_consent_path_page") }
    static var consentPathEntra: String { Words.text("add-account.engine.oauth_consent_path_entra") }
    static func appPasswordNeeded(_ provider: String) -> String { Words.text("%@ wants an app password, not your account password.", provider) }
    static func appPasswordNote(_ provider: String) -> String { Words.text("%@ wants an app password for mail apps.", provider) }
    static var tenant: String { Words.text("Microsoft 365 organisation") }
    static var badAddress: String { Words.text("That is not an address yet. It needs a name, an @ and a domain such as example.com.") }
    static var incomingMissing: String { Words.text("The incoming server needs a name, a port and a security setting.") }
    static var outgoingMissing: String { Words.text("The outgoing server needs a name, a port and a security setting.") }
    static var needPassword: String { Words.text("Enter the password for this address.") }

    // manual setup
    static var incomingCap: String { Words.text("Incoming · IMAP") }
    static var outgoingCap: String { Words.text("Sending · SMTP") }
    static var username: String { Words.text("Username") }
    static var signInWithCap: String { Words.text("Sign in with") }
    static var port: String { Words.text("Port") }
    static var server: String { Words.text("Server") }
    static func security(_ id: String) -> String {
        switch id { case "ssl": return Words.text("SSL"); case "starttls": return Words.text("STARTTLS"); default: return "" }
    }
    static func auth(_ id: String) -> String {
        switch id { case "password": return Words.text("Password"); case "oauth2": return Words.text("OAuth2"); default: return "" }
    }

    // the added state
    static var connectingWords: String { Words.text("Connecting…") }
    static var fetchingWords: String { Words.text("Fetching the inbox…") }
    static var readyWords: String { Words.text("Inbox ready") }
    static var thenFetching: String { Words.text("then fetching the inbox · keeps going after Done") }
    static var sessionCap: String { Words.text("Added in this session") }
    static var addedCap: String { Words.text("Added") }

    // the row ids the socket reads (not shown)
    static let connecting = "connecting"
    static let fetching = "fetching the inbox"
    static let ready = "ready"
}
