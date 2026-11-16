import SwiftUI

/// Reading measure and contrast floor of the body (board: reader/conversation `.body{max-width:540px}`;
/// the 4.5 floor is WCAG AA for body text and the brief's dark-mode acceptance).
/// Proposed rows for docs/CONSTANTS.md (frozen for this lane, see docs/lanes/messagebody.md).
public enum MessageBodyMetrics {
    public static let measure: CGFloat = 540
    public static let minimumContrast: Double = 4.5
}

/// Builds the one HTML document the web view loads. Colours, type size and spacing come from the Native
/// Glass tokens (WhooshPalette, WhooshTypography, WhooshSpacing), resolved for the scheme asked for, so
/// the page never depends on the web view guessing the appearance.
enum MessageBodyDocument {
    /// `colours` is the way the document STARTS (the app's, or the sender's on a white sheet); ⇧I flips it afterwards with a class
    /// on `<html>`, no reload. `loadPictures` is the one document whose pictures the owner asked for (Load once, Always).
    /// `env` nil is this process; a test copy (`RigNet`) loads only pictures on this Mac.
    static func html(for content: MessageBodyContent, quotedOpen: Bool, scheme: ColorScheme, colours: BodyColours? = nil, loadPictures: Bool = false,
                     env: [String: String]? = nil) -> String {
        let (main, folded) = parts(content, loadPictures: loadPictures, env: env)
        // The folded block is always in the page, hidden: opening it is a style flip with no reload, so the text above never
        // repaints or moves (docs/MOTION.md). `quotedOpen` only sets its initial state. The gap before it is the room of the
        // three-dot pill (F2): it is in the page folded or open, so the pill and the text above stay where they are.
        let foldedBlock = folded.map { "<div class=\"dg\"></div><div class=\"q\" id=\"q\"\(quotedOpen ? "" : " hidden")>\($0)</div>" } ?? ""
        return wrap("<div class=\"m\" id=\"m\">\(main)</div>\(foldedBlock)", scheme: scheme, colours: colours ?? BodyColours.start(for: content.layout), loadPictures: loadPictures,
                    rig: RigNet.rig(env: env))
    }

    /// The same message as escaped text only: used when the block list cannot be installed.
    static func textOnly(for content: MessageBodyContent, scheme: ColorScheme) -> String {
        wrap("<div class=\"m\">\(plainBlocks(content.plainText))</div>", scheme: scheme, colours: .app, loadPictures: false)
    }

    /// (written text, folded part). Two documents (the app's and the sender's) go into the page together, one shown by the page's class, only when
    /// the engine gave both and the mail has no fold; a folded mail draws its segments in both ways.
    private static func parts(_ c: MessageBodyContent, loadPictures: Bool, env: [String: String]?) -> (String, String?) {
        let ph = { placeholderImages($0, loadPictures: loadPictures, env: env) }
        if let split = c.foldSplit {
            return c.isHTML || c.prepared != nil ? (ph(split.main), ph(split.folded)) : (plainBlocks(split.main), plainBlocks(split.folded))
        }
        guard c.isHTML else { return (plainBlocks(c.plainText), nil) }
        let app = c.document(for: .app) ?? "", orig = c.document(for: .original) ?? ""
        if app == orig { return (ph(app), nil) }
        return ("<div class=\"ad\">\(ph(app))</div><div class=\"od\">\(ph(orig))</div>", nil)
    }

