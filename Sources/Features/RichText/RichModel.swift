import Foundation

/// The rich text document model of the composers: platform-neutral, no AppKit, no UIKit.
///
/// Pattern: RichTextKit (github.com/danielsaidi/RichTextKit, MIT) keeps one context object beside a
/// platform text view and maps attributed text to a format model. Whoosh follows that split (model and
/// HTML here, the AppKit/UIKit view in RichTextEditor.swift) and adds the paragraph-level undo the
/// AI rewrite needs. No RichTextKit source is copied (Package.swift is frozen and takes no dependency).

/// One styled stretch of text inside a paragraph.
public struct RichRun: Equatable, Sendable {
    public var text: String
    public var bold: Bool
    public var italic: Bool
    public var underline: Bool
    public var strike: Bool
    /// A palette token name (`RichColor`); nil is the default text colour. Never a raw value.
    public var color: String?
    /// Link target; nil for plain text.
    public var href: String?
    /// Content-ID of an inline picture (`cid:` URL); nil for text. A picture run has empty text.
    public var cid: String?
    /// Always nil: pasted text is matched to the composer style, so no source font survives.
    public var font: String? { nil }

    public init(_ text: String, bold: Bool = false, href: String? = nil, cid: String? = nil,
                italic: Bool = false, underline: Bool = false, strike: Bool = false, color: String? = nil) {
        self.text = text; self.bold = bold; self.href = href; self.cid = cid
        self.italic = italic; self.underline = underline; self.strike = strike; self.color = color
    }

    /// The marks on this run, in a fixed order: bold, italic, underline, strike, then the colour as `color:<name>`.
    public var marks: [String] {
        var m: [String] = []
        if bold { m.append("bold") }
        if italic { m.append("italic") }
        if underline { m.append("underline") }
        if strike { m.append("strike") }
        if let c = color { m.append("color:" + c) }
        return m
    }

    /// True when two runs carry the same marks, so they can be one run.
    public func sameStyle(as o: RichRun) -> Bool {
        cid == nil && o.cid == nil && bold == o.bold && italic == o.italic && underline == o.underline
            && strike == o.strike && color == o.color && href == o.href
    }

    /// The format word the control socket row carries: one mark by its name, several joined by "+".
    public var formatWord: String? {
        if cid != nil { return "picture" }
        if href != nil { return "link" }
        return marks.isEmpty ? nil : marks.joined(separator: "+")
    }
}

/// One paragraph: a body line or a list item. The id stays with the paragraph through edits, so an
/// undo entry finds its paragraph even after others moved.
public struct RichParagraph: Equatable, Sendable, Identifiable {
    public var id: UUID
    public var isListItem: Bool
    /// A list item counted 1. 2. 3. instead of bulleted; meaningful only with `isListItem`.
    public var numbered: Bool
    /// A quoted paragraph (the Quote key).
    public var quote: Bool
    /// How far the paragraph is indented, 0 to `RichParagraph.maxIndent`: a list level, or a block indent.
    public var indent: Int
    public var runs: [RichRun]

    public static let maxIndent = 4

    public init(id: UUID = UUID(), isListItem: Bool = false, numbered: Bool = false, quote: Bool = false, indent: Int = 0, runs: [RichRun]) {
        self.id = id; self.isListItem = isListItem; self.numbered = numbered && isListItem; self.quote = quote
        self.indent = min(max(indent, 0), Self.maxIndent); self.runs = runs
    }

    public init(_ text: String, isListItem: Bool = false) {
        self.init(isListItem: isListItem, runs: text.isEmpty ? [] : [RichRun(text)])
    }

    /// The paragraph's kind as one word: body, bullet, number or quote.
    public var block: String { isListItem ? (numbered ? "number" : "bullet") : (quote ? "quote" : "body") }

    public var text: String { runs.map(\.text).joined() }
}

public struct RichDocument: Equatable, Sendable {
    public var paragraphs: [RichParagraph]

    public init(paragraphs: [RichParagraph] = []) { self.paragraphs = paragraphs }

    /// Plain words, paragraphs separated by a line break.
    public var plainText: String { paragraphs.map(\.text).joined(separator: "\n") }

    /// An empty draft has no paragraph at all.
    public var isEmpty: Bool { paragraphs.allSatisfy { $0.runs.isEmpty && !$0.isListItem } }

    /// Rows for `rows mac-composer.runs`: one row per run, one per list item.
    public var rows: [[String: Any]] {
        var out: [[String: Any]] = []
        for p in paragraphs {
            if p.isListItem {
                out.append(Self.row(p.text, p.numbered ? "numbered-item" : "list-item", nil, nil, p))
            } else if p.runs.isEmpty {
                continue
            } else {
                for r in p.runs { out.append(Self.row(r.text, r.formatWord, r.href, r.cid, p)) }
            }
        }
        return out
    }

    private static func row(_ text: String, _ format: String?, _ href: String?, _ cid: String?, _ p: RichParagraph) -> [String: Any] {
        ["text": text, "format": format as Any? ?? NSNull(), "href": href as Any? ?? NSNull(),
         "cid": cid as Any? ?? NSNull(), "font": NSNull(), "block": p.block, "indent": p.indent]
    }
}
