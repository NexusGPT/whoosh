import SwiftUI

/// The palette's measures, every one derived from WhooshMetrics / WhooshSpacing tokens (no literal here).
/// The card is one reading measure wide; the field is a glass pill plus its padding; a row is the pickers' row.
/// Token request for the root (docs/lanes/palettemac.md "## Round 2"): a named palette width and field height.
enum PaletteMacMetrics {
    static let width = WhooshMetrics.readerMeasure
    static let edgeGutter = WhooshSpacing.s24
    /// Gap above the card and the least gap kept below it.
    static let verticalGap = WhooshMetrics.readerTop
    static let fieldHeight = WhooshMetrics.glassPillHeight + WhooshSpacing.s16
    static let rowHeight = WhooshMetrics.sidebarRow + WhooshSpacing.s4
    static let rowGap = WhooshSpacing.s2
    static let headerHeight = WhooshMetrics.listHeaderHeight
    static let footerHeight = WhooshMetrics.keyCap + WhooshSpacing.s12
    static let listPadding = WhooshSpacing.s8
    static let emptyHeight = WhooshMetrics.sidebarRow * 2
    /// Leading slot of a jump row: the compact emblem fits it, the other marks centre in it.
    static let markSlot = EmblemSize.compact.diameter + WhooshSpacing.s2

    /// The list's height: its content, never more than the window leaves after the field and the footer.
    static func listHeight(rows: Int, headers: Int, windowHeight: CGFloat) -> CGFloat {
        let content = rows == 0
            ? emptyHeight + listPadding * 2
            : CGFloat(rows) * (rowHeight + rowGap) + CGFloat(headers) * headerHeight + listPadding * 2
        let room = windowHeight - verticalGap * 2 - fieldHeight - footerHeight - WhooshMetrics.separator * 2
        return max(rowHeight, min(content, room))
    }
}
