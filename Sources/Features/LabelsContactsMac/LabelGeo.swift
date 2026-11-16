import SwiftUI
#if os(macOS)
import AppKit
#endif

/// The inspector's geometry on the socket: every card, row and control carries a measured id (`.whooshAx`), and two regions
/// read them back so a check can prove the layout: `mac-labels.inspector.geometry` (frames in window points) and
/// `mac-labels.inspector.rowfit` (each form row's label width against its longest word, the Settings round's rule).
@MainActor
enum LabelGeo {
    nonisolated static let base = "mac-labels.inspector"
    nonisolated static let prefix = "\(base).geo."
    /// The name field's width.
    static var fieldWidth: CGFloat { WhooshSpacing.s32 * 5 }

    nonisolated static func card(_ c: String) -> String { "\(prefix)card.\(c)" }
    nonisolated static func ctl(_ c: String, _ name: String) -> String { "\(prefix)ctl.\(c).\(name)" }
    nonisolated static func row(_ c: String, _ name: String) -> String { "\(prefix)row.\(c).\(name)" }
    nonisolated static func free(_ name: String) -> String { "\(prefix)free.\(name)" }
    /// A chain's controls belong to the card that holds it: the extra Entry chains (`other1`...) are measured inside the `entry` card.
    nonisolated static func chainCtl(_ card: String, _ name: String) -> String { card.hasPrefix("other") ? ctl("entry", "\(card).\(name)") : ctl(card, name) }
    nonisolated static func chainRow(_ card: String, _ name: String) -> String { card.hasPrefix("other") ? row("entry", "\(card).\(name)") : row(card, name) }
    /// The scroll view's visible frame: what scrolls under the foot is clipped there, so the check clips scrolled frames to it.
    nonisolated static func clip(_ name: String) -> String { "\(prefix)clip.\(name)" }
    nonisolated static func fit(_ c: String, _ name: String) -> String { "\(base).fit.\(c).\(name)" }

    static func rows() -> [String: (AppModel) throws -> [ControlRow]] {
        ["\(base).geometry": { _ in geometry() }, "\(base).rowfit": { _ in rowFit() }]
    }

    /// Every measured frame of the open inspector: kind (card, row, ctl, free), the card it belongs to, and x y w h; first, the
    /// tokens the layout promises (a control's inset from its card's edge, the gap under the drop zone).
    static func geometry() -> [ControlRow] {
        _ = FrameProbe.request(card("label"))
        let tokens: ControlRow = ["id": "\(prefix)tokens", "kind": "tokens", "card": NSNull(), "inset": Double(WhooshSpacing.s12)]
        return [tokens] + FrameProbe.ids.filter { $0.hasPrefix(prefix) }.compactMap { id in
            guard let f = FrameProbe.frame(id), f.width > 0, f.height > 0 else { return nil }
            let parts = id.dropFirst(prefix.count).split(separator: ".").map(String.init)
            return ["id": id, "kind": parts.first ?? "", "card": parts.count > 1 && parts[0] != "free" ? parts[1] : NSNull(),
                    "x": Double(f.minX), "y": Double(f.minY), "w": Double(f.width), "h": Double(f.height)]
        }
    }

    static func rowFit() -> [ControlRow] {
        #if os(macOS)
        let font = NSFont.systemFont(ofSize: WhooshTypography.body.size)
        return FormFitLog.shared.entries.filter { $0.key.hasPrefix("\(base).fit.") }.sorted { $0.key < $1.key }.map { id, e in
            let longest = e.title.split(whereSeparator: { $0 == " " || $0 == "\n" })
                .map { ($0 as NSString).size(withAttributes: [.font: font]).width }.max() ?? 0
            return ["id": id, "title": e.title, "label_width": Double(e.labelWidth), "longest_word": Double(longest.rounded(.up)), "stacked": e.stacked]
        }
        #else
        return []
        #endif
    }
}
