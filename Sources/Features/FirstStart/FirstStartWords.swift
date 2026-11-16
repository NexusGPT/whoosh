import Foundation

/// Every string of the first-start flow, through `Words.text` (key = the English text, 14 languages in the catalogue). The copy is
/// the boards' (3a for step 1, 2b for steps 2 to 5), word for word. The app's name is a name, not a string to translate.
@MainActor
enum FirstStartWords {
    static let appName = "Whoosh"

    // the door that shows the flow again (Settings, General and the palette)
    static var quickStart: String { Words.text("Quick start") }
    static var quickStartLine: String { Words.text("The ten steps of the first launch") }
    static var showAgain: String { Words.text("Show again") }
    /// The palette's line: the ENGLISH key, as the palette's catalogue translates it and finds its door by the key's own words (`mac-palette.result.show-the-quick-start`).
    static let paletteKey = "Show the quick start"

    // the card's chrome
    static var back: String { Words.text("Back") }
    static var skip: String { Words.text("Skip") }
    static var continueWord: String { Words.text("Continue") }
    static var finish: String { Words.text("Finish") }
    static func stepOf(_ n: Int) -> String { Words.text("Step %@ of %@", n, FirstStartModel.stepCount) }
    static func countOf(_ n: Int) -> String { Words.text("%@ of %@", n, FirstStartModel.stepCount) }

    // step 1 (3a)
    static var welcomeKicker: String { Words.text("Welcome to") }
    static var welcomeBody: String { Words.text("All your inboxes in one fast, quiet window, sorted by a model that never leaves this Mac.") }

    // step 2
    static var searchKicker: String { Words.text("Search") }
    static var searchTitle: String { Words.text("Find it by what it means") }
    static var searchBody: String { Words.text("Type what you remember, not the exact words. Whoosh searches every account and every year at once.") }
    static var tryIt: String { Words.text("Try it: type a word.") }
    static var searchOr: String { Words.text("or") }
    static var searchEverywhere: String { Words.text("opens search from anywhere") }
    static var searchPlaceholder: String { Words.text("Search or type a command") }
    static var sampleMail: String { Words.text("Sample mail") }
    static func foundByMeaning(_ typed: String, _ sender: String) -> String { Words.text("Found by meaning: %@ ≈ %@", typed, sender) }
    static func noSample(_ typed: String) -> String { Words.text("No sample line matches “%@”. Try boat, invoice, logo or race.", typed) }

    // step 3
    static var keysKicker: String { Words.text("Keys") }
    static var keysTitle: String { Words.text("Five keys do most of it") }
    static var keysBody: String { Words.text("One letter each, never a chord. Watch the stage, then try them: J and K move, E marks done, H reminds you later.") }
    static var capDone: String { Words.text("Done") }
    static var capRemind: String { Words.text("Remind") }
    static var capCompose: String { Words.text("Compose") }
    static var capSearch: String { Words.text("Search") }
    static var capDown: String { Words.text("Down") }
    static var capUp: String { Words.text("Up") }
    static var markedDone: String { Words.text("Marked done") }
    static var undo: String { Words.text("Undo") }
    static var reminderTomorrow: String { Words.text("Reminder: tomorrow 08:00") }
    static var tomorrowChip: String { Words.text("Tomorrow 08:00") }

    // step 4
    static var labelsKicker: String { Words.text("Labels") }
    static var labelsTitle: String { Words.text("Your rules first, then the model") }
    static var labelsBody: String { Words.text("Every line carries its label and who decided it. One of your rules wins when it matches. When none does, the model on this Mac picks.") }
    static var yourRules: String { Words.text("Your rules") }
    static var youWriteThem: String { Words.text("you write them in Labels") }
    static var modelOnMac: String { Words.text("The model on this Mac") }
    static var whenNoRule: String { Words.text("when no rule matches") }
    static var nodeFirst: String { Words.text("first") }
    static var nodeModel: String { Words.text("The model") }
    static var nodeOnThisMac: String { Words.text("on this Mac") }
    static func decidedByRule(_ rule: String) -> String { Words.text("decided by: your rule “%@”", rule) }
    static var decidedByModel: String { Words.text("decided by: the model on this Mac") }

    // step 5
    static var privacyKicker: String { Words.text("Privacy") }
    static var privacyTitle: String { Words.text("Your mail stays on this Mac") }
    static var privacyBody: String { Words.text("Your mail, the search index and the model that sorts it live on this Mac. Two parts are optional and stay off until you turn them on.") }
    static var jevKey: String { Words.text("Jev key") }
    static var jevHow: String { Words.text("Decisions by a hosted model, your own key") }
    static var pushRelay: String { Words.text("Push relay") }
    static var pushHow: String { Words.text("Wakes your iPhone through the relay you choose") }
    static var nothingSent: String { Words.text("Nothing is sent to Microsoft or Google.") }
    static var artMail: String { Words.text("your mail") }
    static var artIndex: String { Words.text("index") }
    static var artModel: String { Words.text("model") }
    static var thisMac: String { Words.text("This Mac") }
    static var off: String { Words.text("off") }

    // step 6 and the skip state
    static var yourAccount: String { Words.text("Your account") }
    static var appearsHere: String { Words.text("appears here") }
    static var fetching: String { Words.text("Fetching your history") }
    static var noAccount: String { Words.text("No account yet") }
    static var fillInbox: String { Words.text("Add one to fill this inbox.") }
    static var addAccount: String { Words.text("Add account") }
}
