import Foundation

/// Every word this screen shows, from the catalogue through `Words.text` (the app's language, switchable without a
/// restart). A key the catalogue lacks shows as its English text until the i18n lane adds it; the new keys are listed
/// in docs/lanes/threadphone.md.
enum ThreadPhoneWords {
    static var notDownloaded: String { Words.text("Not downloaded yet") }
    static var copy: String { Words.text("Copy") }
    static var copied: String { Words.text("Copied") }
    static var signInCode: String { Words.text("Sign-in code") }
    static var reply: String { Words.text("Reply") }
    static var archive: String { Words.text("Archive") }
    static var snooze: String { Words.text("Snooze") }
    static var pin: String { Words.text("Pin") }
    static var unpin: String { Words.text("Unpin") }
    static var more: String { Words.text("More") }
    static var back: String { Words.text("Inbox") }
    static var save: String { Words.text("Save") }
    static var done: String { Words.text("Done") }
    static var unread: String { Words.text("Unread") }
    static var attachments: String { Words.text("Attachments") }
    // New keys (not in the catalogue yet).
    static var note: String { Words.text("Note") }
    static var markUnread: String { Words.text("Mark unread") }
    static var notePlaceholder: String { Words.text("A private note on this thread") }
    static var noteLine: String { Words.text("Only you see it.") }
    static var unavailableTitle: String { Words.text("This message is not here") }
    static var unavailableLine: String { Words.text("It may have left the inbox. Go back to the list.") }
    static func to(_ names: String) -> String { Words.text("To %@", names) }
    static func codeLine(_ code: String) -> String { signInCode + " " + code }
}
