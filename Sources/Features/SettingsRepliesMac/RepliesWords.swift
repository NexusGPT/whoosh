import Foundation

/// Every word of Settings › Réponses, through the catalogue (keys in English, the board's French in Localizable.xcstrings; the new keys and their
/// French are listed in docs/lanes/settingsreplies-mac.md "New strings"). Titles name the thing; one plain short line per setting (owner's
/// standing rule, docs/CONSTRAINTS.md last row).
enum RepliesWords {
    static var replies: String { Words.text("Replies") }
    static var enabled: String { Words.text("On") }
    static var on: String { Words.text("On") }
    static var off: String { Words.text("Off") }
    static var forWhich: String { Words.text("For which messages") }
    static var tryOnMessage: String { Words.text("Try on a message") }
    static var entryFilter: String { Words.text("Entry filter") }
    static var aiDecision: String { Words.text("AI decision") }
    static var exitFilter: String { Words.text("Exit filter") }
    static var cardOff: String { Words.text("Turned off: every message passes") }
    static var withWhat: String { Words.text("With what") }
    static var how: String { Words.text("How") }
    static var sources: String { Words.text("Sources") }
    static var tone: String { Words.text("Tone") }
    static var myVoice: String { Words.text("My voice") }
    static var matchThread: String { Words.text("In a thread, write like my earlier replies") }
    static var prompt: String { Words.text("Prompt") }
    static var repliesPrompt: String { Words.text("The replies prompt") }
    static var everything: String { Words.text("Every message") }
    static var inThread: String { Words.text("In a thread") }
    static var writeDraft: String { Words.text("Write a draft") }
    static var instructionBefore: String { Words.text("Give an instruction first") }
    static var instructionBeforeLine: String { Words.text("A field to say what to reply, in one sentence.") }
    static var changeAfter: String { Words.text("Ask for a change after") }
    static var changeAfterLine: String { Words.text("A field under the draft.") }
    static var entryLine: String { Words.text("what does not pass is never read by the AI") }
    static var aiLine: String { Words.text("from the description") }
    static var exitLine: String { Words.text("what the AI kept must pass it") }
    static var andWhen: String { Words.text("And when") }
    static var allOfThem: String { Words.text("All of them") }
    static var oneOfThem: String { Words.text("One of them") }
    static var areTrue: String { Words.text("of these conditions are true") }
    static var isTrue: String { Words.text("of these conditions is true") }
    static var allLabels: String { Words.text("All labels") }
    static var noLabelTicked: String { Words.text("No label ticked: no message gets a draft.") }
    static var sender: String { Words.text("Sender") }
    static var senderHint: String { Words.text("name, address or domain") }
    static var clear: String { Words.text("Clear") }
    static var restOfThread: String { Words.text("Rest of the thread") }
    static var descriptionHint: String { Words.text("In your words: which messages wait for a reply from you.") }
    /// The AI card's shipped description (the real engine seeds this English text, core/crates/store/migrations/sql/0013_ai_replies.sql:92; the test
    /// engine the same). "don't mix languages": while the stored text is still a default (the seed or any of its 14 catalogue texts) the screen shows the
    /// catalogue sentence of the app's language; text the owner typed is shown as typed (hid-ai round 6, main92 pictures: a French sentence in English).
    static let aiDescriptionDefaultKey = "Emails that wait for a reply from me: a question, a request, a decision that someone is waiting for."
    static func aiDescriptionShown(_ stored: String) -> String {
        Words.allTexts(aiDescriptionDefaultKey).contains(stored.trimmingCharacters(in: .whitespacesAndNewlines)) ? Words.text(aiDescriptionDefaultKey) : stored
    }
    /// hid-ai round 18 (lead's ruling on "don't mix languages ... consistent language throughout"): the three other model-facing texts the engine
    /// ships in English (the reply prompt: store migration 0013_ai_replies.sql:34-37 = contracts reply_ai_logic.rs:423-428; the retrieval prompt and the
    /// AI filter question: reply_ai_logic.rs:264 and :268). Each key is that English text, letter for letter; its 14 catalogue texts keep the same
    /// `{{variables}}`, line count and `//` note line. The untouched marker is the text itself: a stored text equal to the key or to any of its catalogue
    /// texts is still the shipped default (no flag in the store).
    static let promptDefaultKey = "You are replying to {{message}}. You have read {{thread}} and {{excerpts}}. My availability is in {{calendar}}.\nWrite the reply in this tone: {{tone}}. Follow {{instruction}} when there is one. Always keep in mind: {{notes}}.\nAnswer the question first. Never promise a price or a deadline.\n// One sentence per rule. This line is a note: the model does not read it."
    static let retrievalPromptDefaultKey = "From {{message}} and {{instruction}}, write up to 3 short questions to find what I need to know before I reply."
    static let aiFilterDefaultKey = "Does this excerpt help to answer?"
    @MainActor private static var defaultTexts: [String: Set<String>] = [:]
    /// The app language's text of a shipped default when `stored` is still that default in another language; nil when the owner wrote it, or when
    /// it is already in the app's language (nothing to write).
    @MainActor static func languageDefault(_ stored: String, key: String) -> String? {
        let all: Set<String>
        if let known = defaultTexts[key] { all = known } else { all = Words.allTexts(key); defaultTexts[key] = all }
        let s = stored.trimmingCharacters(in: .whitespacesAndNewlines)
        let here = Words.text(key)
        return all.contains(s) && s != here ? here : nil
    }
    static var addSource: String { Words.text("Add a source") }
    static var addFolder: String { Words.text("Add a folder") }
    static var noAccount: String { Words.text("No mail account yet.") }
    static var calendars: String { Words.text("Calendars") }
    static var calendarsNote: String { Words.text("Managed in Settings › Calendars") }
    static var noCalendar: String { Words.text("No calendar connected") }
    static var searchIn: String { Words.text("To reply from an account, search in") }
    static var thisAccount: String { Words.text("This account") }
    static var questions: String { Words.text("The questions") }
    static var retrievalPromptName: String { Words.text("The retrieval prompt") }
    static var excerpts: String { Words.text("The excerpts") }
    static var perQuestion: String { Words.text("Excerpts per question") }
    static var excerptSize: String { Words.text("Size of an excerpt") }
    static var attached: String { Words.text("Joined to each excerpt") }
    static var sorting: String { Words.text("The sorting") }
    static var keep: String { Words.text("Keep only") }
    static var clash: String { Words.text("If two excerpts disagree") }
    static var aiFilter: String { Words.text("AI filter") }
    static var aiFilterLine: String { Words.text("The AI keeps an excerpt only when it answers yes to this question.") }
    static var aiFilterHint: String { Words.text("Does this excerpt help to reply?") }
    static var noFilter: String { Words.text("No filter") }
    static var analysis: String { Words.text("Analysis") }
    static var analyseSent: String { Words.text("Analyse my sent messages") }
    static var analyse: String { Words.text("Analyse") }
    static var myVoiceNote: String { Words.text("Written by the analysis. You can change it.") }
    static var examples: String { Words.text("Examples") }
    static var samePerson: String { Words.text("Join my replies to the same person") }
    static var otherTones: String { Words.text("Other tones") }
    static var toneName: String { Words.text("Name of the tone") }
    static var toneWords: String { Words.text("How it writes, in one sentence") }
    static var none: String { Words.text("None") }
    static var noOtherTone: String { Words.text("No other tone") }
    static var seeWhatModelGets: String { Words.text("See what the model receives") }
    static var variables: String { Words.text("Variables") }
    static var fixedNotes: String { Words.text("Fixed notes") }
    static var noteTitle: String { Words.text("Title of the note") }
    static var noteText: String { Words.text("The note, in one sentence") }
    static var versions: String { Words.text("Versions") }
    static var restore: String { Words.text("Restore") }
    static var revert: String { Words.text("Revert") }
    static var previewOfDraft: String { Words.text("The text being edited, not saved yet.") }
    static func previewOfSaved(_ v: UInt32) -> String { Words.text("The saved text, version %@.", RepliesWords.n(v)) }
    static var notSavedYet: String { Words.text("Not saved") }
    static var revertAsk: String { Words.text("Press Esc again to go back to the saved text.") }
    static var noVariable: String { Words.text("No variable starts like this.") }
    static var newNoteHint: String { Words.text("The note, in your words.") }
    static var draft: String { Words.text("The draft") }
    static var noDraft: String { Words.text("No draft: a step stopped this message.") }
    static var nothingSent: String { Words.text("A trial stores nothing and sends nothing.") }
    static var noMessage: String { Words.text("No message to try on.") }
    static var notReady: String { Words.text("Not available yet.") }
    static var notFound: String { Words.text("Not found") }
    static var notSaved: String { Words.text("Not saved") }
}
