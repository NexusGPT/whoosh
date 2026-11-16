import Foundation

/// How an address is added: the route, in the owner's words. Pure data, no view and no clock.
/// Detection reads the domain only; a host not listed here is "IMAP" and Whoosh asks the host for its settings.
public enum WelcomeRouteKind: String, Sendable { case google, microsoft, icloud, fastmail, yahoo, aol, zoho, spacemail, proton, imap }

public struct WelcomeRoute: Equatable, Sendable {
    public let kind: WelcomeRouteKind
    /// Matches the contract's provider names where one exists: Google, Microsoft, iCloud, Fastmail, IMAP.
    public let name: String
    /// The sign-in path offered, in words.
    public let how: String
    public let steps: [String]
    public let link: String?
    public let linkWords: String?
    public let server: String
    public let usesSheet: Bool
    public let macOnly: Bool

    /// Wrong password, or 2-step sign-in is off: the line every app-password route shows when the host refuses.
    public func refusedWords(_ serverWords: String? = nil) -> String {
        serverWords ?? Words.text("wrong password, or 2-step sign-in is off at %@", name)
    }
}

public enum WelcomeRoutes {
    private static func domain(_ address: String) -> String {
        address.split(separator: "@", omittingEmptySubsequences: false).last.map { String($0).lowercased() } ?? ""
    }

    public static func detect(_ address: String) -> WelcomeRoute {
        let d = domain(address)
        if ["gmail.com", "googlemail.com"].contains(d) { return google }
        if ["outlook.com", "hotmail.com", "live.com", "office365.com", "msn.com"].contains(d) || d.hasSuffix(".onmicrosoft.com") { return microsoft }
        if ["icloud.com", "me.com", "mac.com"].contains(d) { return icloud }
        if ["fastmail.com", "fastmail.fm", "messagingengine.com"].contains(d) { return fastmail }
        if ["yahoo.com", "ymail.com", "rocketmail.com"].contains(d) || d.hasPrefix("yahoo.") { return yahoo }
        if ["aol.com", "aim.com"].contains(d) { return aol }
        if ["proton.me", "protonmail.com", "pm.me", "protonmail.ch"].contains(d) { return proton }
        if d == "spacemail.com" { return spacemail }
        if let region = zohoRegion(d) { return zoho(region) }
        return other(d)
    }

    /// Zoho keeps one mail host per data centre: zoho.com, .eu, .in, .com.au, .jp, .sa, .ca.
    static func zohoRegion(_ d: String) -> String? {
        for base in ["zoho", "zohomail"] where d.hasPrefix(base + ".") { return String(d.dropFirst(base.count + 1)) }
        return nil
    }

    static let google = WelcomeRoute(kind: .google, name: "Google", how: "Make a Google app password and paste it here.",
        steps: ["Turn on 2-step sign-in in your Google account.", "Make an app password named Whoosh.", "Paste the 16 letters here."],
        link: "https://myaccount.google.com/apppasswords", linkWords: "Open Google app passwords", server: "imap.gmail.com", usesSheet: false, macOnly: false)
    static let microsoft = WelcomeRoute(kind: .microsoft, name: "Microsoft", how: "Sign in with Microsoft in a sheet. You never type a password here.",
        steps: ["Press Sign in with Microsoft.", "Say yes to the access Whoosh asks for.", "The sheet closes and the account starts to load."],
        link: nil, linkWords: nil, server: "outlook.office365.com", usesSheet: true, macOnly: false)
    static let icloud = WelcomeRoute(kind: .icloud, name: "iCloud", how: "Make an app-specific password at Apple and paste it here.",
        steps: ["Sign in at Apple and open Sign-In and Security.", "Make an app-specific password named Whoosh.", "Paste it here."],
        link: "https://account.apple.com/account/manage", linkWords: "Open your Apple account", server: "imap.mail.me.com", usesSheet: false, macOnly: false)
    static let fastmail = WelcomeRoute(kind: .fastmail, name: "Fastmail", how: "Sign in with Fastmail in a sheet.",
        steps: ["Press Sign in with Fastmail.", "Say yes to the access Whoosh asks for.", "Whoosh uses JMAP, Fastmail's own fast protocol."],
        link: nil, linkWords: nil, server: "api.fastmail.com", usesSheet: true, macOnly: false)
    static let yahoo = WelcomeRoute(kind: .yahoo, name: "Yahoo", how: "Make a Yahoo app password and paste it here.",
        steps: ["Open Account security at Yahoo.", "Make an app password named Whoosh.", "Paste it here."],
        link: "https://login.yahoo.com/account/security", linkWords: "Open Yahoo account security", server: "imap.mail.yahoo.com", usesSheet: false, macOnly: false)
    static let aol = WelcomeRoute(kind: .aol, name: "AOL", how: "Make an AOL app password and paste it here.",
        steps: ["Open Account security at AOL.", "Make an app password named Whoosh.", "Paste it here."],
        link: "https://login.aol.com/account/security", linkWords: "Open AOL account security", server: "imap.aol.com", usesSheet: false, macOnly: false)
    static let proton = WelcomeRoute(kind: .proton, name: "Proton Mail", how: "Proton Mail reaches Whoosh through Proton Mail Bridge, on this Mac only.",
        steps: ["Install Proton Mail Bridge and sign in to it.", "Copy the Bridge password for this account.", "Paste it here. Bridge must stay open."],
        link: "https://proton.me/mail/bridge", linkWords: "Open Proton Mail Bridge", server: "127.0.0.1 (Bridge, this Mac)", usesSheet: false, macOnly: true)
    static let spacemail = WelcomeRoute(kind: .spacemail, name: "Spacemail", how: "Spacemail is a preset: type your mailbox password.",
        steps: ["Type the password of this mailbox.", "Whoosh fills in the Spacemail servers."],
        link: nil, linkWords: nil, server: "mail.spacemail.com", usesSheet: false, macOnly: false)

    static func zoho(_ region: String) -> WelcomeRoute {
        WelcomeRoute(kind: .zoho, name: "Zoho", how: "Make a Zoho app password and paste it here. Your data centre is \(region).",
            steps: ["Open Security in your Zoho account.", "Make an app password named Whoosh.", "Paste it here."],
            link: "https://accounts.zoho.\(region)/home#security/app_password", linkWords: "Open Zoho app passwords",
            server: "imap.zoho.\(region)", usesSheet: false, macOnly: false)
    }
    static func other(_ d: String) -> WelcomeRoute {
        WelcomeRoute(kind: .imap, name: "IMAP", how: "Whoosh asks \(d.isEmpty ? "the host" : d) for its settings, then you paste the password.",
            steps: ["Whoosh looks for JMAP, then for IMAP settings.", "If the host gives none, you type the server.", "Paste the password or app password."],
            link: nil, linkWords: nil, server: d.isEmpty ? "found from the address" : "imap.\(d), found from the address", usesSheet: false, macOnly: false)
    }

    public static func isValid(_ address: String) -> Bool {
        guard address.count <= 254, !address.contains(where: \.isWhitespace) else { return false }
        let parts = address.split(separator: "@", omittingEmptySubsequences: false)
        guard parts.count == 2, !parts[0].isEmpty, parts[0].count <= 64, let dot = parts[1].lastIndex(of: ".") else { return false }
        return dot != parts[1].startIndex && parts[1].index(after: dot) != parts[1].endIndex
    }
}
