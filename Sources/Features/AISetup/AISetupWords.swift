import Foundation

/// Every string of the AI setup read model and step, through `Words.text` (key = the English text, 14 languages in the catalogue).
/// Writing rule (docs/CONSTRAINTS.md last row): a noun names the thing, one name per thing, one short line.
/// The three names: Writing model (an LLM provider), Decision model (Laya or Jev), Meaning model (search and Sources).
@MainActor
enum AISetupWords {
    static var writingName: String { Words.text("Writing model") }
    static var decisionName: String { Words.text("Decision model") }
    static var meaningName: String { Words.text("Meaning model") }

    static var writingReady: String { Words.text("A provider is connected.") }
    static var writingMissing: String { Words.text("No provider is connected. Paste a key.") }
    static var writingChecking: String { Words.text("Checking the key…") }
    static var writingRefused: String { Words.text("The provider refused the key. Paste another.") }

    static var layaReady: String { Words.text("The decision model is on this Mac.") }
    static var layaMissing: String { Words.text("The decision model is not downloaded.") }
    static func downloading(_ received: String, _ total: String) -> String { Words.text("Downloading %@ of %@", received, total) }
    /// the figure the design draws; the store learns the real total at the first byte
    static let layaBytes: Int64 = 843_000_000
    /// The words as drawn: a size keeps its number and its unit on one line ("843 MB" broke after 843 in the first-start card at 960x600, main170).
    static func unbroken(_ words: String) -> String {
        // \x20 is the space: a literal space in this pattern reads as a sentence to the language check (checks/i18n-check.py)
        words.replacingOccurrences(of: #"(\d)\x20(?=(?:[kKMGT]B|[kMGT]o)\b)"#, with: "$1\u{00A0}", options: .regularExpression)
    }
    static var jevReady: String { Words.text("Jev decides, with your key and your consent.") }
    static var jevNoKey: String { Words.text("The Jev key is missing.") }
    static var jevNoConsent: String { Words.text("Jev needs your consent.") }
    static var decisionOff: String { Words.text("Decisions are off. Rules only.") }

    static var meaningReady: String { Words.text("The meaning model is on this Mac.") }
    static var meaningFailedFallback: String { Words.text("The meaning model could not be downloaded.") }
    static var meaningMissing: String { Words.text("The meaning model is not on this Mac.") }

    // the doors: which control fixes it
    static var doorKey: String { Words.text("Paste a key") }
    /// a button that starts a download takes the verb; the key "Download" stays the noun (a title, a heading)
    static var doorDownload: String { Words.text("Download model") }
    static var doorJevKey: String { Words.text("Paste the Jev key") }
    static var doorConsent: String { Words.text("Give consent") }
    static var doorTurnOn: String { Words.text("Choose a decision model") }
    static var doorMeaning: String { Words.text("Download model") }

    // the mark (design W1): one line that says what is missing, one button that fixes it
    static func notOnThisMac(_ size: String) -> String { Words.text("Not on this Mac yet. %@, downloaded once.", size) }
    static var cannotSort: String { Words.text("The AI cannot sort yet: the decision model is not on this Mac.") }
    static var needsWriting: String { Words.text("needs a writing model") }
    static var noWriting: String { Words.text("No writing model is connected.") }
    static var searchOff: String { Words.text("Search by meaning is off: the meaning model is not on this Mac.") }
    static var connectModel: String { Words.text("Connect a model") }
    static func downloadSize(_ size: String) -> String { Words.text("Download · %@", size) }
    static var cancel: String { Words.text("Cancel") }
    static var retry: String { Words.text("Retry") }
    static var replyWithAI: String { Words.text("Reply with AI · connect a model") }

    // the quick-start step (design Q1)
    static var stepKicker: String { Words.text("AI") }
    static var stepTitle: String { Words.text("What the AI may use") }
    static var stepBody: String { Words.text("Three things make the AI parts work. Set them now or later: Whoosh marks what is missing.") }
    static var sorting: String { Words.text("Sorting") }
    /// the size is read from the byte count and written in the Mac's own units, never typed into the sentence
    static var sortingLine: String { Words.text("Laya, on this Mac. %@, once.", AISetupStatus.byteWords(layaBytes)) }
    static var writingAsking: String { Words.text("Writing and asking") }
    static var writingLine: String { Words.text("Your key for Anthropic, OpenAI, Google or Mistral. Kept in the Keychain.") }
    static var searchByMeaning: String { Words.text("Search by meaning") }
    static var meaningLine: String { Words.text("On this Mac. %@, once.", AISetupStatus.byteWords(MeaningModelStore.shared.spec.totalBytes)) }
    static var useJev: String { Words.text("Use Jev") }
    static var pasteKey: String { Words.text("Paste a key") }
    static var pasteJevKey: String { Words.text("Paste your Jev key") }
    static var connected: String { Words.text("Connected") }
    static var ready: String { Words.text("Ready") }
    static var allow: String { Words.text("Allow") }
    static var jevSent: String { Words.text("The text of the mail a label's AI card ticks is sent to Jev to sort it. Nothing else.") }
    static func providerModels(_ name: String, _ n: Int) -> String { Words.text("%@ · %lld models", name, n) }
    static var stageTitle: String { Words.text("Sorted, written, found.") }
    static var stageSorting: String { Words.text("Sorting your mail into labels") }
    static var stageWriting: String { Words.text("Summaries, drafts, Ask") }
    static var stageSearch: String { Words.text("Search by meaning") }
}
