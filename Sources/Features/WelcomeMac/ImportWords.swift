import Foundation

/// Every string of the import step goes through `Words.text`; the key is the English text (Sources/Resources/Localizable.xcstrings, 14 languages).
/// The refusal sentences are the contract's own words, verbatim (docs/lanes/contracts.md "Delta 18", core `import_access_words` and the
/// `check_import_*` functions): one sentence, one key, so a refusal never reaches the screen in two languages.
enum ImportWords {
    static var title: String { Words.text("Import the mail you already have") }
    static var subtitle: String { Words.text("Bring older mail from this Mac into Whoosh. Nothing leaves this Mac.") }
    static var nothingFound: String { Words.text("Nothing found on this Mac. Choose a file instead.") }
    static var scanning: String { Words.text("Looking for mail on this Mac…") }
    static var appleMail: String { Words.text("Apple Mail") }
    static var outlookKind: String { Words.text("Outlook archive") }
    static var mboxKind: String { Words.text("Mailbox file") }
    static var choose: String { Words.text("Choose…") }
    static var chooseHint: String { Words.text("Outlook (.olm) or Thunderbird (.mbox)") }
    static var chooseMessage: String { Words.text("Choose a file to import") }
    static var countUnknown: String { Words.text("Message count not known") }
    static var into: String { Words.text("Into") }
    static var matching: String { Words.text("The matching account, or Imported") }
    static var start: String { Words.text("Start import") }
    static var cancel: String { Words.text("Cancel") }
    static var skip: String { Words.text("Skip to the inbox") }
    static var goInbox: String { Words.text("Go to the inbox") }
    static var close: String { Words.text("Close") }
    static var openFullDiskAccess: String { Words.text("Open Full Disk Access") }
    static var cancelled: String { Words.text("Import cancelled. What was stored stays.") }
    static var settingsOffer: String { Words.text("Import mail…") }

    static func count(_ n: Int) -> String { Words.text("%lld messages", n) }
    static func alreadyImported(_ n: Int) -> String { Words.text("%lld already imported", n) }
    static func progress(done: Int, total: Int?, current: String) -> String {
        total.map { Words.text("%lld of %lld · %@", done, $0, current) } ?? Words.text("%lld read · %@", done, current)
    }
    static func finished(new: Int, already: Int, left: Int) -> String {
        Words.text("Done. %lld new, %lld already here, %lld left out.", new, already, left)
    }
    static func kind(_ k: String) -> String { k == "apple-mail" ? appleMail : (k == "olm" ? outlookKind : mboxKind) }

    /// A refusal's contract sentence in the app's language, by its code; any other code keeps the core's own words.
    static func refusal(code: String, words: String) -> String {
        switch code {
        case "import_needs_full_disk_access":
            return Words.text("Whoosh needs Full Disk Access to read Apple Mail. Open System Settings, choose Privacy & Security, then Full Disk Access, add Whoosh to the list and switch it on. You can also skip this step and import later from Settings, Accounts.")
        case "import_source_not_found": return Words.text("That file or folder is not there any more. Pick it again.")
        case "import_source_unreadable": return Words.text("That file or folder could not be read. Check that it is a mail archive.")
        case "import_unknown_source": return Words.text("That source is not in the list. Scan again.")
        case "import_bad_target": return Words.text("That account is not on this Mac. Pick an account from the list, or Imported.")
        case "import_already_running": return Words.text("An import is already running. Wait for it to end or cancel it.")
        case "import_not_running": return Words.text("That import is not running.")
        case "import_bad_pick": return Words.text("Pick an Outlook archive (.olm) or a Thunderbird or Unix mailbox file (.mbox).")
        case "import_store_failed": return Words.text("The local database refused the imported mail. What was stored stays; try again.")
        default: return words
        }
    }
    /// The code a source's `access` word stands for (the contract's `ImportAccess`), nil when readable.
    static func accessCode(_ access: String) -> String? {
        switch access {
        case "needs_full_disk_access": return "import_needs_full_disk_access"
        case "not_found": return "import_source_not_found"
        case "unreadable": return "import_source_unreadable"
        default: return nil
        }
    }
}
