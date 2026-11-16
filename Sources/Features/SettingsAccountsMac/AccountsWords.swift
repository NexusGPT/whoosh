import Foundation

/// Every word the Accounts pane shows, in one place (owner's register: plain, short, no jargon). Each string goes
/// through the 14-language catalogue (`Words.text`, docs/lanes/i18n.md).
enum AccountsWords {
    static var title: String { Words.text("Accounts") }
    static var paletteTitle: String { Words.text("Accounts settings") }
    static func lead(count: Int, failing: Int) -> String {
        let n = count == 1 ? Words.text("1 account") : Words.text("%@ accounts", count)
        if failing == 0 { return Words.text("%@. All of them are working.", n) }
        return failing == 1 ? Words.text("%@. One needs you.", n) : Words.text("%@. %@ need you.", n, failing)
    }
    static var signInAgain: String { Words.text("Sign in again") }
    static var notifications: String { Words.text("Notifications") }
    static var noNotifications: String { Words.text("no notifications") }
    static var iconButton: String { Words.text("Icon and colour") }
    static var moveTitle: String { Words.text("Move accounts to iPhone") }
    static var moveLine: String { Words.text("Scan a code on your iPhone. Your accounts go across on this Wi-Fi. Your passwords never leave this Mac.") }
    static var moveButton: String { Words.text("Move accounts to iPhone") }
    static var footer: String { Words.text("Whoosh keeps your mail on this Mac. Turning notifications off only stops the banners and sounds for that account.") }
    static var syncing: String { Words.text("Reading your mail for the first time") }
    static var neverSynced: String { Words.text("Not synced yet") }
    static var paused: String { Words.text("Paused") }
    static var servicesTitle: String { Words.text("Mail services") }
    static var otherGroup: String { Words.text("Other accounts") }
    static var findPlaceholder: String { Words.text("Find an account") }
    static var findNothing: String { Words.text("No account matches that.") }
    static func history(percent: Int) -> String { Words.text("Reading older mail, %@ done", "\(percent)%") }
    static func groupCount(_ n: Int) -> String { n == 1 ? Words.text("1 account") : Words.text("%@ accounts", n) }

    // Icon picker
    static var pickerTitle: String { Words.text("Icon and colour") }
    static var pickerLead: String { Words.text("Pick a picture and a colour. You see it on your list, in the sidebar and in each message.") }
    static var colourHeader: String { Words.text("Colour") }
    static var uploadHeader: String { Words.text("Your own image") }
    static var uploadButton: String { Words.text("Choose an image") }
    static var uploadHint: String { Words.text("Or drop one here. It is cut to a square.") }
    static var done: String { Words.text("Done") }
    static var removeImage: String { Words.text("Use a picture from the library") }
    static func tooBig(limit: Int) -> String { Words.text("That image is too big. The limit is %@ MB.", limit / 1_048_576) }
    static var notAnImage: String { Words.text("That file is not a picture Whoosh can read.") }
    static var couldNotSave: String { Words.text("Whoosh could not keep that image on this Mac.") }

    // The window's account list and the account's own page
    static var needsYou: String { Words.text("Needs you") }
    static func needsYouCount(_ n: Int) -> String { Words.text("Needs you · %lld", n) }
    static func workingCount(_ n: Int) -> String { Words.text("Working · %lld", n) }
    static var working: String { Words.text("Working") }
    static var addAccount: String { Words.text("Add account") }
    static var moveToIphone: String { Words.text("Move to iPhone") }
    static var accountSection: String { Words.text("Account") }
    static var nameTitle: String { Words.text("Name") }
    static var addressTitle: String { Words.text("Address") }
    static var providerTitle: String { Words.text("Provider") }
    static var syncSection: String { Words.text("Sync") }
    static var statusTitle: String { Words.text("Status") }
    static var pauseTitle: String { Words.text("Pause syncing") }
    static var pauseLine: String { Words.text("Keeps the account and its mail on this Mac, stops fetching and sending.") }
    static var bannersTitle: String { Words.text("Banners and sounds") }
    static var bannersLine: String { Words.text("Mail still arrives when this is off.") }
    static var removeButton: String { Words.text("Remove account") }
    static var removeTitle: String { Words.text("Remove this account?") }
    static var removeLine: String { Words.text("Whoosh forgets the account on this Mac. Your mail stays with your provider.") }
    static var signatureTitle: String { Words.text("Signature") }
    static var signaturePlaceholder: String { Words.text("None") }
    static var deliveryTitle: String { Words.text("Delivery") }
    static func delivery(_ d: NotificationDelivery) -> String {
        switch d { case .notch: return Words.text("Notch"); case .notifications: return Words.text("Notifications"); case .both: return Words.text("Notch and notifications") }
    }
    static var cancel: String { Words.text("Cancel") }
    static var needsSignInLine: String { Words.text("Needs you: sign in again") }
    static var allWorking: String { Words.text("Working") }
    static var open: String { Words.text("Open") }
    static func unreadCount(_ n: Int) -> String { Words.text("%lld unread", n) }

    static func familyTitle(_ raw: String) -> String { raw.prefix(1).uppercased() + raw.dropFirst() }

    static func provider(_ p: MailProvider) -> String {
        switch p {
        case .google: return Words.text("Gmail")
        case .microsoft: return Words.text("Outlook.com")
        case .icloud: return Words.text("iCloud")
        case .fastmail: return Words.text("Fastmail")
        case .imap: return Words.text("Other mail (IMAP)")
        }
    }
    static func sentence(_ s: String) -> String { s.prefix(1).uppercased() + s.dropFirst() }
}

/// The words only the Import mail sheet adds (the rest are ImportWords, the first start's own: one name per thing).
enum AccountsImportWords {
    static var chooseFile: String { Words.text("Choose a file…") }
    static var needsAccess: String { Words.text("Whoosh needs Full Disk Access to read Apple Mail.") }
    static var nothingNew: String { Words.text("Nothing new: every message is already here.") }
    /// "12,480 messages imported into Imported": the count with its thousands mark, the account's name.
    static func imported(_ n: Int, into account: String) -> String {
        let f = NumberFormatter(); f.numberStyle = .decimal
        return Words.text("%@ messages imported into %@", f.string(from: NSNumber(value: n)) ?? "\(n)", account)
    }
}
