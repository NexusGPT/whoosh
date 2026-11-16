#if os(macOS)
import AppKit

/// The text of a Replies editor as TextKit holds it. A variable is ONE character: an NSTextAttachment whose cell draws the blue « @nom »
/// pill, so Backspace removes it whole, the arrows step over it and undo treats it as one change (Apple's documented NSTextAttachmentCell
/// route of TextKit 1; the repo's RichText editor puts pictures in its text the same way). Stored form: `{{id}}`.
@MainActor
enum RepliesPills {
    /// Marks a pill's character with its variable id (copy, the socket row, the stored form read it).
    static let variable = NSAttributedString.Key("whoosh.replies.variable")

    /// The stored form as attributed text: `{{id}}` of a known variable is a pill, everything else words. Styles come after (RepliesLineStyle).
    static func attributed(_ stored: String, _ vars: [ReplyPromptVariable]) -> NSMutableAttributedString {
        let out = NSMutableAttributedString()
        for piece in RepliesVariables.pieces(stored, vars) {
            switch piece {
            case .words(let w): out.append(NSAttributedString(string: w, attributes: RepliesLineStyle.body))
            case .variable(let name):
                if let v = vars.first(where: { RepliesVariables.name($0) == name }) { out.append(pill(v)) }
                else { out.append(NSAttributedString(string: "{{\(name)}}", attributes: RepliesLineStyle.body)) }
            }
        }
        return out
    }

    /// One variable as one character.
    static func pill(_ v: ReplyPromptVariable) -> NSAttributedString {
        let att = NSTextAttachment()
        att.attachmentCell = RepliesPillCell(label: "@" + RepliesVariables.name(v))
        let s = NSMutableAttributedString(attachment: att)
        s.addAttributes(RepliesLineStyle.body, range: NSRange(location: 0, length: s.length))
        s.addAttribute(variable, value: v.id, range: NSRange(location: 0, length: s.length))
        return s
    }

    /// The stored form of an attributed text: each pill back to `{{id}}`.
    static func stored(_ s: NSAttributedString) -> String { render(s) { promptPlaceholder(id: $0) } }

    /// The text as the owner reads it: each pill as « @nom » (copy as plain text, the socket's `shown`).
    static func shown(_ s: NSAttributedString, _ vars: [ReplyPromptVariable]) -> String {
        render(s) { id in "@" + (vars.first { $0.id == id }.map(RepliesVariables.name) ?? id) }
    }

    /// The socket's view: each pill as « [@nom] », so a check tells a pill from typed words.
    static func marked(_ s: NSAttributedString, _ vars: [ReplyPromptVariable]) -> String {
        render(s) { id in "[@" + (vars.first { $0.id == id }.map(RepliesVariables.name) ?? id) + "]" }
    }

    private static func render(_ s: NSAttributedString, _ pillText: (String) -> String) -> String {
        var out = ""
        let ns = s.string as NSString
        s.enumerateAttribute(variable, in: NSRange(location: 0, length: s.length)) { value, range, _ in
            if let id = value as? String { out += String(repeating: pillText(id), count: range.length) }
            else { out += ns.substring(with: range).replacingOccurrences(of: "\u{FFFC}", with: "") }
        }
        return out
    }

    /// Plain text pasted or typed in: « @nom » of a known variable (the longest name first) and `{{id}}` become pills.
    static func fromPlain(_ text: String, _ vars: [ReplyPromptVariable]) -> NSMutableAttributedString {
        attributed(RepliesVariables.store(text, vars), vars)
    }

    /// « @mot » left as words that names no variable: said in one line under the editor.
    static func unknown(_ stored: String) -> [String] {
        guard let re = try? NSRegularExpression(pattern: "(?<![\\p{L}\\p{N}])@[\\p{L}\\p{N}_]+") else { return [] }
        let ns = stored as NSString
        var seen: [String] = []
        for m in re.matches(in: stored, range: NSRange(location: 0, length: ns.length)) {
            let w = ns.substring(with: m.range)
            if !seen.contains(w) { seen.append(w) }
        }
        return seen
    }
}

/// `.ref`: the board's pill, drawn in the text's own line: 19 pt high, radius 5, the accent at 15 % under the accent words.
final class RepliesPillCell: NSTextAttachmentCell {
    let label: String
    init(label: String) { self.label = label; super.init(textCell: "") }
    required init(coder: NSCoder) { label = ""; super.init(coder: coder) }

    private var words: NSAttributedString {
        NSAttributedString(string: label, attributes: [.font: NSFont.systemFont(ofSize: RepliesLineStyle.size, weight: .medium),
                                                       .foregroundColor: NSColor.controlAccentColor])
    }
    override func cellSize() -> NSSize { NSSize(width: ceil(words.size().width) + 12 + 2, height: 19) }
    override func cellBaselineOffset() -> NSPoint { NSPoint(x: 0, y: -4) }
    override func draw(withFrame cellFrame: NSRect, in controlView: NSView?) {
        let r = cellFrame.insetBy(dx: 1, dy: 0)
        NSColor.controlAccentColor.withAlphaComponent(0.15).setFill()
        NSBezierPath(roundedRect: r, xRadius: 5, yRadius: 5).fill()
        let w = words, size = w.size()
        w.draw(at: NSPoint(x: r.minX + 6, y: r.minY + (r.height - size.height) / 2))
    }
}
#endif
