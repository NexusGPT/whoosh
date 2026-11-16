import SwiftUI

/// Where each key badge sits, so none lands on text or on another badge (owner 2026-11-08 16:02 rejected overlaps):
/// tabs: hanging above the tab's top edge (its count stays readable); sidebar rows: in the count's slot at the row's trailing end,
/// replacing the count while ⌘ is held; row actions: over their own icon, spread apart so neighbours never touch; toolbar: a row
/// of caps directly under the icons, one per icon, centred (drawn by the toolbar; frames computed here from the same numbers).
@MainActor
enum MacHintPlace {
    static let h = WhooshMetrics.keyCap
    static let gap: CGFloat = 2

    /// The cap's size as KeyCap lays it out: its minimum, else the glyphs plus the side padding.
    static func size(_ label: String) -> CGSize {
        let g = KeyCap.glyph(for: label)   // the cap draws the glyph ("Enter" is one ↩), so its width follows the glyph
        return CGSize(width: max(h, CGFloat(g.filter { $0 != " " }.count) * 7.5 + 12 + (g.contains(" ") ? 6 : 0)), height: h)
    }

    /// The row strip's caps: its icons sit 22 pt apart, so its caps are the compact size (one glyph 16, two glyphs 19), under their icons.
    static let compactH = CGFloat(WhooshConstants.CAP_H)   // 9: what fits between the row above's subject and the icons' ink
    static let compactFont: CGFloat = 8
    static func compactSize(_ label: String) -> CGSize {
        CGSize(width: max(compactH, CGFloat(KeyCap.glyph(for: label).count) * 5 + 4), height: compactH)
    }
    /// half a point between a cap's bottom edge and the highest icon ink, so the cap never covers a glyph
    static let inkGap: CGFloat = 0.5

    static func place(_ hints: [MacHint]) -> [MacHint] {
        let w = MacShellState.shared.paneWidths
        let listRight = (w[.sidebar] ?? 0) + (w[.list] ?? 0)   // a tab badge stays inside the list column
        let pill = MacShellGeometry.mutant == "pill"   // the ms13 form, kept only as the mutant the check must turn red on
        let out = hints.filter { !pill || !$0.ax.contains(".act.") }.map { hint -> MacHint in
            let s = hint.ax.contains(".act.") ? compactSize(hint.label) : size(hint.label), r = hint.rect
            var o = CGPoint(x: r.midX - s.width / 2, y: r.midY - s.height / 2)   // centred on the control: the mutant's placement
            if MacShellGeometry.mutant != "no-offset" {
                if hint.ax.hasPrefix("mac-main.splits.") { o = CGPoint(x: min(r.midX, listRight - s.width / 2 - 4) - s.width / 2, y: r.minY - s.height) }
                else if hint.ax.hasPrefix("mac-main.sidebar.") { o = CGPoint(x: r.maxX - 2 - s.width, y: r.midY - s.height / 2) }
                // owner 2026-11-08 23:08 ("floating better"): each row cap floats directly above its own icon; the icons stay on the
                // row's first line where they are, so the strip keeps its width and the sender keeps its room. The caps may sit over
                // the bottom of the row above (its trailing pin/clip slot), never over its sender or subject.
                else if hint.ax.contains(".act.") {
                    // lead ruling 23:20: lowered into the icons' top padding, down to the highest ink of the strip, so the cap clears
                    // the row above's subject text by at least 1 pt and covers no glyph. Mutant high-caps: the ms14 height, above the frame.
                    let bottom = MacShellGeometry.mutant == "high-caps" ? r.minY - 1
                        : r.minY + MacHintGlyphs.stripInkTop(frameHeight: r.height) - inkGap
                    o = CGPoint(x: r.midX - s.width / 2, y: bottom - s.height)
                }
                else { o = CGPoint(x: r.maxX - s.width, y: r.minY - s.height + 2) }
            }
            return MacHint(ax: hint.ax, label: hint.label, rect: CGRect(origin: o, size: s))
        }
        return out
    }

    /// MUTANT ONLY (WHOOSH_MUTANT_R8=pill): the ms13 form the owner replaced at 23:08, one pill of icon + cap pairs on the row's
    /// first line. Kept so par-macshell can prove its row-cap checks turn red on it.
    static func rowStrips(_ acts: [MacHint]) -> (caps: [MacHint], strips: [MacHintStrip]) {
        guard MacShellGeometry.mutant == "pill" else { return ([], []) }
        let iconW: CGFloat = 11, capGap: CGFloat = 2, pairGap: CGFloat = 5, padX: CGFloat = 6, pillH: CGFloat = 24
        var groups: [String: [MacHint]] = [:]
        for a in acts where a.ax.contains(".act.") { groups[String(a.ax[..<a.ax.range(of: ".act.")!.lowerBound]), default: []].append(a) }
        var caps: [MacHint] = [], strips: [MacHintStrip] = []
        for (row, list) in groups.sorted(by: { $0.key < $1.key }) {
            let items = list.sorted { $0.rect.minX < $1.rect.minX }
            let widths = items.map { iconW + capGap + compactSize($0.label).width }
            let total = widths.reduce(0, +) + pairGap * CGFloat(items.count - 1) + 2 * padX
            let right = items.last!.rect.maxX + 1
            let midY = items.map { $0.rect.midY }.reduce(0, +) / CGFloat(items.count)
            let pill = CGRect(x: right - total, y: midY - pillH / 2, width: total, height: pillH)
            var x = pill.minX + padX
            var pairs: [MacHintStrip.Pair] = []
            for (i, it) in items.enumerated() {
                let id = String(it.ax[it.ax.range(of: ".act.")!.upperBound...])
                let cs = compactSize(it.label)
                let cap = MacHint(ax: it.ax, label: it.label, rect: CGRect(x: x + iconW + capGap, y: midY - cs.height / 2, width: cs.width, height: cs.height))
                pairs.append(.init(symbol: RowStrip.items.first { $0.id == id }?.symbol ?? "circle", icon: CGRect(x: x, y: midY - iconW / 2, width: iconW, height: iconW), cap: cap))
                caps.append(cap)
                x += widths[i] + pairGap
            }
            strips.append(MacHintStrip(id: row, pill: pill, pairs: pairs))
        }
        return (caps, strips)
    }

    /// The toolbar's caps, from the icon row's frame: one per icon, centred in its cell, directly under the icons.
    static func toolbarCaps(iconRow r: CGRect) -> [MacHint] {
        MacToolbarItem.all.enumerated().compactMap { i, item in
            guard let k = MacHintTable.keyLabel(item.command) else { return nil }
            let s = size(k)
            let cx = r.minX + WhooshSpacing.s4 + CGFloat(i) * (MacToolbarView.cell + WhooshSpacing.s2) + MacToolbarView.cell / 2
            let y = MacShellGeometry.mutant == "no-offset" ? r.midY - s.height / 2 : r.maxY + 1
            return MacHint(ax: "mac-main.toolbar.cap.\(item.command)", label: k, rect: CGRect(x: cx - s.width / 2, y: y, width: s.width, height: s.height))
        }
    }
}

/// One row's strip while ⌘ is held: the pill and its icon + cap pairs, in window points.
struct MacHintStrip: Identifiable {
    struct Pair { let symbol: String; let icon: CGRect; let cap: MacHint }
    let id: String
    let pill: CGRect
    let pairs: [Pair]
}
