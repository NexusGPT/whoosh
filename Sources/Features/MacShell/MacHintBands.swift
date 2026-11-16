import SwiftUI
#if os(macOS)
import AppKit
#endif

/// The text a row cap must never touch, per visible row, in window points: the sender label (measured, `.text`) and the
/// subject line (ESTIMATED: the list lane publishes no subject frame, so its width is the subject's natural width at the
/// subhead size, stopped before the row's trailing slot; its height is the font's ascender to descender). The trailing slot (labels, star, pin, clip) is reported apart:
/// a cap over it is allowed while ⌘ is held (owner 23:08). Served as `rows mac-main.hints.bands`.
@MainActor
enum MacHintBands {
    static func rows(_ m: AppModel) -> [ControlRow] {
        #if os(macOS)
        let line = WhooshMetrics.listRowLine
        let listRight = (MacShellState.shared.paneWidths[.sidebar] ?? 0) + (MacShellState.shared.paneWidths[.list] ?? 0)
        let font = NSFont.systemFont(ofSize: WhooshTypography.subhead.size)
        let caption = NSFont.systemFont(ofSize: WhooshTypography.caption.size, weight: .semibold)
        return m.rows.compactMap { r -> ControlRow? in
            guard let s = FrameProbe.frame("mac-main.list.row.\(r.messageId).text") else { return nil }
            let icon = WhooshTypography.caption.size + WhooshSpacing.s6
            var trail = CGFloat(r.pinned ? 1 : 0) * icon + CGFloat(r.attachmentCount > 0 ? 1 : 0) * icon
            for l in r.labelNames { trail += (l as NSString).size(withAttributes: [.font: caption]).width + WhooshSpacing.s6 }
            let right = listRight - WhooshSpacing.s12 - WhooshMetrics.listRowInset
            let slotX = right - trail
            let natural = (r.subject as NSString).size(withAttributes: [.font: font]).width
            let subjW = min(natural, slotX - WhooshSpacing.s4 - s.minX)
            // the subject's text box, ascender to descender (not its 18 pt line box), centred in its line under the sender's line
            let textH = font.ascender - font.descender + font.leading
            let y2 = s.minY + line + (line - textH) / 2
            return ["id": r.messageId,
                    "sender": ["x": s.minX, "y": s.minY, "w": s.width, "h": s.height],
                    "subject": ["x": s.minX, "y": y2, "w": max(0, subjW), "h": textH],
                    "trailing": ["x": slotX, "y": y2, "w": max(0, trail), "h": line]]
        }
        #else
        return []
        #endif
    }
}
