import Foundation
import SwiftUI
#if canImport(AppKit)
import AppKit
typealias PFont = NSFont
typealias PColor = NSColor
typealias PImage = NSImage
#else
import UIKit
typealias PFont = UIFont
typealias PColor = UIColor
typealias PImage = UIImage
#endif

/// Document <-> attributed text for the platform text view. Custom keys carry what the model needs
/// (bold, list, content-id); the text view's fonts and colours come only from WhooshTheme tokens.
enum RichAttributed {
    static let boldKey = NSAttributedString.Key("whoosh.bold")
    static let cidKey = NSAttributedString.Key("whoosh.cid")
    static let listKey = NSAttributedString.Key("whoosh.list")
    static let markerKey = NSAttributedString.Key("whoosh.marker")
    static let italicKey = NSAttributedString.Key("whoosh.italic")
    static let underlineKey = NSAttributedString.Key("whoosh.underline")
    static let strikeKey = NSAttributedString.Key("whoosh.strike")
    static let colorKey = NSAttributedString.Key("whoosh.color")
    static let numberedKey = NSAttributedString.Key("whoosh.numbered")
    static let quoteKey = NSAttributedString.Key("whoosh.quote")
    static let indentKey = NSAttributedString.Key("whoosh.indent")
    static let marker = "•\t"

    static func color(_ t: ThemeColor) -> PColor {
        #if canImport(AppKit)
        NSColor(name: nil) { a in
            let c = a.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua ? t.dark : t.light
            return NSColor(srgbRed: c.red, green: c.green, blue: c.blue, alpha: c.opacity)
        }
        #else
        UIColor { tr in
            let c = tr.userInterfaceStyle == .dark ? t.dark : t.light
            return UIColor(red: c.red, green: c.green, blue: c.blue, alpha: c.opacity)
        }
        #endif
    }

    static func font(bold: Bool, italic: Bool = false) -> PFont {
        let s = WhooshTypography.body
        let base = PFont.systemFont(ofSize: s.size, weight: bold ? .semibold : .regular)
        guard italic else { return base }
        #if canImport(AppKit)
        return NSFont(descriptor: base.fontDescriptor.withSymbolicTraits(.italic), size: s.size) ?? base
        #else
        return base.fontDescriptor.withSymbolicTraits(.traitItalic).map { UIFont(descriptor: $0, size: s.size) } ?? base
        #endif
    }

    /// One indent step: a list level or a block indent.
    static let indentStep = WhooshSpacing.s20

    static func paragraphStyle(list: Bool, numbered: Bool = false, quote: Bool = false, indent: Int = 0) -> NSParagraphStyle {
        let p = NSMutableParagraphStyle()
        p.minimumLineHeight = WhooshTypography.body.lineHeight
        p.maximumLineHeight = WhooshTypography.body.lineHeight
        let base = CGFloat(indent) * indentStep
        if list {
            let hang = numbered ? WhooshSpacing.s24 : WhooshSpacing.s20
            p.firstLineHeadIndent = base + (numbered ? WhooshSpacing.s4 : WhooshSpacing.s8); p.headIndent = base + hang
            p.tabStops = [NSTextTab(textAlignment: .left, location: base + hang)]
        } else if quote {
            p.firstLineHeadIndent = base + WhooshSpacing.s16; p.headIndent = base + WhooshSpacing.s16
        } else if indent > 0 {
            p.firstLineHeadIndent = base; p.headIndent = base
        }
        return p
    }

