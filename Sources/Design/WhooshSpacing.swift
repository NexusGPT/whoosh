import CoreGraphics

/// The spacing scale, points, on a 4 pt grid with the board's half steps. Board: native-glass v2 CSS gaps
/// and paddings (`gap:2/4/6/8/10/12/14/16`, reader pad 32, sheet pad 24, list header 20/24).
public enum WhooshSpacing {
    public static let s2: CGFloat = 2
    public static let s4: CGFloat = 4
    public static let s6: CGFloat = 6
    public static let s8: CGFloat = 8
    public static let s10: CGFloat = 10
    public static let s12: CGFloat = 12
    public static let s14: CGFloat = 14
    public static let s16: CGFloat = 16
    public static let s20: CGFloat = 20
    public static let s24: CGFloat = 24
    public static let s32: CGFloat = 32
}

/// Fixed heights and widths the board measures, points. Density: regular (docs/DESIGN.md).
public enum WhooshMetrics {
    /// `.r` a two-line list row, sender + subject, no preview; each line 18 (`.l1` `.l2`).
    public static let listRow = CGFloat(WhooshConstants.LIST_ROW_PX)
    public static let listRowLine: CGFloat = 18
    /// `.r` inset from the list edge and its leading pad that leaves room for the 16 pt mark (v2: 40).
    public static let listRowInset: CGFloat = 8
    public static let listRowLeading: CGFloat = 40
    /// `.r .ud` unread dot: 7 pt at x 5, y 11.
    public static let unreadDot: CGFloat = 7
    /// `.sbr` `.sbh` `.grp` sidebar rows and headers; `.aci` account items.
    public static let sidebarRow: CGFloat = 24
    public static let accountItem: CGFloat = 22
    /// `.sb .in` sidebar content top (clears the traffic lights) and side pad.
    public static let sidebarTop: CGFloat = 52
    /// `.tb` toolbar controls: top 12, height 32; `.seg span` segment 28.
    public static let toolbarTop: CGFloat = 12
    public static let glassPillHeight: CGFloat = 32
    public static let segmentHeight: CGFloat = 28
    /// `.lh` list header: top 52, height 28.
    public static let listHeaderHeight: CGFloat = 28
    /// `.foot` the list's reserved foot strip the undo capsule docks in; also conversation `.ks`.
    public static let footStrip = CGFloat(WhooshConstants.FOOT_STRIP_PX)
    /// `.r .acts span` `.meta .ia span` row and meta actions; `.qr .send` send button.
    public static let actionButton: CGFloat = 28
    public static let actionIcon: CGFloat = 16
    /// `.rd .pad` reader content: 32 pt sides, 60 pt top; `.cv-c` conversation column bottom 48.
    public static let readerSide: CGFloat = 32
    public static let readerTop: CGFloat = 60
    /// reader/conversation `.bk` block: 500 wide, pad 8 / 14 / 10, 8 between blocks.
    public static let blockWidth: CGFloat = 500
    public static let blockGap: CGFloat = 8
    /// `.av` sender avatar in the reader; `.body` body measure (≤ 540 pt).
    public static let avatar: CGFloat = 32
    public static let readerMeasure: CGFloat = 540
    /// `.qr .fld` quick-reply field; `.att` attachment chip; conversation `.ft` file tile.
    public static let quickReplyField: CGFloat = 72
    public static let attachment: CGFloat = 48
    public static let fileTile: CGFloat = 36
    /// `.kc` key cap: min width and height 20.
    public static let keyCap: CGFloat = 20
    /// `.bar` progress bar height; `.dot` warning dot.
    public static let progressBar: CGFloat = 4
    public static let warningDot: CGFloat = 6
    /// `.sep` separators: 1 pt.
    public static let separator: CGFloat = 1
}
