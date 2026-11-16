import Foundation

/// The three kinds of line a Replies text holds (shared by every platform; the look is AppKit's, below).
enum RepliesLineKind {
    enum Kind: String { case body, heading, note }

    static func kind(_ line: String) -> Kind {
        if line.hasPrefix("## ") { return .heading }
        if line.hasPrefix("//") { return .note }
        return .body
    }

    /// The kind of every line, for the socket (`lines`): what the check reads to prove a note is drawn the moment it is typed.
    static func kinds(_ text: String) -> [String] { text.components(separatedBy: "\n").map { kind($0).rawValue } }
}

#if os(macOS)
import AppKit

/// The look of the rest state, kept while typing (`.doc`: 13.5 pt, line height 1.6, 6 pt between paragraphs; « ## » a 15 pt heading; « // » a
/// grey italic note the model never reads). Applied to the edited paragraphs only, the moment they change: typing never restyles the whole text.
@MainActor
enum RepliesLineStyle {
    nonisolated static let size: CGFloat = 13.5

    static var body: [NSAttributedString.Key: Any] {
        [.font: NSFont.systemFont(ofSize: size), .foregroundColor: NSColor.labelColor, .paragraphStyle: paragraph(before: 0)]
    }

    static func paragraph(before: CGFloat) -> NSParagraphStyle {
        let p = NSMutableParagraphStyle()
        p.minimumLineHeight = 21.6; p.maximumLineHeight = 21.6
        p.paragraphSpacing = 6; p.paragraphSpacingBefore = before
        return p
    }


    /// Restyles the paragraphs that hold `range` (a few hundred characters at most per key), keeping each pill's own attachment.
    static func restyle(_ st: NSTextStorage, around range: NSRange) {
        let ns = st.string as NSString
        guard ns.length > 0 else { return }
        let start = min(range.location, ns.length - 1)
        let span = ns.paragraphRange(for: NSRange(location: start, length: min(range.length, ns.length - start)))
        ns.enumerateSubstrings(in: span, options: .byParagraphs) { line, lineRange, enclosing, _ in
            let k = RepliesLineKind.kind(line ?? "")
            var a = body
            switch k {
            case .heading:
                a[.font] = NSFont.systemFont(ofSize: 15, weight: .semibold)
                a[.paragraphStyle] = paragraph(before: lineRange.location == 0 ? 0 : 8)
            case .note:
                a[.font] = NSFontManager.shared.convert(NSFont.systemFont(ofSize: size), toHaveTrait: .italicFontMask)
                a[.foregroundColor] = NSColor.tertiaryLabelColor
            case .body: break
            }
            st.addAttributes(a, range: enclosing)
            // « ## » itself is quieter than the heading it opens (it stays typable: a heading is three keys away)
            if k == .heading { st.addAttribute(.foregroundColor, value: NSColor.tertiaryLabelColor, range: NSRange(location: lineRange.location, length: 3)) }
        }
    }

}
#endif
