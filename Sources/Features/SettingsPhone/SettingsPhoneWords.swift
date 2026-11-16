import Foundation

/// Every line the iPhone Settings says, through the app's catalogue (`Words.text`: the key is the English text, 14 languages).
/// A name already picked is reused (SettingsPane, AISetupWords, AccountsWords); a sentence that is new is marked `new` and
/// listed in docs/lanes/settingsphone.md with its English text. A title is a noun; a setting is one plain short line.
@MainActor
enum SettingsPhoneWords {
    static func t(_ english: String) -> String { Words.text(english) }

    // names that exist
    static var title: String { t("Settings") }
    static var done: String { t("Done") }
    static var back: String { t("Back") }
    static var mail: String { t("Mail") }
    static var sync: String { t("Sync") }
    static var ai: String { t("AI") }
    static var general: String { t("General") }
    static var accounts: String { t("Accounts") }
    static var language: String { t("Language") }
    static var appearance: String { t("Appearance") }
    static var system: String { t("System") }
    static var retry: String { t("Retry") }
    static var ready: String { t("Ready") }
    static var writingModel: String { t("Writing model") }
    static var decisionModel: String { t("Decision model") }
    static var notSyncedYet: String { t("Not synced yet") }
    static var syncOwn: String { t("Latest change wins. Nothing is sent to a Whoosh server.") }
    static var jev: String { t("Jev") }
    static var rules: String { t("Rules") }
    static func theme(_ id: String) -> String {
        switch id { case "light": return t("Light"); case "dark": return t("Dark"); default: return t("Auto") }
    }
    static func accountsLead(count: Int, failing: Int) -> String { AccountsWords.lead(count: count, failing: failing) }

    // new sentences (the English text is the key)
    static func inSync(_ mac: String) -> String { Words.text("In sync with %@", mac) }
    static func notInSync(_ mac: String) -> String { Words.text("Not in sync with %@", mac) }
    static func lastSync(_ when: String) -> String { Words.text("Last sync %@", when) }
    static var noMac: String { t("No Mac is paired yet") }
    static var syncLine: String { t("Your settings travel between your devices, encrypted, on your own iCloud.") }
    static var syncFailed: String { t("The last sync did not finish.") }
    static var accountsFooter: String { t("Accounts come from your Mac.") }
    static var laya: String { t("Laya") }

    /// A moment as words in the language the app speaks now ("2 min ago", "il y a 2 min"): the system's formatter, no key to translate.
    static func ago(_ seconds: Int) -> String {
        let f = RelativeDateTimeFormatter()
        f.locale = Locale(identifier: Words.current)
        f.unitsStyle = .short
        f.dateTimeStyle = .named
        return f.localizedString(fromTimeInterval: -Double(max(seconds, 0)))
    }
}
