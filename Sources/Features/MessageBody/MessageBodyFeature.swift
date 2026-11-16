import Foundation

/// The body view the reader embeds. Registers the probe regions the checks read (docs/CONTROL-SOCKET.md `rows`), the click doors
/// of its controls and its three keys (A129): O the three dots, I Load once, ⇧I the other colours.
public enum MessageBodyFeature: FeatureModule {
    public static let id = "messagebody"
    public static let surfaces: [String] = ["mac-main"]

    public static func rowSources() -> [String: (AppModel) throws -> [ControlRow]] {
        [
            "mac-main.reader.body": { _ in
                guard let m = MessageBodyProbe.shared.current else { throw ControlFailure("no message body is on screen") }
                let doc = MessageBodyDocument.html(for: m.content, quotedOpen: m.quotedOpen, scheme: .light, colours: m.startColours, loadPictures: m.picturesAsked)
                return [[
                    "paragraphs": m.content.visibleParagraphs(quotedOpen: m.quotedOpen),
                    "has_quote": m.content.hasQuote,
                    "quoted_folded": !m.quotedOpen,
                    "javascript_allowed": m.javaScriptAllowed as Any? ?? NSNull(),
                    "remote_blocked": m.blockListInstalled,
                    "format": m.content.safeHtml == nil ? "plain" : "html",
                    // A129: B3 (layout, colours, the sheet), F2 (the dots and what they hold), part 5 (blocks and links the page draws)
                    "layout": m.content.layout.rawValue,
                    "layout_source": m.content.layoutSource.rawValue,
                    "degraded": m.content.degraded,
                    "fold_source": m.content.prepared == nil ? "quote_offset" : "segments",
                    "folded_text": m.content.foldSplit.map { BodyText.paragraphs(BodyText.plain(fromHTML: $0.folded)) } ?? [],
                    "documents": !m.content.hasQuote && m.content.prepared?.writtenHtml != nil && m.content.prepared?.displayHtml != nil && m.content.prepared?.writtenHtml != m.content.prepared?.displayHtml ? 2 : 1,
                    "drawn_document": m.colours == .app && (m.content.prepared?.writtenHtml != nil) ? "written" : "designed",
                    "colours": m.colours.rawValue,
                    "drawn": m.drawn,   // what the page itself computed (a test copy only): class, pad_left, tile_fill, app_document, sender_document, page_fill
                    "sheet": m.colours == .original && m.content.safeHtml != nil,
                    "painted": m.painted,
                    "dots": m.content.hasQuote,
                    "fold_open": m.quotedOpen,
                    "dots_key": DotsPill.key ?? "",
                    "colours_key": "⇧I",
                    "colours_action": m.colours == .original ? MessageBodyWords.appColours : MessageBodyWords.originalColours,   // the foot strip's word: what ⇧I does now
                    "colours_applies": m.content.safeHtml != nil,
                    "text_above_y": Double(m.mainBottom),
                    // A146, the fit as the last measure read it: the content's natural width and the page's width (points, 0 before a
                    // measure answered), the scale shown, the width it is shown at, the owner's pinch, whether the card scrolls sideways
                    "natural_width": Double(m.naturalWidth),
                    "view_width": Double(m.paneWidth),
                    "scale": Double(m.scale),
                    "fit_scale": Double(m.fitScale),
                    "scaled_width": Double(m.naturalWidth * m.scale),
                    "zoom": Double(m.zoom),
                    "scrolls_horizontally": m.scrollsHorizontally,
                    "height_reserved": Double(m.height),
                    "blocks": count(doc, ["<p class=\"pt\">", "<blockquote"]),
                    "links": count(doc, ["<a href=\""]),
                    "quote_blocks": count(doc, ["<blockquote"]),
                ]]
            },
            // The view's height on screen beside the height the model reserved, read WITHOUT asking for a measure (the row below asks for one at each read,
            // which is how a probe made the height unfold): the check of "a mail opens at its full height at once" samples this.
            "mac-main.reader.body.frame": { _ in
                guard let m = MessageBodyProbe.shared.current else { throw ControlFailure("no message body is on screen") }
                let f = m.webFacts()
                return [["id": m.content.id, "view_height": (f["height"] as? Double) ?? -1, "reserved": Double(m.height), "measured": m.measured, "page_loaded": m.pageLoaded]]
            },
            // What the web view and its page say (A129d, the blank body): read it twice, the first read asks the page to measure again.
            "mac-main.reader.web": { _ in
                guard let m = MessageBodyProbe.shared.current else { throw ControlFailure("no message body is on screen") }
                m.refresh()
                var row: ControlRow = m.webFacts()
                row["painted"] = m.painted; row["text_length"] = m.textLength; row["measure_answered"] = m.measureAnswered
                row["height_reserved"] = Double(m.height); for (k, v) in m.stats { row[k] = v }
                return [row]
            },
            // The blocked pictures as the page will draw them: one row each, read from the document itself (never a screenshot): the size it declared, the size the box is given,
            // whether it is the small mark with no words, and whether it carries words.
            "mac-main.reader.body.placeholders": { _ in
                guard let m = MessageBodyProbe.shared.current else { throw ControlFailure("no message body is on screen") }
                let doc = MessageBodyDocument.html(for: m.content, quotedOpen: false, scheme: .light, colours: m.startColours, loadPictures: false)
                // The tag first, then its size from its own attributes. One pattern with the style as an optional group in the middle never read a size: the
                // lazy run before the group matched nothing, the group was skipped and the rest of the tag took the style (lead, main117: both size cases read
                // width and height null while the page drew the declared sizes).
                let re = try NSRegularExpression(pattern: "<span class=\"blocked( mark)?\"([^>]*)>([^<]*)</span>")
                let size = try NSRegularExpression(pattern: "style=\"width:(\\d+)px(?:;height:(\\d+)px)?")
                return re.matches(in: doc, range: NSRange(doc.startIndex..., in: doc)).enumerated().map { i, x in
                    func s(_ n: Int) -> String? { Range(x.range(at: n), in: doc).map { String(doc[$0]) } }
                    let attrs = s(2) ?? ""
                    let m = size.firstMatch(in: attrs, range: NSRange(attrs.startIndex..., in: attrs))
                    func n(_ g: Int) -> Int? { m.flatMap { Range($0.range(at: g), in: attrs) }.flatMap { Int(attrs[$0]) } }
                    return ["ax": "mac-main.reader.body.placeholder.\(i)", "mark": s(1) != nil, "width": n(1).map { $0 as Any } ?? NSNull(), "height": n(2).map { $0 as Any } ?? NSNull(),
                            "words": s(3) ?? ""]
                }
            },
            // The links as the page draws them: the text drawn, the address it opens (`href`) and the tooltip (`title`), read from the document itself.
            "mac-main.reader.body.links": { _ in
                guard let m = MessageBodyProbe.shared.current else { throw ControlFailure("no message body is on screen") }
                let doc = MessageBodyDocument.html(for: m.content, quotedOpen: false, scheme: .light, colours: m.startColours, loadPictures: false)
                let re = try NSRegularExpression(pattern: "<a href=\"([^\"]*)\"(?: title=\"([^\"]*)\")?>([^<]*)</a>")
                func plain(_ s: String) -> String { s.replacingOccurrences(of: "&amp;", with: "&").replacingOccurrences(of: "&quot;", with: "\"").replacingOccurrences(of: "&lt;", with: "<").replacingOccurrences(of: "&gt;", with: ">") }
                return re.matches(in: doc, range: NSRange(doc.startIndex..., in: doc)).enumerated().map { i, x in
                    func s(_ n: Int) -> String { Range(x.range(at: n), in: doc).map { plain(String(doc[$0])) } ?? "" }
                    return ["ax": "mac-main.reader.body.link.\(i)", "href": s(1), "title": s(2), "text": s(3)]
                }
            },
            "mac-main.reader.trackers": { _ in
                guard let m = MessageBodyProbe.shared.current else { throw ControlFailure("no message body is on screen") }
                return [["count": m.content.trackersBlocked, "label": m.content.blockedLabel ?? "", "explain_open": m.explainOpen]]
            },
            // The pictures line: a row only while something was blocked. Nothing loads before the owner asks.
            "mac-main.reader.pictures": { _ in
                guard let m = MessageBodyProbe.shared.current else { throw ControlFailure("no message body is on screen") }
                guard let line = m.content.blockedLabel else { return [] }
                return [[
                    "line": m.picturesSentence ?? "", "line_shown": m.picturesSentence != nil, "first_line": line, "pictures": m.content.blockedPictures, "trackers": m.content.pixelTrackers,
                    // the rule: pictures load when a mail opens (`by_rule`), never for a mail read in Junk; `askable` pictures kept an address; the note under the line
                    "by_rule": m.picturesAsked && !m.picturesByOwner, "quarantined": m.quarantined, "askable": m.askable, "rule_on": PicturesPolicy.loadsOnOpen, "note": m.picturesNote,
                    // after the ask: blocked, loading, loaded or missing; how many pictures are not on screen; the load again and the page loads so far
                    "state": !m.picturesAsked ? "blocked" : (m.picturesMissing.map { $0 > 0 ? "missing" : "loaded" } ?? "loading"),
                    "missing": m.picturesMissing as Any? ?? NSNull(), "retry_shown": m.picturesAsked && (m.picturesRetryable ?? 0) > 0, "loads": m.stats["loads"] ?? 0,
                    "loaded": m.picturesAsked, "always": PicturesAlways.contains(m.sender), "sender": m.sender,
                    "load_once_key": "I", "load_once_shown": !m.picturesAsked && m.content.blockedPictures > 0, "always_shown": !m.sender.isEmpty && ((!m.picturesAsked && m.content.blockedPictures > 0) || (m.picturesByOwner && m.picturesMissing == 0 && !m.alwaysOn)),
                    "fetchable": count(MessageBodyDocument.html(for: m.content, quotedOpen: false, scheme: .light, colours: m.startColours, loadPictures: m.picturesAsked), ["<img src=\"https://"]),   // pictures the page now asks the sender's server for: none before the click, never a pixel
                    "list": m.picturesAsked ? "pictures" : "none", "remote_blocked": m.blockListInstalled,
                    "javascript_allowed": m.javaScriptAllowed as Any? ?? NSNull(),
                ]]
            },
        ]
    }

