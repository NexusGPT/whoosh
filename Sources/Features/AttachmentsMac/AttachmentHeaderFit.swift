#if os(macOS)
import AppKit
import SwiftUI

/// The gallery header's one-line rule and what the socket reads (`mac-main.attachments.header`): the rung in use, and
/// each label's text width in its own font (a label narrower than its text was cut or wrapped).
@MainActor
enum AttachmentHeaderFit {
    /// The ladder's rung on screen: 1 words and the Space cap, 2 words, 3 icons.
    static var rung = 0
    /// The label's width as SwiftUI lays the same Text out in the same style (whooshText: monospaced digits, tracking),
    /// so it compares with the label's measured AppKit bounds (CONTROL-SOCKET delta 47). A plain NSFont width did not:
    /// it was wider than the drawn text and every label read "narrower than its text" on main38.
    static func width(_ text: String) -> CGFloat {
        let host = NSHostingView(rootView: Text(text).whooshText(WhooshTypography.caption, emphasized: true).lineLimit(1).fixedSize())
        return ceil(host.fittingSize.width)
    }

    /// A label at its natural width on one line; the wrap-label mutant gives it main's freedom to wrap downward.
    struct Natural: ViewModifier {
        func body(content: Content) -> some View {
            if AttachmentRig.mutant("wrap-label") { content.fixedSize(horizontal: false, vertical: true) }
            else { content.lineLimit(1).fixedSize(horizontal: true, vertical: false) }
        }
    }

    static func row(_ set: AttachmentSet) -> ControlRow {
        let r = AttachmentSocket.reader
        var labels: [String: Int] = ["\(r).gallery.count": Int(width(AttachmentGalleryHeader.words(set)))]
        if rung <= 2 {
            labels["\(r).gallery.quick-look-words"] = Int(width(AttachmentWords.quickLook))
            labels["\(r).gallery.save-all-words"] = Int(width(AttachmentWords.saveAll))
        }
        // The Space key cap is not under the width rule: Components' KeyCap draws a glyph ("␣", 20.5 pt on main39), not
        // the word its caller passes ("Space", 34 pt), and keeps itself on one line. It stays in `frames` (one midY).
        return ["rung": rung, "text_width": labels,
                "frames": [r + ".gallery.count", r + ".gallery.quick-look", r + ".gallery.save-all"] + (rung == 1 ? [r + ".gallery.space-cap"] : [])]
    }
}
#endif