    /// The marks and colour of a run, as the attributes the text view draws and `read` takes back.
    static func runAttributes(_ r: RichRun, _ p: RichParagraph) -> [NSAttributedString.Key: Any] {
        var a: [NSAttributedString.Key: Any] = [.font: font(bold: r.bold, italic: r.italic),
                                                .foregroundColor: color(p.quote ? WhooshPalette.secondaryLabel : WhooshPalette.label),
                                                .paragraphStyle: paragraphStyle(list: p.isListItem, numbered: p.numbered, quote: p.quote, indent: p.indent)]
        if p.isListItem { a[listKey] = true }
        if p.numbered { a[numberedKey] = true }
        if p.quote { a[quoteKey] = true }
        if p.indent > 0 { a[indentKey] = p.indent }
        if r.bold { a[boldKey] = true }
        if r.italic { a[italicKey] = true }
        if r.underline { a[underlineKey] = true; a[.underlineStyle] = NSUnderlineStyle.single.rawValue }
        if r.strike { a[strikeKey] = true; a[.strikethroughStyle] = NSUnderlineStyle.single.rawValue }
        if let name = r.color, let c = RichColor(rawValue: name) { a[colorKey] = name; a[.foregroundColor] = color(c.token) }
        return a
    }

    /// Turn one mark on or off in an attribute set; the font and the drawn line follow the flags.
    static func setMark(_ key: NSAttributedString.Key, _ on: Bool, in a: inout [NSAttributedString.Key: Any]) {
        a[key] = on ? true : nil
        if key == underlineKey { a[.underlineStyle] = on ? NSUnderlineStyle.single.rawValue : nil }
        if key == strikeKey { a[.strikethroughStyle] = on ? NSUnderlineStyle.single.rawValue : nil }
        if key == boldKey || key == italicKey { a[.font] = font(bold: a[boldKey] != nil, italic: a[italicKey] != nil) }
    }

    static func setColor(_ name: String?, in a: inout [NSAttributedString.Key: Any]) {
        a[colorKey] = name
        a[.foregroundColor] = color(name.flatMap(RichColor.init(rawValue:))?.token ?? WhooshPalette.label)
    }

    /// The width of the list marker that starts the line at `at`, 0 when it has none.
    static func markerLength(_ s: NSAttributedString, at: Int) -> Int {
        guard at < s.length else { return 0 }
        var range = NSRange(location: 0, length: 0)
        return s.attribute(markerKey, at: at, effectiveRange: &range) != nil ? range.length : 0
    }

    /// Where a position lands after the document is rebuilt: the same paragraph, the same offset in its words.
    static func remap(_ loc: Int, from old: NSAttributedString, to new: NSAttributedString) -> Int {
        let o = old.string as NSString, n = new.string as NSString
        let line = o.lineRange(for: NSRange(location: min(loc, o.length), length: 0))
        let index = o.substring(to: line.location).filter { $0 == "\n" }.count
        let offset = max(0, loc - line.location - markerLength(old, at: line.location))
        var start = 0
        for _ in 0..<index {
            let r = n.range(of: "\n", range: NSRange(location: start, length: n.length - start))
            if r.location == NSNotFound { break }
            start = NSMaxRange(r)
        }
        let nl = n.lineRange(for: NSRange(location: min(start, n.length), length: 0))
        var end = NSMaxRange(nl)
        if end > nl.location, n.character(at: end - 1) == 10 { end -= 1 }
        return min(nl.location + markerLength(new, at: nl.location) + offset, end)
    }

    static var typing: [NSAttributedString.Key: Any] {
        [.font: font(bold: false), .foregroundColor: color(WhooshPalette.label), .paragraphStyle: paragraphStyle(list: false)]
    }

