import Foundation

/// Every string of the Réponses step (first start, step 7), through `Words.text`; the key is the English text (Sources/Resources/Localizable.xcstrings).
/// The French text is the approved board's (docs/design/boards/ai-replies-r5, the FS(...) screen) word for word.
@MainActor
enum RepliesStepWords {
    static var kicker: String { Words.text("Replies") }
    static var title: String { Words.text("Replies written like you") }
    static var body: String { Words.text("Whoosh prepares a draft; you read it and send it. Three choices are enough, and you change everything later in Settings.") }

    static var draftRow: String { Words.text("Write a draft") }
    static var searchRow: String { Words.text("Search in") }
    static var voiceRow: String { Words.text("Write like") }
    static var searchCaption: String { Words.text("Indexed on this Mac. Nothing leaves it.") }
    static var voiceCaption: String { Words.text("An analysis of the messages you sent.") }

    static var onDemand: String { Words.text("When I ask") }
    static var automatic: String { Words.text("Automatically") }
    static var sentAndInbox: String { Words.text("Sent and Inbox") }
    static var inboxOnly: String { Words.text("Inbox only") }
    static var sentOnly: String { Words.text("Sent only") }

    static func voice(_ n: Int) -> String { Words.text("My last %lld messages", n) }

    // the example on the stage
    static var example: String { Words.text("Example") }
    static var draftMark: String { Words.text("Draft") }
    static var exampleGreeting: String { Words.text("Hello Claire,") }
    static var exampleText: String { Words.text("I confirm the revised price of the quote of 3 November…") }

    static func draft(_ c: RepliesDraftChoice) -> String { c == .auto ? automatic : onDemand }
    static func search(_ c: RepliesSearchChoice) -> String {
        switch c { case .sentAndInbox: return sentAndInbox; case .inbox: return inboxOnly; case .sent: return sentOnly }
    }
}
