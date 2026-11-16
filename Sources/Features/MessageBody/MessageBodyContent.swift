import Foundation

/// What the body view shows: the core's sanitised HTML (or plain text when the message has none),
/// the count of remote images the sanitiser replaced, and where quoted text starts.
/// The view never receives raw mail: `safeHtml` is the output of the parse crate's `sanitise_html`
/// (core/crates/parse/src/sanitise.rs), whose `quote_offset` is a UTF-8 byte offset into that string.
public struct MessageBodyContent: Equatable, Sendable {
    public let id: String
    public let safeHtml: String?
    public let plainText: String
    public let trackersBlocked: Int
    /// UTF-8 byte offset into `safeHtml` (or into `plainText` when there is no HTML) where quoted text begins.
    public let quoteOffset: Int?
    /// designed or written (B3), from the engine; a body with no prepared fields is WRITTEN and says so (`layoutSource`).
    public let layout: MailLayout
    public let layoutSource: LayoutSource
    /// What the engine prepared once at body fetch (contract delta 26); nil for a body that has none (an old stored mail).
    public let prepared: PreparedBody?
    /// Pictures not loaded and, apart from them, the tracking pixels: the engine's own counts when it prepared the body.
    public let blockedPictures: Int
    public let pixelTrackers: Int

    public enum LayoutSource: String, Equatable, Sendable { case engine, fallback }

    /// The engine's prepared fields as the view takes them (`MessageBody.segments`, `foldFrom`, `layout`, `displayHtml`, `writtenHtml`,
    /// `bodyWarnings`, `picturesBlocked`, `pixelTrackers`).
    public struct PreparedBody: Equatable, Sendable {
        public var layout: MailLayout?
        public var displayHtml: String?
        public var writtenHtml: String?
        /// Each segment's sanitised HTML, in reading order.
        public var segmentHtml: [String]
        public var foldFrom: Int?
        public var warnings: [String]
        public var pictures: Int
        public var pixels: Int
        public init(layout: MailLayout?, displayHtml: String?, writtenHtml: String?, segmentHtml: [String], foldFrom: Int?, warnings: [String], pictures: Int, pixels: Int) {
            self.layout = layout; self.displayHtml = displayHtml; self.writtenHtml = writtenHtml; self.segmentHtml = segmentHtml
            self.foldFrom = foldFrom; self.warnings = warnings; self.pictures = pictures; self.pixels = pixels
        }
    }

    public init(id: String, safeHtml: String?, plainText: String, trackersBlocked: Int, quoteOffset: Int?, prepared: PreparedBody? = nil) {
        self.id = id
        let html = safeHtml?.trimmingCharacters(in: .whitespacesAndNewlines)
        self.safeHtml = (html?.isEmpty ?? true) ? nil : safeHtml
        self.plainText = plainText
        self.trackersBlocked = trackersBlocked
        self.quoteOffset = quoteOffset
        self.prepared = prepared
        self.layout = prepared?.layout ?? .written
        self.layoutSource = prepared?.layout == nil ? .fallback : .engine
        // The engine's counts; when it prepared the body but counted nothing while the old field counts images (the fixture core), the tag scan stands in.
        if let p = prepared, p.pictures > 0 || p.pixels > 0 || trackersBlocked == 0 {
            self.blockedPictures = p.pictures
            self.pixelTrackers = p.pixels
        } else {
            let seen = MessageBodyPictures.counts(html: prepared?.displayHtml ?? self.safeHtml)
            self.blockedPictures = max(seen.blocked, trackersBlocked)
            self.pixelTrackers = seen.pixels
        }
    }

    /// The same mail with the engine's layout set (the drive seam's designed sample).
    public func withLayout(_ l: MailLayout) -> Self {
        let p = PreparedBody(layout: l, displayHtml: safeHtml, writtenHtml: nil, segmentHtml: [], foldFrom: nil, warnings: [],
                             pictures: blockedPictures, pixels: pixelTrackers)
        return .init(id: id, safeHtml: safeHtml, plainText: plainText, trackersBlocked: trackersBlocked, quoteOffset: nil, prepared: p)
    }

    /// The document drawn for these colours: the app's take the engine's written document (sender fonts and colours removed), the original ones
    /// the designed document; a missing one falls back to the other, then to the old sanitised HTML.
    func document(for colours: BodyColours) -> String? {
        colours == .app ? (prepared?.writtenHtml ?? prepared?.displayHtml ?? safeHtml) : (prepared?.displayHtml ?? safeHtml)
    }

    /// The mail is HTML (any document); else plain text.
    var isHTML: Bool { safeHtml != nil || prepared?.displayHtml != nil || prepared?.writtenHtml != nil }

    /// Degraded by the engine (too large, nested too deep, stopped, not a message): the card says it shows plain text. A warning that leaves
    /// the page whole (a charset read tolerantly, styles not inlined) says nothing: the line would be false on a drawn page.
    public var degraded: Bool { prepared?.warnings.contains { $0.contains("escaped") || $0.contains("bytes") } ?? false }

