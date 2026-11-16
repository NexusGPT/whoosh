import Foundation

/// Drive seam, fixture core only: mails that carry the engine's prepared fields (contract delta 26) the fixture corpus does not produce.
/// `mac-main.reader.body.sample.<trackers|degraded|twodocs|folded>`. Kept apart from MessageBodySamples.swift (another lane moves those mails).
enum MessageBodyEngineSamples {
    typealias Prepared = MessageBodyContent.PreparedBody

    static func make(_ name: String) -> (MessageBodyContent, String)? {
        switch name {
        case "trackers": return (trackers, "news@shop.example")
        case "degraded": return (degraded, "big@sender.example")
        case "twodocs": return (twoDocs, "claire.martin@lorvan-hotels.example")
        case "folded": return (folded, "claire.martin@lorvan-hotels.example")
        case "signature": return (signature, "robin.vale@workshop.example")
        case "newsletter": return (newsletter, "letters@garden.example")
        case "trackinglinks": return (trackingLinks, "digest@news.example")
        default: return nil
        }
    }

    /// Two tracking pixels and no picture: the line says "2 trackers blocked" and offers no Load once.
    static var trackers: MessageBodyContent {
        let html = "<p>Hello Assem, your statement is ready.</p><img src=\"cid:whoosh-blocked\" width=\"1\" height=\"1\"><img src=\"cid:whoosh-blocked\" width=\"1\" height=\"1\">"
        return .init(id: "sample-trackers", safeHtml: html, plainText: BodyText.plain(fromHTML: html), trackersBlocked: 2, quoteOffset: nil,
                     prepared: Prepared(layout: .written, displayHtml: html, writtenHtml: html, segmentHtml: [html], foldFrom: nil, warnings: [], pictures: 0, pixels: 2))
    }

    /// The engine degraded the mail: its text is shown as plain text with one quiet line.
    static var degraded: MessageBodyContent {
        let html = "<div class=\"whoosh-body\"><pre>Too large to read as a designed mail; the text is kept.</pre></div>"
        return .init(id: "sample-degraded", safeHtml: html, plainText: BodyText.plain(fromHTML: html), trackersBlocked: 0, quoteOffset: nil,
                     prepared: Prepared(layout: .written, displayHtml: html, writtenHtml: html, segmentHtml: [html], foldFrom: nil,
                                        warnings: ["HTML nested deeper than the cap: shown as escaped text"], pictures: 0, pixels: 0))
    }

    /// A written document and a designed one for the same mail: the app's colours draw the first, the sender's the second; ⇧I switches.
    static var twoDocs: MessageBodyContent {
        let written = "<div class=\"whoosh-body\"><p>Hi Assem, the quote is attached.</p></div>"
        let designed = "<div class=\"whoosh-body\" style=\"background-color:#f3efe6\"><table><tr><td bgcolor=\"#ff7900\">Hi Assem, the quote is attached.</td></tr></table></div>"
        return .init(id: "sample-twodocs", safeHtml: designed, plainText: BodyText.plain(fromHTML: written), trackersBlocked: 0, quoteOffset: nil,
                     prepared: Prepared(layout: .written, displayHtml: designed, writtenHtml: written, segmentHtml: [], foldFrom: nil, warnings: [], pictures: 0, pixels: 0))
    }

    private static func blocked(_ w: Int, _ h: Int) -> String { "<img src=\"cid:whoosh-blocked\" width=\"\(w)\" height=\"\(h)\">" }

    /// A short mail a person wrote, its signature a small layout table with pictures (one 100 px portrait, three 24 px icons, four 16 px marks), all blocked: the
    /// engine decides WRITTEN (the corpus case `written-with-picture-signature.case`), so the app's colours draw it; the placeholders keep their declared size.
    static var signature: MessageBodyContent {
        let text = "<p>Thanks for showing us around the workshop on Tuesday.</p><p>The team liked the new layout of the bench area.</p><p>Could you send the revised plan when it is ready?</p>"
            + "<p>We would like to book the next visit before the end of the month.</p><p>Please tell Sam that the samples arrived safely.</p><p>Speak soon.</p>"
        let sig = "<p>--</p><table><tr><td>\(blocked(100, 100))</td><td><b>Robin Vale</b><br>Workshop host<br>\(blocked(24, 24)) \(blocked(24, 24)) \(blocked(24, 24)) \(blocked(16, 16)) \(blocked(16, 16)) \(blocked(16, 16)) \(blocked(16, 16))</td></tr></table>"
        let html = "<div class=\"whoosh-body\">" + text + sig + "</div>"
        return .init(id: "sample-signature", safeHtml: html, plainText: BodyText.plain(fromHTML: html), trackersBlocked: 0, quoteOffset: nil,
                     prepared: Prepared(layout: .written, displayHtml: html, writtenHtml: html, segmentHtml: [html], foldFrom: nil, warnings: [], pictures: 8, pixels: 0))
    }