    public static func clickTargets() -> [String: (AppModel) -> Bool] {
        let fold: (AppModel) -> Bool = { _ in MessageBodyProbe.shared.current?.toggleFold() ?? false }
        return [
            // The three-dot pill (key O); the older door of the "Show quoted text" button answers the same.
            "mac-main.reader.body.dots": fold,
            "mac-main.reader.body.show-quoted": fold,
            // The other colours (key ⇧I): its door for the drive; the screen shows it in the foot strip.
            "mac-main.reader.body.colours": { _ in MessageBodyProbe.shared.current?.switchColours() ?? false },
            // The pictures line: its sentence opens the one-line note; Load once (key I); Always for the sender.
            "mac-main.reader.trackers.button": { _ in
                guard let m = MessageBodyProbe.shared.current, m.content.blockedLabel != nil else { return false }
                m.explainOpen.toggle()
                return true
            },
            "mac-main.reader.pictures.load-once": { _ in MessageBodyProbe.shared.current?.loadOnce() ?? false },
            "mac-main.reader.pictures.always": { _ in MessageBodyProbe.shared.current?.loadAlways() ?? false },
            "mac-main.reader.pictures.retry": { _ in MessageBodyProbe.shared.current?.retryPictures() ?? false },
            // Drive seam, fixture core only: one of the drawings' mails in the shown body's place (MessageBodySamples).
            "mac-main.reader.body.sample.designed": { sample("designed", $0) },
            "mac-main.reader.body.sample.designed-junk": { sample("designed", $0, quarantined: true) },   // the same mail as read in Junk: its pictures ask first
            "mac-main.reader.body.sample.written": { sample("written", $0) },
            "mac-main.reader.body.sample.plain": { sample("plain", $0) },
            "mac-main.reader.body.sample.trackers": { sample("trackers", $0) },
            "mac-main.reader.body.sample.degraded": { sample("degraded", $0) },
            "mac-main.reader.body.sample.twodocs": { sample("twodocs", $0) },
            "mac-main.reader.body.sample.folded": { sample("folded", $0) },
            "mac-main.reader.body.sample.wide": { sample("wide", $0) },
            "mac-main.reader.body.sample.signature": { sample("signature", $0) },
            "mac-main.reader.body.sample.newsletter": { sample("newsletter", $0) },
            "mac-main.reader.body.sample.trackinglinks": { sample("trackinglinks", $0) },
            // Drive seam, no control: forgets the shown sender so a drive leaves no Always behind.
            "mac-main.reader.pictures.always.forget": { _ in
                guard let m = MessageBodyProbe.shared.current, PicturesAlways.contains(m.sender) else { return false }
                PicturesAlways.remove(m.sender)
                m.syncAlways()
                return true
            },
        ]
    }

