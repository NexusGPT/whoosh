import Foundation
import Testing
@testable import WhooshUI

// The three sample mails of the drive seam (mac-main.reader.body.sample.designed|written|plain) are read from the design fixture's
// "sample_mails" (round-fixes item 1: no hex literal in feature Swift). These fail when a sample is missing or empty.
// MUTANTS: each test names, in its trailing comment, the mutation that must turn it red.

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()

private func samplesFixture() { setenv("WHOOSH_FIXTURE_PATH", repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1) }

@Test func everySampleMailIsServedAndNotEmpty() throws {
    samplesFixture()
    for name in MessageBodySamples.ids {
        let (content, sender) = try #require(MessageBodySamples.make(name), "\(name) is not served")   // mutant: a sample removed from the fixture
        #expect(!sender.isEmpty, "\(name) has no sender")
        #expect(!(content.safeHtml ?? content.plainText).trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, "\(name) is empty")   // mutant: a sample with empty html or text
    }
    #expect(MessageBodySamples.make("nobody") == nil)   // mutant: an unknown name served
}

@Test func theSamplesKeepTheirHtmlAndTheirFigures() throws {
    samplesFixture()
    let designed = try #require(MessageBodySamples.make("designed")).0
    #expect(designed.safeHtml?.contains("bgcolor=") == true && designed.trackersBlocked == 3 && designed.quoteOffset == nil)   // mutant: the designed mail loses its own colours or its three pictures
    let written = try #require(MessageBodySamples.make("written")).0
    let html = try #require(written.safeHtml)
    let offset = try #require(written.quoteOffset)
    #expect(html.utf8.count > offset && offset > 0 && html.contains("gmail_quote"))   // mutant: the quote offset dropped
    let plain = try #require(MessageBodySamples.make("plain")).0
    #expect(plain.safeHtml == nil && plain.plainText.contains("https://example.com/delivery"))   // mutant: the plain mail served as html
}
