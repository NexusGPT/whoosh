import Foundation

/// Document <-> the HTML part of the MIME body. The writer emits only what the composer can show
/// (strong, ul/li, a href, img cid, p); the reader keeps only the same set and drops everything else,
/// so pasted or received HTML can never put a script, an event handler or a source font into a draft.
public enum RichHTML {
    // MARK: write

    public static func serialize(_ doc: RichDocument) -> String {
        let ps = doc.paragraphs
        if ps.isEmpty { return "" }
        if ps.count == 1, !ps[0].isListItem, !ps[0].quote, ps[0].indent == 0 { return inline(ps[0].runs) }
        var out = ""
        var open: [String] = []           // the list tags still open, innermost last; the innermost <li> is open too
        func closeTo(_ n: Int) { while open.count > n { out += "</li></\(open.removeLast())>" } }
        for p in ps {
            if p.isListItem {
                let tag = p.numbered ? "ol" : "ul"
                let want = min(p.indent, open.count) + 1     // a list opens one level deeper than the one above it, no more
                closeTo(want)
                if open.count == want {
                    out += "</li>"
                    if open[want - 1] != tag { out += "</\(open.removeLast())><\(tag)>"; open.append(tag) }
                } else { out += "<\(tag)>"; open.append(tag) }
                out += "<li>\(inline(p.runs))"
                continue
            }
            closeTo(0)
            if p.quote { out += "<blockquote>\(inline(p.runs))</blockquote>" }
            else if p.indent > 0 { out += "<p style=\"margin-left:\(p.indent * indentPx)px\">\(inline(p.runs))</p>" }
            else { out += "<p>\(inline(p.runs))</p>" }
        }
        closeTo(0)
        return out
    }

    /// One indent step in the HTML part.
    static let indentPx = 24

    private static func inline(_ runs: [RichRun]) -> String {
        runs.map { r -> String in
            if let cid = r.cid { return "<img src=\"cid:\(escape(cid, quotes: true))\">" }
            var t = escape(r.text, quotes: false)
            if r.bold { t = "<strong>\(t)</strong>" }
            if r.italic { t = "<em>\(t)</em>" }
            if r.underline { t = "<u>\(t)</u>" }
            if r.strike { t = "<s>\(t)</s>" }
            if let c = r.color.flatMap(RichColor.init(rawValue:)) { t = "<span style=\"color:\(c.hex)\">\(t)</span>" }
            if let h = r.href { t = "<a href=\"\(escape(h, quotes: true))\">\(t)</a>" }
            return t
        }.joined()
    }

    static func escape(_ s: String, quotes: Bool) -> String {
        var o = s.replacingOccurrences(of: "&", with: "&amp;").replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
        if quotes { o = o.replacingOccurrences(of: "\"", with: "&quot;") }
        return o
    }

    // MARK: read

    /// Parse HTML into a document. `matchStyle` is implied: no font, size or colour is ever read.
    public static func parse(_ html: String) -> RichDocument {
        var b = Builder()
        var i = html.startIndex
        var skip: String?                 // inside <script> or <style>: drop until the close tag
        var boldDepth = 0, italicDepth = 0, underlineDepth = 0, strikeDepth = 0
        var href: [String?] = []
        var text = ""
        func flush() {
            guard !text.isEmpty else { return }
            b.add(RichRun(decode(text), bold: boldDepth > 0, href: href.last ?? nil, italic: italicDepth > 0, underline: underlineDepth > 0, strike: strikeDepth > 0))
            text = ""
        }
        while i < html.endIndex {
            let c = html[i]
            if c == "<", let close = html[i...].firstIndex(of: ">") {
                let raw = String(html[html.index(after: i)..<close])
                i = html.index(after: close)
                let (name, closing, attrs) = tag(raw)
                if let s = skip { if closing && name == s { skip = nil }; continue }
                if raw.hasPrefix("!") || raw.hasPrefix("?") { continue }
                if !closing && (name == "script" || name == "style") { flush(); skip = name; continue }
                flush()
                switch (name, closing) {
                case ("strong", false), ("b", false): boldDepth += 1
                case ("strong", true), ("b", true): boldDepth = max(0, boldDepth - 1)
                case ("em", false), ("i", false): italicDepth += 1
                case ("em", true), ("i", true): italicDepth = max(0, italicDepth - 1)
                case ("u", false): underlineDepth += 1
                case ("u", true): underlineDepth = max(0, underlineDepth - 1)
                case ("s", false), ("strike", false), ("del", false): strikeDepth += 1
                case ("s", true), ("strike", true), ("del", true): strikeDepth = max(0, strikeDepth - 1)
                case ("a", false): href.append(safeURL(attrs["href"]))
                case ("a", true): if !href.isEmpty { href.removeLast() }
                case ("img", false):
                    if let s = attrs["src"], s.lowercased().hasPrefix("cid:") { b.add(RichRun("", cid: String(s.dropFirst(4)))) }
                case ("ul", _), ("ol", _):
                    b.breakParagraph()
                    if closing { if !b.lists.isEmpty { b.lists.removeLast() } } else { b.lists.append(name == "ol") }
                case ("li", false): b.breakParagraph(); b.forceList = true
                case ("blockquote", _): b.breakParagraph(); b.quoteDepth = max(0, b.quoteDepth + (closing ? -1 : 1))
                case ("p", false), ("div", false): b.breakParagraph(); b.pendingIndent = indent(of: attrs["style"])
                case ("li", true), ("p", true), ("div", true), ("br", _), ("tr", _), ("h1", _), ("h2", _), ("h3", _):
                    b.breakParagraph()
                default: break
                }
            } else {
                if skip == nil { text.append(c) }
                i = html.index(after: i)
            }
        }
        if skip == nil { flush() }
        return b.finish()
    }