    private static func sample(_ name: String, _ m: AppModel, quarantined: Bool = false) -> Bool {
        guard m.core is FixtureCore, let model = MessageBodyProbe.shared.current, let (content, sender) = MessageBodySamples.make(name) ?? MessageBodyEngineSamples.make(name) else { return false }
        model.adopt(name == "designed" || name == "wide" ? content.withLayout(.designed) : content, sender: sender, quarantined: quarantined)
        return true
    }

    /// Drive seam, fixture core only (`fixture mail-picture <address>`): a designed mail with ONE blocked picture kept at `address` and one tracking pixel kept
    /// beside it (`<address>-pixel`), in the shown body's place. The address must be on this Mac: a check serves the picture from its own local server and
    /// reads that server's request log, so the whole load path is proven by a real fetch that never leaves this Mac.
    private static var pictureSamples = 0
    static func pictureSample(_ address: String, _ m: AppModel) -> Bool {
        guard m.core is FixtureCore, let model = MessageBodyProbe.shared.current, let u = URL(string: address), RigNet.isThisMac(u.host) else { return false }
        pictureSamples += 1
        let a = MessageBodyDocument.escape(address)
        let html = "<p>A picture served from this Mac.</p><img src=\"cid:whoosh-blocked\" data-whoosh-src=\"\(a)\" width=\"120\" height=\"40\">"
            + "<img src=\"cid:whoosh-blocked\" data-whoosh-src=\"\(a)-pixel\" width=\"1\" height=\"1\">"
        let prepared = MessageBodyContent.PreparedBody(layout: .designed, displayHtml: html, writtenHtml: nil, segmentHtml: [], foldFrom: nil, warnings: [], pictures: 1, pixels: 1)
        let content = MessageBodyContent(id: "sample-picture-\(pictureSamples)", safeHtml: html, plainText: "A picture served from this Mac.", trackersBlocked: 2, quoteOffset: nil, prepared: prepared)
        model.adopt(content, sender: "pictures@this-mac.example", quarantined: false)
        return true
    }