    /// The two long tracking addresses of a plain text mail (invented, never a real address), and a third in plain text.
    static let trackingStory = "https://click.news-digest.example/track/L2g5a3Jhd2xlci9yZWRpcmVjdC9hYmNkZWZnaGlqa2xtbm9wcXJzdHV2d3h5ejAxMjM0NTY3ODk/redirect?u=aHR0cHM6Ly93d3cubmV3cy1kaWdlc3QuZXhhbXBsZS9zdG9yeS8xMjM0NQ&c=0a1b2c3d4e5f"
    static let trackingPrefs = "https://click.news-digest.example/prefs/MDEyMzQ1Njc4OWFiY2RlZmdoaWprbG1ub3BxcnN0dXZ3eHl6/manage?t=ZGlnZXN0LXdlZWtseQ&sig=9f8e7d6c5b4a3210fedcba9876543210"
    static let trackingPlain = "https://www.tmcnet.example/articles/2026/11/13/the-week-in-print/index.html?utm_source=digest&utm_medium=email&utm_campaign=weekly&id=88231977"

    /// A plain text mail whose body carries two long tracking addresses in angle brackets after their own words and one in the open: each is ONE short link (the words, or the
    /// site's name and "link"), never ten lines of letters; the full address is the link's target and tooltip.
    static var trackingLinks: MessageBodyContent {
        let text = "Hello Sam,\n\nYour weekly digest is ready. Read the full story <\(trackingStory)>\n\nTo adjust your preferences <\(trackingPrefs)>\n\nAlso this week: \(trackingPlain)\n\nSee you next week."
        return .plain(id: "sample-trackinglinks", text: text)
    }

    /// A designed newsletter beside it: a coloured wrapper, a 600 px card, a hero picture and two more; it keeps its white paper (with the page's inner margin).
    static var newsletter: MessageBodyContent {
        let html = "<div class=\"whoosh-body\" style=\"background-color:#f3efe6\"><table width=\"100%\" style=\"background-color:#f3efe6\"><tr><td align=\"center\"><table width=\"600\" style=\"background-color:#ffffff\">"
            + "<tr><td>\(blocked(600, 200))</td></tr><tr><td><h1>The Garden Letter</h1><p>Plant your tulip bulbs now for a spring full of colour.</p></td></tr>"
            + "<tr><td>\(blocked(280, 180)) \(blocked(280, 180))</td></tr><tr><td><p>Autumn workshop on pruning opens for booking this week.</p></td></tr></table></td></tr></table></div>"
        return .init(id: "sample-newsletter", safeHtml: html, plainText: BodyText.plain(fromHTML: html), trackersBlocked: 0, quoteOffset: nil,
                     prepared: Prepared(layout: .designed, displayHtml: html, writtenHtml: nil, segmentHtml: [html], foldFrom: nil, warnings: [], pictures: 3, pixels: 0))
    }

    /// Engine segments: the answer, an inline quote it depends on, authored text after it (all before `fold_from`), then history, signature and
    /// a legal footer behind the dots. The old `quote_offset` is deliberately wrong here: the fold must follow `fold_from`.
    static var folded: MessageBodyContent {
        let segs = ["<p>Hi Assem,</p>", "<blockquote>Can you confirm the price?</blockquote>", "<p>Yes: 18,400 EUR.</p>",
                    "<blockquote>Earlier message from Assem</blockquote>", "<p>Claire Martin, Head of Purchasing</p>", "<p>Confidential.</p>"]
        let all = segs.joined()
        return .init(id: "sample-folded", safeHtml: all, plainText: BodyText.plain(fromHTML: all), trackersBlocked: 0, quoteOffset: 5,
                     prepared: Prepared(layout: .written, displayHtml: all, writtenHtml: all, segmentHtml: segs, foldFrom: 3, warnings: [], pictures: 0, pixels: 0))
    }
}