    /// Every image becomes a quiet box: the blocked ones say so, inline `cid:` ones are not fetched here. With `loadPictures`, a blocked
    /// image that keeps its original https address (`data-whoosh-src`, kept by core/crates/parse/src/sanitise.rs:11) is drawn from it. On a
    /// test copy (`RigNet`, `env` nil: this process) only an address on this Mac is drawn; any other stays a "Picture not loaded" box.
    static func placeholderImages(_ html: String, loadPictures: Bool = false, env: [String: String]? = nil) -> String {
        guard let img = try? NSRegularExpression(pattern: "<img\\b[^>]*>", options: .caseInsensitive) else { return html }
        var out = html
        for m in img.matches(in: html, range: NSRange(html.startIndex..., in: html)).reversed() {
            guard let r = Range(m.range, in: out) else { continue }
            let tag = String(out[r])
            let blocked = tag.contains("cid:whoosh-blocked")
            if blocked, loadPictures, !MessageBodyPictures.isPixel(tag), let url = originalSource(in: tag), RigNet.allows(URL(string: url), env: env) {
                out.replaceSubrange(r, with: "<img src=\"\(escape(url))\" alt=\"\">")
                continue
            }
            let label = blocked ? MessageBodyWords.pictureNotLoaded : MessageBodyWords.picture
            // A blocked picture keeps the size it declared: narrower than the words need (BODY_BLOCKED_TEXT_MIN_PX) it is a small neutral mark with no text, never taller than
            // declared; the words only where they fit on one line (owner 2026-11-13: "Picture not loaded" stacked one letter per line, 300 px tall, in a signature's 24 px icons).
            let size = declaredSize(tag)
            if let w = size.width, w < WhooshConstants.BODY_BLOCKED_TEXT_MIN_PX {
                let h = size.height.map { min($0, max(w, 1)) } ?? w   // a mark is at most as tall as declared (and no taller than wide when the height is unknown)
                out.replaceSubrange(r, with: "<span class=\"blocked mark\" role=\"img\" aria-label=\"\(escape(label))\" style=\"width:\(w)px;height:\(h)px\"></span>")
            } else {
                let style = size.width.map { " style=\"width:\($0)px\(size.height.map { ";height:\($0)px" } ?? "")\"" } ?? ""
                out.replaceSubrange(r, with: "<span class=\"blocked\" role=\"img\" aria-label=\"\(escape(label))\"\(style)>\(escape(label))</span>")
            }
        }
        return out
    }

    /// The width and height an image tag declares (attributes, or `width:` / `height:` in its style), in px; nil when it declares none.
    static func declaredSize(_ tag: String) -> (width: Int?, height: Int?) {
        func value(_ name: String) -> Int? {
            let patterns = ["\\b\(name)\\s*=\\s*[\"']?(\\d+)", "\(name)\\s*:\\s*(\\d+)\\s*px"]
            for p in patterns {
                if let re = try? NSRegularExpression(pattern: p, options: .caseInsensitive), let m = re.firstMatch(in: tag, range: NSRange(tag.startIndex..., in: tag)),
                   let r = Range(m.range(at: 1), in: tag), let n = Int(tag[r]) { return n }
            }
            return nil
        }
        return (value("width"), value("height"))
    }

    private static let sourceAttribute = try! NSRegularExpression(pattern: "data-whoosh-src\\s*=\\s*[\"'](https?://[^\\s\"'<>]+)[\"']", options: .caseInsensitive)

    private static func originalSource(in tag: String) -> String? {
        guard let m = sourceAttribute.firstMatch(in: tag, range: NSRange(tag.startIndex..., in: tag)), let r = Range(m.range(at: 1), in: tag) else { return nil }
        let address = String(tag[r])
        // a picture kept with an http address is asked for over https: the page's own policy lets https pictures in, nothing else (owner's store, 13 Nov: 36 of 1277)
        // (an address on this Mac stays as it is: a check's own local server speaks http)
        guard address.lowercased().hasPrefix("http://"), !RigNet.isThisMac(URL(string: address)?.host) else { return address }
        return "https://" + address.dropFirst("http://".count)
    }

    private static func plainBlocks(_ text: String) -> String { MessageBodyPlain.blocks(text) }