    /// I (Load once). The three dots follow the preset's `expand-message` key through `ReaderKeys.expand`, and ⇧I is the command `toggle-colours`: no literal O here.
    public static func keyBindings() -> [String: (AppModel) -> Bool] {
        [
            // I: Load once; after pictures did not come, the same key tries them again
            "I": { m in !QuickReplyModel.shared.isOpen && ((MessageBodyProbe.shared.current?.loadOnce() ?? false) || (MessageBodyProbe.shared.current?.retryPictures() ?? false)) },
        ]
    }

    /// The reader state: a mail is open (`surface == mac-reader`: Return and a click on a row open it). NOT the pane focus: `focusedPane` is set only by a
    /// click or Tab and keeps a stale `.reader` after the thread closes, so on the built app (main77) ⇧I in the list still switched colours, and after Return
    /// (pane still `.list`) it did not (A129d, A129e). The list state is every other surface. This is the ONE answer the foot strip (`MacFootHints.surface`), ⇧I, the colours hint and `ReaderKeys.isOpen` (O) read.
    static func readerHoldsKeys(_ m: AppModel) -> Bool { m.surface == ReaderState.surface }

    /// The colour switch (command `toggle-colours`, context key ⇧I): only while the READER pane has the focus; in the list ⇧I stays the
    /// "important" filter (the handler answers false and the key falls to its row).
    static func switchColours(_ m: AppModel) -> Bool {
        readerHoldsKeys(m) && !QuickReplyModel.shared.isOpen && (MessageBodyProbe.shared.current?.switchColours() ?? false)
    }

    /// The foot strip's word for the switch, nil unless it applies now (reader pane focused, a designed mail: a written one has no colours of its own to show; the key still works on it).
    static func coloursHint() -> String? {
        guard readerHoldsKeys(AppModel.shared), let b = MessageBodyProbe.shared.current, b.content.safeHtml != nil, b.content.layout == .designed else { return nil }
        return b.colours == .original ? MessageBodyWords.appColours : MessageBodyWords.originalColours
    }

    private static func count(_ s: String, _ needles: [String]) -> Int {
        needles.reduce(0) { $0 + (s.components(separatedBy: $1).count - 1) }
    }
}

/// The body view on screen now, for the socket probe.
@MainActor
final class MessageBodyProbe {
    static let shared = MessageBodyProbe()
    private(set) weak var current: MessageBodyModel?
    func show(_ model: MessageBodyModel) { current = model }
}

extension MessageBodyFeature: CommandProviding {
    public static func commands() -> [String: (AppModel) -> Bool] { ["toggle-colours": { switchColours($0) }] }
}