    /// The core's sanitised HTML with its tracker count and quote offset.
    public static func sanitised(id: String, html: String, trackersBlocked: Int, quoteOffset: Int?) -> Self {
        .init(id: id, safeHtml: html, plainText: BodyText.plain(fromHTML: html), trackersBlocked: trackersBlocked, quoteOffset: quoteOffset)
    }

    /// A message that arrived without an HTML part, or whose HTML is empty after sanitising.
    public static func plain(id: String, text: String, trackersBlocked: Int = 0, quoteOffset: Int? = nil) -> Self {
        .init(id: id, safeHtml: nil, plainText: text, trackersBlocked: trackersBlocked, quoteOffset: quoteOffset)
    }

    /// Paragraphs as the design fixture writes a message body (docs/design/fixtures/mac-main-sample.json).
    public static func paragraphs(id: String, _ paragraphs: [String], trackersBlocked: Int = 0) -> Self {
        .plain(id: id, text: paragraphs.joined(separator: "\n\n"), trackersBlocked: trackersBlocked)
    }

    /// F2: everything that is not the sender's own text sits behind the one three-dot pill. With the engine's segments the cut is its
    /// `fold_from`: segments before it show, the rest folds. The engine's rule, not weakened here: an inline quote an answer depends on and any
    /// authored text after it are before `fold_from`, never behind the dots. A prepared body with no `fold_from` has nothing to fold. Only a body
    /// with no prepared fields uses the old single `quote_offset` split.
    var foldSplit: (main: String, folded: String)? {
        guard let p = prepared else { return quoteSplit.map { ($0.main, $0.quoted) } }
        guard let f = p.foldFrom, f > 0, f < p.segmentHtml.count else { return nil }
        return (p.segmentHtml[..<f].joined(), p.segmentHtml[f...].joined())
    }

    public var hasQuote: Bool { foldSplit != nil }

    /// The source string the quote offset points into.
    private var source: String { safeHtml ?? plainText }

    /// (main, quoted) when the offset lies inside the source and leaves visible text before it.
    var quoteSplit: (main: String, quoted: String)? {
        guard let off = quoteOffset, off > 0 else { return nil }
        let bytes = Array(source.utf8)
        guard off < bytes.count else { return nil }
        let main = String(decoding: bytes[..<off], as: UTF8.self)
        let quoted = String(decoding: bytes[off...], as: UTF8.self)
        let visible = safeHtml == nil ? main : BodyText.plain(fromHTML: main)
        return visible.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : (main, quoted)
    }

    /// The paragraphs on screen: the main text, plus the folded text only once the reader opened it.
    public func visibleParagraphs(quotedOpen: Bool) -> [String] {
        if let s = foldSplit {
            let parts = quotedOpen ? [s.main, s.folded] : [s.main]
            return parts.flatMap { BodyText.paragraphs(isHTML || prepared != nil ? BodyText.plain(fromHTML: $0) : $0) }
        }
        let whole = document(for: .app) ?? plainText
        return BodyText.paragraphs(isHTML ? BodyText.plain(fromHTML: whole) : whole)
    }

    /// The pictures line, "3 pictures not loaded, 2 trackers blocked", or nil when nothing was blocked.
    public var blockedLabel: String? {
        (blockedPictures > 0 || pixelTrackers > 0) ? MessageBodyWords.picturesLine(pictures: blockedPictures, trackers: pixelTrackers) : nil
    }
}

/// Tag stripping for the plain-text fallback and the paragraph probe. The input is sanitised HTML
/// (well-formed, no scripts), so a regex pass is enough; it mirrors `safe_html_to_text` in the parse crate.
enum BodyText {
    private static let block = try! NSRegularExpression(pattern: "</(?:p|div|tr|table|h[1-6]|li|ul|ol|blockquote|pre)>|<br\\s*/?>|<hr\\s*/?>", options: .caseInsensitive)
    private static let cell = try! NSRegularExpression(pattern: "</t[dh]>", options: .caseInsensitive)
    private static let tag = try! NSRegularExpression(pattern: "<[^>]*>")

    static func plain(fromHTML html: String) -> String {
        func sub(_ r: NSRegularExpression, _ s: String, _ with: String) -> String {
            r.stringByReplacingMatches(in: s, range: NSRange(s.startIndex..., in: s), withTemplate: with)
        }
        var s = sub(block, html, "\n")
        s = sub(cell, s, " ")
        s = sub(tag, s, "")
        for (e, c) in [("&nbsp;", " "), ("&lt;", "<"), ("&gt;", ">"), ("&quot;", "\""), ("&#39;", "'"), ("&amp;", "&")] {
            s = s.replacingOccurrences(of: e, with: c)
        }
        return s
    }

    static func paragraphs(_ text: String) -> [String] {
        text.components(separatedBy: .newlines)
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
    }
}