    static func make(_ doc: RichDocument) -> NSAttributedString {
        let out = NSMutableAttributedString()
        var counters: [Int] = []          // 1. 2. 3. per list level; a paragraph that is not a numbered item starts over
        for (n, p) in doc.paragraphs.enumerated() {
            if n > 0 { out.append(NSAttributedString(string: "\n", attributes: typing)) }
            let blank = RichRun("")
            let pa = runAttributes(blank, p)
            if p.isListItem {
                var text = marker
                if p.numbered {
                    counters = Array(counters.prefix(p.indent + 1))
                    while counters.count <= p.indent { counters.append(0) }
                    counters[p.indent] += 1
                    text = "\(counters[p.indent]).\t"
                } else { counters = [] }
                var ma = pa
                ma[.font] = font(bold: false); ma[.foregroundColor] = color(WhooshPalette.secondaryLabel); ma[markerKey] = true
                out.append(NSAttributedString(string: text, attributes: ma))
            } else { counters = [] }
            for r in p.runs {
                var a = runAttributes(r, p)
                if let h = r.href, let u = URL(string: h) { a[.link] = u; a[.foregroundColor] = color(WhooshPalette.accent) }
                if let cid = r.cid {
                    let att = NSTextAttachment()
                    if let picture = RichPictures.shared.cached(cid) {
                        // the picture itself (a thumbnail made off the main thread), shrunk to fit the text
                        att.image = picture
                        att.bounds = CGRect(origin: .zero, size: RichPictures.drawnSize(width: picture.size.width, height: picture.size.height))
                    } else {
                        #if canImport(AppKit)
                        att.image = NSImage(systemSymbolName: "photo", accessibilityDescription: cid)
                        #else
                        att.image = UIImage(systemName: "photo")
                        #endif
                        RichPictures.shared.load(cid)   // no file registered for this content id: nothing happens, the symbol stays
                    }
                    let s = NSMutableAttributedString(attachment: att)
                    s.addAttributes(a.merging([cidKey: cid]) { $1 }, range: NSRange(location: 0, length: s.length))
                    out.append(s)
                } else {
                    out.append(NSAttributedString(string: r.text, attributes: a))
                }
            }
        }
        return out
    }

    /// The attributes new text takes inside this paragraph (an empty list item or quote keeps its look while typing).
    static func typing(for p: RichParagraph) -> [NSAttributedString.Key: Any] {
        var a = runAttributes(RichRun(""), p)
        if p.isListItem { a[listKey] = true }
        return a
    }

    static func read(_ s: NSAttributedString) -> RichDocument {
        var paragraphs: [RichParagraph] = []
        let ns = s.string as NSString
        var loc = 0
        while loc <= ns.length {
            let line = ns.lineRange(for: NSRange(location: min(loc, ns.length), length: 0))
            var end = NSMaxRange(line)
            if end > line.location, ns.character(at: end - 1) == 10 { end -= 1 }
            let body = NSRange(location: line.location, length: end - line.location)
            var runs: [RichRun] = []
            var isList = false, numbered = false, quote = false, indent = 0
            s.enumerateAttributes(in: body) { attrs, range, _ in
                if attrs[numberedKey] != nil { numbered = true }
                if attrs[quoteKey] != nil { quote = true }
                if let i = attrs[indentKey] as? Int { indent = i }
                if attrs[markerKey] != nil { isList = true; return }
                if attrs[listKey] != nil { isList = true }
                if let cid = attrs[cidKey] as? String { runs.append(RichRun("", cid: cid)); return }
                let t = ns.substring(with: range)
                let href = (attrs[.link] as? URL)?.absoluteString ?? (attrs[.link] as? String)
                let run = RichRun(t, bold: attrs[boldKey] != nil, href: href, italic: attrs[italicKey] != nil,
                                  underline: attrs[underlineKey] != nil, strike: attrs[strikeKey] != nil, color: attrs[colorKey] as? String)
                if let last = runs.last, last.sameStyle(as: run) { runs[runs.count - 1].text += t } else { runs.append(run) }
            }
            if !(runs.isEmpty && !isList && paragraphs.isEmpty && ns.length == 0) {
                paragraphs.append(RichParagraph(isListItem: isList, numbered: numbered, quote: quote, indent: indent, runs: runs))
            }
            if NSMaxRange(line) >= ns.length { break }
            loc = NSMaxRange(line)
        }
        return RichDocument(paragraphs: paragraphs)
    }
}
