import Foundation

/// Drive seam, fixture core only (no control draws it): puts one of the three mails of the owner's drawings in the shown body's place,
/// so a hidden render can reach each state without a corpus that carries raw HTML (the fixture serves paragraphs only).
/// `mac-main.reader.body.sample.<designed|written|plain>`. The mail on screen is replaced in the view only; nothing is stored.
/// The three mails are DATA, read from the design fixture's `sample_mails` (docs/design/fixtures/mac-main-sample.json, found as FixtureFile.locate()
/// finds it): the designed mail carries its own bgcolor and color, which are a mail's colours and not design colours, so no hex literal sits in this
/// feature file (token-check, round-fixes item 1). A fixture without the key, or an unreadable one, serves no sample (`make` answers nil).
enum MessageBodySamples {
    /// `wide` (A146): a newsletter on a 700 px fixed table, wider than the reading pane, plain colours and no remote picture.
    static let ids = ["designed", "written", "plain", "wide"]

    /// One sample as the fixture writes it: `html` (sanitised mail HTML) or `text` (plain), the sender address, and for html the blocked-picture
    /// count and the UTF-8 length of the part above the quoted earlier message.
    private struct Sample: Decodable {
        let sender: String
        let html: String?
        let text: String?
        let trackers_blocked: Int?
        let quote_offset: Int?
    }
    private struct File: Decodable { let sample_mails: [String: Sample]? }

    private static let samples: [String: Sample] = {
        guard let url = FixtureFile.locate(), let data = try? Data(contentsOf: url), let f = try? JSONDecoder().decode(File.self, from: data) else { return [:] }
        return f.sample_mails ?? [:]
    }()

    /// (content, sender address) for a sample name; nil for an unknown name or a sample the fixture lacks or leaves empty.
    static func make(_ name: String) -> (MessageBodyContent, String)? {
        guard ids.contains(name), let s = samples[name], !s.sender.isEmpty else { return nil }
        if name == "plain" {
            guard let text = s.text, !text.isEmpty else { return nil }
            return (.plain(id: "sample-plain", text: text), s.sender)
        }
        guard let html = s.html, !html.isEmpty else { return nil }
        return (.sanitised(id: "sample-\(name)", html: html, trackersBlocked: s.trackers_blocked ?? 0, quoteOffset: s.quote_offset), s.sender)
    }
}
