import Foundation

/// Every fixed word of the guide. The step words live with their steps (EntraGuideData); the shared ones (Close, Back, Next, Copy,
/// Copied) are the app's own catalogue keys. Computed, so the language picker applies at once.
enum EntraGuideWords {
    static let surface = "mac-entra-guide"
    static let entraURL = "https://entra.microsoft.com"
    static let entraHost = "entra.microsoft.com"
    static var title: String { Words.text("entra-guide.title") }
    static func stepOf(_ n: Int, _ of: Int) -> String { Words.text("Step %@ of %@", n, of) }
    static var close: String { Words.text("Close") }
    static var back: String { Words.text("Back") }
    static var next: String { Words.text("Next") }
    static var copy: String { Words.text("Copy") }
    static var copied: String { Words.text("Copied") }
    static var skip: String { Words.text("entra-guide.skip") }
    static var openEntra: String { Words.text("entra-guide.open-entra") }
    static var opened: String { Words.text("entra-guide.opened") }
    static var save: String { Words.text("entra-guide.save") }
    static var pasteLabel: String { Words.text("entra-guide.paste-label") }
    static var pastePlaceholder: String { Words.text("entra-guide.paste-placeholder") }
    static var slotLabel: String { Words.text("entra-guide.slot-label") }
    static var slotPlaceholder: String { Words.text("entra-guide.slot-placeholder") }
    static var idWrong: String { Words.text("entra-guide.id-wrong") }
    static var idOk: String { Words.text("entra-guide.id-ok") }
    static var keychain: String { Words.text("entra-guide.keychain") }
    static var pickTag: String { Words.text("entra-guide.pick-tag") }
    static var drawn: String { Words.text("entra-guide.drawn") }
    /// A98: the mailbox's domain as the guide says it, or the plain words when no address was in the sheet
    static func consentDomain(_ domain: String) -> String { domain.isEmpty ? Words.text("entra-guide.consent.nodomain") : domain }
    static func plate(_ cap: String) -> String { Words.text("Screenshot of Microsoft Entra: %@", cap) }
    static func tooltip(_ title: String, _ key: String) -> String { Words.text("%@ (%@)", title, key) }
    static func key(_ id: String) -> String {
        switch id { case "close": return "esc"; case "skip": return "⌘↓"; case "back": return "⌘["; case "next", "save": return "↩"; default: return "" }
    }
}