    static func escape(_ s: String) -> String {
        s.replacingOccurrences(of: "&", with: "&amp;").replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;").replacingOccurrences(of: "\"", with: "&quot;")
    }

    static func css(_ rgba: RGBA) -> String {
        func c(_ v: Double) -> Int { Int((v * 255).rounded()) }
        return "rgba(\(c(rgba.red)),\(c(rgba.green)),\(c(rgba.blue)),\(rgba.opacity))"
    }

    private static func wrap(_ body: String, scheme: ColorScheme, colours: BodyColours, loadPictures: Bool, rig: Bool = false) -> String {
        let p = WhooshPalette.self
        let t = WhooshTypography.body, cap = WhooshTypography.caption
        let picturesMark = loadPictures ? " data-wp=\"1\"" : ""   // the page says it is the pictures document (read by the app's own script after the load)
        let gap = WhooshSpacing.s12, pad = WhooshSpacing.s12
        let label = css(p.label.value(scheme)), second = css(p.secondaryLabel.value(scheme)), third = css(p.tertiaryLabel.value(scheme))
        let accent = css(p.accent.value(scheme)), sep = css(p.separator.value(scheme)), fill = css(p.fill.value(scheme))
        let dotsGap = Int(WhooshSpacing.s10 + WhooshMetrics.keyCap)       // the pill's room: its 10 pt margin and its 20 pt height
        return """
        <!doctype html><html class="\(colours.rawValue)"><head><meta charset="utf-8">
        <meta http-equiv="Content-Security-Policy" content="\(loadPictures ? (rig ? MessageBodyPolicy.rigPicturesContentSecurityPolicy : MessageBodyPolicy.picturesContentSecurityPolicy) : MessageBodyPolicy.contentSecurityPolicy)">
        <style>
        *{box-sizing:border-box;max-width:100%;overflow-wrap:anywhere}
        html,body{margin:0;padding:0;background:transparent}
        html.app{color-scheme:\(scheme == .dark ? "dark" : "light")}
        body{font:\(Int(t.size))px/\(Int(t.lineHeight))px -apple-system,BlinkMacSystemFont,"SF Pro Text",sans-serif;
          -webkit-font-smoothing:antialiased;font-variant-numeric:tabular-nums}
        html.app body{color:\(label);max-width:\(Int(MessageBodyMetrics.measure))px}
        html.app body *{color:inherit!important;background:transparent!important;font-family:inherit!important}
        p{margin:\(Int(gap))px 0 0}.m>*:first-child,.m>p:first-child{margin-top:0}
        p.pt{white-space:pre-wrap}
        html.app a{color:\(accent)!important;text-decoration:none}html.app a:hover{text-decoration:underline}
        img,svg,video,iframe{display:none}
        table{border-collapse:collapse}html.app table{width:auto}td,th{padding:2px 4px;vertical-align:top}
        pre{white-space:pre-wrap;font:\(Int(cap.size + 1))px/\(Int(t.lineHeight))px ui-monospace,"SF Mono",monospace}
        blockquote{margin:\(Int(WhooshSpacing.s4))px 0 \(Int(WhooshSpacing.s8))px;padding:1px 0 1px \(Int(pad))px;border-left:2px solid \(third)}
        html.app blockquote,html.app blockquote *,html.app .q{color:\(second)!important}
        html.app .od,html.original .ad{display:none}
        .dg{height:\(dotsGap)px}
        .q[hidden]{display:none}.q{padding-left:\(Int(pad))px;border-left:2px solid \(third)}html.app .q *{color:\(second)!important}
        .q blockquote{border-left:0;padding-left:0}.q blockquote blockquote{border-left:2px solid \(third);padding-left:\(Int(pad))px}
        .blocked{display:inline-flex;align-items:center;justify-content:center;margin:2px 0;padding:0 \(Int(WhooshSpacing.s8))px;border-radius:\(Int(WhooshRadius.control))px;white-space:nowrap;overflow:hidden;overflow-wrap:normal;vertical-align:top;
          background:\(fill)!important;color:\(second)!important;font-size:\(Int(cap.size))px;line-height:\(Int(cap.lineHeight))px}
        .blocked.mark{padding:0;margin:0;font-size:0;border-radius:4px}
        html.app .blocked{background:\(fill)!important;color:\(second)!important}
        /* Lead, 13 Nov (owner A162, picture 198 "some emails not super well dsiplayed"): these rules were written `html.orig` while the page's class is
           BodyColours' raw value, `original`. None of them ever applied: a designed mail had no paper of the app's (black text on the dark reader when
           the mail sets no background), no inner margin, both documents shown when the engine gives two, and no tile for a blocked picture. */
        html.original{color-scheme:light;background:#fff}
        html.original body{background:#fff;color:#1d1d1f;max-width:none;padding:\(WhooshConstants.BODY_PAPER_PAD_PX)px}   /* the page's inner margin on all four sides */
        html.original a{color:#0066cc}
        html.original blockquote,html.original .q{border-left-color:#c7c7cc;color:#6e6e73}
        /* A blocked picture on the sender's white page is a TILE the eye reads as "a picture goes here": a grey a step under the paper, a hairline, its words in
           the middle. Lead, 13 Nov (owner A162, main117 pictures): faint stripes on white with the words at the top left read as a blank hole in the page. */
        html.original .blocked{background:#f0f0f3!important;box-shadow:inset 0 0 0 1px #e2e2e7;color:#7c7c84!important;border-radius:8px;padding:0 12px}
        html.original .blocked.mark{padding:0;border-radius:4px}
        \(loadPictures ? "img{display:inline-block;height:auto}" : "")
        \(PassageMark.css(accent: p.accent.value(scheme)))
        ::selection{background:\(css(p.selection.value(scheme)))}
        </style></head><body\(picturesMark)>\(body)</body></html>
        """
    }
}
