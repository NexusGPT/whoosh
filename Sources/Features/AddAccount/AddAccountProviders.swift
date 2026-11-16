import Foundation

/// What the owner needs to know about a provider that offers an app password or an OAuth2 sign-in (docs/PRD.md
/// "Add account by IMAP"). Read from the detected server and the address's domain; never contacts anything.
struct AddAccountProvider: Equatable {
    let name: String
    /// the OAuth2 sign-in this provider offers; nil when it offers none
    let oauthName: String?
    let steps: [String]
    let link: String
    let linkWords: String

    static func of(host: String, address: String) -> AddAccountProvider? {
        let h = host.lowercased(), d = address.split(separator: "@").last.map { String($0).lowercased() } ?? ""
        func has(_ parts: String...) -> Bool { parts.contains { h.contains($0) || d == $0 } }
        if has("gmail.com", "googlemail.com", "imap.gmail") {
            return AddAccountProvider(name: "Google", oauthName: "Google", steps: [
                Words.text("Turn on 2-Step Verification in your Google account."),
                Words.text("Make an app password for Mail."),
                Words.text("Paste it above as the password.")], link: "https://myaccount.google.com/apppasswords", linkWords: Words.text("Open Google app passwords"))
        }
        if has("yahoo.", "ymail.com") {
            return AddAccountProvider(name: "Yahoo", oauthName: "Yahoo", steps: [
                Words.text("Open Yahoo account security and make an app password."),
                Words.text("Paste it above as the password.")], link: "https://login.yahoo.com/account/security", linkWords: Words.text("Open Yahoo account security"))
        }
        if has("icloud.com", "me.com", "mac.com", "mail.me.com") {
            return AddAccountProvider(name: "iCloud", oauthName: nil, steps: [
                Words.text("Sign in at Apple and make an app-specific password."),
                Words.text("Paste it above as the password.")], link: "https://account.apple.com/account/manage", linkWords: Words.text("Open the Apple ID page"))
        }
        if has("fastmail.") {
            return AddAccountProvider(name: "Fastmail", oauthName: nil, steps: [
                Words.text("In Fastmail, open Privacy & Security, then Integrations, and make an app password for Mail."),
                Words.text("Paste it above as the password.")], link: "https://app.fastmail.com/settings/security/integrations", linkWords: Words.text("Open Fastmail app passwords"))
        }
        if has("office365", "outlook.", "hotmail.", "live.com") {
            return AddAccountProvider(name: "Microsoft", oauthName: "Microsoft", steps: [], link: "", linkWords: "")
        }
        return nil
    }
}
