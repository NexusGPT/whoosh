import Foundation

/// Every line this screen says, in one place, through the 14-language catalogue (docs/lanes/i18n.md).
enum LabelsWords {
    static var oneMessage: String { Words.text("1 message") }
    static var pickPerson: String { Words.text("Pick a person") }
    static var clickSender: String { Words.text("Click a sender's name in any message to see everything they sent you.") }
    static var labelName: String { Words.text("Label name") }
    static var name: String { Words.text("Name") }
    static var forExample: String { Words.text("for example Provanta") }
    static var dropHint: String { Words.text("Drop a label on another to nest it, on Labels to take it out") }
    static var pickLabel: String { Words.text("Pick a label") }
    static var noMail: String { Words.text("No mail carries this label yet. Press L on a message, in any account, to file it here.") }
    static var labels: String { Words.text("Labels") }
    static var dropHere: String { Words.text("Drop a label here to take it out of its parent") }
    static var smartFolders: String { Words.text("Smart folders") }
    static var people: String { Words.text("People") }
    static var smartFolder: String { Words.text("Smart folder") }
    static var showsMail: String { Words.text("Shows mail that is") }
    static var livesUnder: String { Words.text("Lives under") }
    static var save: String { Words.text("Save") }
    static var cancel: String { Words.text("Cancel") }
    static var ruleHint: String { Words.text("from someone, has attachment, mentioning a word") }
    static var topLevel: String { Words.text("Top level") }
    static var nothingMatches: String { Words.text("Nothing matches these words yet. The folder stays empty until a message does.") }
    static var mergeInto: String { Words.text("Merge into one card") }
    static var newFolder: String { Words.text("New smart folder") }
    static var folderName: String { Words.text("Folder name") }
    static var folderNameHint: String { Words.text("for example Supplier quotes") }
    static var gone: String { Words.text("That label is gone.") }
    static func messages(_ n: any CustomStringConvertible) -> String { Words.text("%lld messages", n) }
    /// The AI card's count: what the AI accepted, and the picked mails still waiting for its ruling.
    /// `aside`: the picked mails Apple's model refused for themselves (content, language, size), set aside so they are not asked again (`label/<id>/flow/ai_set_aside`).
    static func accepted(_ n: Int, waiting: Int, aside: Int = 0) -> String {
        let base = waiting > 0 ? Words.text("%lld accepted · %lld waiting for the AI", n, waiting) : Words.text("%lld accepted", n)
        return aside > 0 ? base + Words.text(" · %lld that Apple's model would not read", aside) : base
    }
    static func sameSender(_ other: any CustomStringConvertible) -> String { Words.text("%@ looks like the same sender.", other) }
    static func level(_ depth: any CustomStringConvertible, _ max: any CustomStringConvertible) -> String { Words.text("Level %@ of %@", depth, max) }
    static func newInside(_ name: any CustomStringConvertible) -> String { Words.text("New label inside %@", name) }
    static func holdsNone(_ name: any CustomStringConvertible, _ max: any CustomStringConvertible) -> String { Words.text("%@ is %@ levels deep, so it holds no labels of its own.", name, max) }
    static func nestLine(_ max: any CustomStringConvertible) -> String { Words.text("Labels nest up to %@ levels, like Clients > Provanta.", max) }
    static func inFolder(_ detail: any CustomStringConvertible) -> String { Words.text("in %@", detail) }
}
