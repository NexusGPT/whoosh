import Foundation
import Testing
@testable import WhooshUI

// SPEC A144: a reply the engine cannot attach to its conversation shows ONE catalogue sentence (the app's language), and the engine's own
// words are reachable under Details. Each test names, in its trailing comment, the one-line source edit (its "broken copy") that turns it red.
// No fixture door drives this state end to end (see docs/lanes/reply-words.md), so the tests call the two places that choose the words.

private let engineWords = "the message you reply to could not be read, so the reply cannot be threaded yet: the server has no message with UID 143986"
private let field = "reply_to_thread_id"
private func refusal(_ f: String = field, _ w: String = engineWords) -> Error { CoreError.Invalid(field: f, reasonWords: w) }

@MainActor @Test func theComposerShowsTheCatalogueSentenceForAReplyTheEngineCannotThread() {
    let shown = ComposerModel.words(refusal())
    #expect(shown == Words.text("This reply cannot be linked to its conversation yet."))
    #expect(!shown.contains("UID") && !shown.contains("threaded") && shown != engineWords, "the engine's English is on the line: \(shown)")
    // broken copy: ComposerWords.swift, delete the line `case replyThreadField: return Words.text("This reply cannot be linked to its conversation yet.")`
}

@MainActor @Test func theComposerKeepsTheEngineWordsBehindDetailsOnlyWhileTheSentenceShows() {
    #expect(ComposerModel.details(refusal()) == engineWords)
    let m = ComposerModel()
    m.line = ComposerModel.words(refusal())
    m.lineDetail = ComposerModel.details(refusal()).map { (m.line, $0) }
    #expect(m.lineHelp == engineWords)
    m.line = "Attached a.pdf"
    #expect(m.lineHelp == nil, "a later line must not carry the old details")
    // broken copy: ComposerModel+Send.swift, in `details` change `ComposerWords.refusal(field) != nil` to `false` (the engine's words are lost); and in ComposerModel.swift
    // `lineHelp` change `$0.line == line` to `true` (second expectation)
}

@MainActor @Test func aReplyRefusalKeepsTheComposerFailedWithRetryNotAFileRefusal() {
    #expect(ComposerModel.refusalCode(refusal()) == nil, "a file refusal would drop the failed phase and its Retry")
    #expect(ComposerModel.refusalCode(refusal("attachment_too_large", "x")) == "attachment_too_large")
    // broken copy: ComposerModel+Files.swift, in `refusalCode` delete `field != ComposerWords.replyThreadField, `
}

@MainActor @Test func theQuickReplyShowsTheCatalogueSentenceAndTheEngineWordsBehindDetails() {
    let shown = QuickReplyModel.refusalWords(field: field, engine: engineWords)
    #expect(shown.words == Words.text("This reply cannot be linked to its conversation yet."))
    #expect(!shown.words.contains("UID") && shown.words != engineWords, "the engine's English is on the card: \(shown.words)")
    #expect(shown.details == engineWords)
    let q = QuickReplyModel()
    q.phase = .failed(words: shown.words)
    q.failedDetail = shown.details.map { (shown.words, $0) }
    #expect(q.failureWords == shown.words && q.failureDetails == engineWords)
    q.phase = .failed(words: "another")
    #expect(q.failureDetails == nil, "details of an earlier failure must not follow a later one")
    // broken copy: QuickReplyModel.swift, in the `catch CoreError.Invalid` branch use `phase = .failed(words: words)` (the raw words), and in `refusalWords`
    // return `(engine, nil)` first line of the guard body; the last expectation turns red with `d.words == w` changed to `true` in `failureDetails`
}

@MainActor @Test func anUnknownFieldStillShowsTheEnginesWordsOnBothScreens() {
    let words = "an agent needs a name"
    #expect(ComposerModel.words(refusal("name", words)) == words)
    #expect(ComposerModel.details(refusal("name", words)) == nil)
    let q = QuickReplyModel.refusalWords(field: "name", engine: words)
    #expect(q.words == words && q.details == nil)
    // broken copy: ComposerWords.swift, in `refusal` change `default: return nil` to `default: return Words.text("This reply cannot be linked to its conversation yet.")`
}