    /// `margin-left:48px` is two indent steps; anything else is none.
    private static func indent(of style: String?) -> Int {
        guard let style, let r = style.range(of: "margin-left:") else { return 0 }
        let digits = style[r.upperBound...].drop { $0 == " " }.prefix { $0.isNumber }
        return min((Int(digits) ?? 0) / indentPx, RichParagraph.maxIndent)
    }

    private static func tag(_ raw: String) -> (String, Bool, [String: String]) {
        var s = Substring(raw.trimmingCharacters(in: .whitespacesAndNewlines))
        let closing = s.hasPrefix("/")
        if closing { s = s.dropFirst() }
        if s.hasSuffix("/") { s = s.dropLast() }
        let name = String(s.prefix { !$0.isWhitespace }).lowercased()
        var attrs: [String: String] = [:]
        var rest = s.dropFirst(name.count)
        while let eq = rest.firstIndex(of: "=") {
            let key = rest[..<eq].trimmingCharacters(in: .whitespaces).split(whereSeparator: \.isWhitespace).last.map(String.init)?.lowercased() ?? ""
            var v = rest[rest.index(after: eq)...].drop { $0.isWhitespace }
            var val = ""
            if let q = v.first, q == "\"" || q == "'" {
                v = v.dropFirst()
                let end = v.firstIndex(of: q) ?? v.endIndex
                val = String(v[..<end]); rest = end == v.endIndex ? "" : v[v.index(after: end)...]
            } else {
                let end = v.firstIndex(where: \.isWhitespace) ?? v.endIndex
                val = String(v[..<end]); rest = v[end...]
            }
            if !key.isEmpty { attrs[key] = decode(val) }
        }
        return (name, closing, attrs)
    }

    /// Only web, mail and cid targets survive; `javascript:` and the rest are dropped.
    static func safeURL(_ s: String?) -> String? {
        guard let s, !s.isEmpty else { return nil }
        let l = s.trimmingCharacters(in: .whitespaces).lowercased()
        return ["http://", "https://", "mailto:"].contains { l.hasPrefix($0) } ? s : nil
    }

    static func decode(_ s: String) -> String {
        guard s.contains("&") else { return s }
        var out = "", i = s.startIndex
        let named = ["amp": "&", "lt": "<", "gt": ">", "quot": "\"", "apos": "'", "nbsp": " "]
        while i < s.endIndex {
            if s[i] == "&", let semi = s[i...].prefix(10).firstIndex(of: ";") {
                let ent = String(s[s.index(after: i)..<semi])
                var rep: String?
                if let n = named[ent] { rep = n }
                else if ent.hasPrefix("#x") || ent.hasPrefix("#X"), let v = UInt32(ent.dropFirst(2), radix: 16), let u = Unicode.Scalar(v) { rep = String(u) }
                else if ent.hasPrefix("#"), let v = UInt32(ent.dropFirst()), let u = Unicode.Scalar(v) { rep = String(u) }
                if let rep { out += rep; i = s.index(after: semi); continue }
            }
            out.append(s[i]); i = s.index(after: i)
        }
        return out
    }

    private struct Builder {
        var paragraphs: [RichParagraph] = []
        var current: [RichRun] = []
        var lists: [Bool] = []             // open <ul>/<ol>, innermost last; true for <ol>
        var quoteDepth = 0
        var pendingIndent = 0
        var forceList = false
        var started = false
        var isList = false, numbered = false, quote = false, indent = 0

        mutating func add(_ r: RichRun) {
            if !started {
                started = true
                isList = !lists.isEmpty && forceList
                numbered = isList && (lists.last ?? false)
                quote = quoteDepth > 0
                indent = isList ? lists.count - 1 : pendingIndent
            }
            if r.cid == nil && r.text.isEmpty { return }
            if let last = current.last, last.sameStyle(as: r) { current[current.count - 1].text += r.text }
            else { current.append(r) }
        }
        mutating func breakParagraph() {
            if !current.isEmpty {
                paragraphs.append(RichParagraph(isListItem: isList, numbered: numbered, quote: quote, indent: indent, runs: current))
            }
            current = []; started = false; forceList = false; isList = false; numbered = false; quote = false; indent = 0; pendingIndent = 0
        }
        mutating func finish() -> RichDocument {
            breakParagraph()
            return RichDocument(paragraphs: paragraphs)
        }
    }
}
