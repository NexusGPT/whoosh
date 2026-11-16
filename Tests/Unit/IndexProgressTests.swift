import Foundation
import Testing
@testable import WhooshUI

// The index lines (status line, Settings card): the pure sentences for each state, and the one observable model that follows the builder's
// announcement. Each sentence test pins the language it expects (`LanguagePin`, Tests/Support), so it reads the same on any Mac.

private func status(indexed: Int = 0, queued: Int = 0, paused: Bool = false, failed: Int = 0) -> MeaningIndexBuilder.Status {
    var s = MeaningIndexBuilder.Status()
    s.indexed = indexed; s.queued = queued; s.building = queued > 0; s.pausedForMemory = paused; s.failed = failed
    return s
}

@Test func statusLineSaysNothingWhenNothingIsQueued() {
    #expect(IndexProgress.statusLinePart(status(indexed: 12_000)) == nil)
    #expect(IndexProgress.statusLinePart(status()) == nil)
    #expect(IndexProgress.statusLinePart(status(indexed: 5, paused: true)) == nil)
}

@Test func statusLineSaysTheShareWhileBuilding() {
    LanguagePin.with("en") {
        #expect(IndexProgress.statusLinePart(status(indexed: 1_200, queued: 10_800)) == "indexing 10 %")
        #expect(IndexProgress.statusLinePart(status(indexed: 0, queued: 500)) == "indexing 0 %")
        #expect(IndexProgress.statusLinePart(status(indexed: 11_999, queued: 1)) == "indexing 99 %")
    }
}

@Test func statusLineSaysPausedInPlaceOfThePercentage() {
    LanguagePin.with("en") {
        #expect(IndexProgress.statusLinePart(status(indexed: 6_400, queued: 5_600, paused: true)) == "indexing paused")
    }
}

@Test func settingsLineGivesTheFiguresAndTheBarWhileBuilding() {
    LanguagePin.with("en") {
        let line = IndexProgress.settingsLine(status(indexed: 1_240, queued: 10_760))
        #expect(line?.text == "Indexing your mail: 1,240 of 12,000")
        #expect(abs((line?.fraction ?? -1) - 1_240.0 / 12_000.0) < 1e-9)
    }
}

@Test func settingsLineSaysDoneWithoutABar() {
    LanguagePin.with("en") {
        let line = IndexProgress.settingsLine(status(indexed: 12_000))
        #expect(line?.text == "12,000 mails indexed.")
        #expect(line?.fraction == nil)
        #expect(IndexProgress.settingsLine(status(indexed: 1))?.text == "1 mail indexed.")
    }
}

@Test func settingsLineSaysPausedAtTheFigures() {
    LanguagePin.with("en") {
        let line = IndexProgress.settingsLine(status(indexed: 6_400, queued: 5_600, paused: true))
        #expect(line?.text == "Paused at 6,400 of 12,000: memory is full. It resumes by itself.")
    }
}

@Test func settingsLineSaysHowManyMailsCouldNotBeRead() {
    LanguagePin.with("en") {
        #expect(IndexProgress.settingsLine(status(indexed: 11_988, failed: 12))?.text == "11,988 of 12,000 indexed. 12 mails could not be read.")
        #expect(IndexProgress.settingsLine(status(indexed: 9, failed: 1))?.text == "9 of 10 indexed. 1 mail could not be read.")
    }
}

// The language path, not avoided: the same lines in French read the French catalogue and the French number format (the thousands separator is
// French's own space, never the English comma). Fails if a line ignores the chosen language (English text) or formats its figures in another one.
@Test func theSameLinesInFrenchReadTheFrenchCatalogueAndTheFrenchNumberFormat() {
    LanguagePin.with("fr") {
        let n = { (v: Int) in LanguagePin.number(v, "fr") }
        #expect(n(1_240) != "1,240" && n(1_240).contains(",") == false)   // control: the French format is not the English one
        #expect(IndexProgress.statusLinePart(status(indexed: 1_200, queued: 10_800)) == "indexation 10 %")
        #expect(IndexProgress.statusLinePart(status(indexed: 6_400, queued: 5_600, paused: true)) == "indexation en pause")
        #expect(IndexProgress.settingsLine(status(indexed: 1_240, queued: 10_760))?.text == "Indexation de vos e-mails : \(n(1_240)) sur \(n(12_000))")
        #expect(IndexProgress.settingsLine(status(indexed: 12_000))?.text == "\(n(12_000)) e-mails indexés.")
        #expect(IndexProgress.settingsLine(status(indexed: 1))?.text == "1 e-mail indexé.")
    }
}

@Test func zeroTotalGivesNoLineAndNoShare() {
    #expect(IndexProgress.settingsLine(status()) == nil)
    #expect(IndexProgress.fraction(status()) == nil)
    #expect(IndexProgress.total(status()) == 0)
}

@MainActor @Test func theModelTakesAnAnnouncementAndOnlyThen() async throws {
    let center = NotificationCenter()
    let model = IndexProgressModel(center: center)
    #expect(model.status == MeaningIndexBuilder.Status())
    // no announcement: nothing moves
    try await Task.sleep(nanoseconds: 100_000_000)
    #expect(model.status == MeaningIndexBuilder.Status())
    // an announcement carrying figures: the model holds them
    let figures = status(indexed: 100, queued: 900)
    center.post(name: IndexProgress.didChange, object: nil, userInfo: [IndexProgress.statusKey: figures])
    for _ in 0..<200 where model.status != figures { try await Task.sleep(nanoseconds: 10_000_000) }
    #expect(model.status == figures)
    // a notification of the same name without figures changes nothing
    center.post(name: IndexProgress.didChange, object: nil)
    try await Task.sleep(nanoseconds: 100_000_000)
    #expect(model.status == figures)
}
