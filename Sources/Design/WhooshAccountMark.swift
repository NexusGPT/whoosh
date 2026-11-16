import CoreGraphics

/// The account mark: a 1.5 pt ring in the account's colour holding a monochrome icon. The ring is also
/// the account's state (solid = live; arc = first sync; dashed + orange dot = needs a sign-in).
/// Board: native-glass v2 `.mk` CSS and its `mark(a, size)` calls; the colours are account data.
public enum WhooshAccountMark {
    /// Ring diameters by place, points.
    public enum Size {
        /// The sidebar's account grid (docs/CONSTANTS.md ACCOUNT_RING_MIN_PX).
        public static let sidebar = CGFloat(WhooshConstants.ACCOUNT_RING_MIN_PX)
        /// The sidebar's compact groups and the settings account list.
        public static let sidebarCompact = CGFloat(WhooshConstants.ACCOUNT_MARK_COMPACT_PX)
        /// Every list row, the reader's meta line, the failing-account banner.
        public static let row = CGFloat(WhooshConstants.ACCOUNT_MARK_ROW_PX)
        /// Beside the quick reply's from-address.
        public static let replyFrom = CGFloat(WhooshConstants.ACCOUNT_MARK_REPLY_PX)
        /// Settings: Accounts detail header.
        public static let settings = CGFloat(WhooshConstants.ACCOUNT_MARK_SETTINGS_PX)
    }
    /// `.mk` ring stroke; `.mk.big` the 56 pt settings mark.
    public static let stroke: CGFloat = 1.5
    public static let strokeLarge: CGFloat = 2.5
    /// The account-icon slot: the icon is centred and drawn at this share of the ring's diameter.
    public static let iconRatio = CGFloat(WhooshConstants.ACCOUNT_ICON_RATIO)
    public static func iconSlot(_ diameter: CGFloat) -> CGFloat { diameter * iconRatio }
    /// Stroke for a ring of this diameter.
    public static func stroke(_ diameter: CGFloat) -> CGFloat { diameter >= Size.settings ? strokeLarge : stroke }

    /// `.mk.bad` the needs-sign-in ring: dash and gap length (docs/CONSTANTS.md ACCOUNT_RING_DASH_PX).
    public static let dash = CGFloat(WhooshConstants.ACCOUNT_RING_DASH_PX)
    /// `.mk.bad::after` the failing-account dot: 6 pt, 3 pt outside the top-right, 1.5 pt background halo.
    public static let badgeDot: CGFloat = 6
    public static let badgeOutset: CGFloat = 3
    public static let badgeHalo: CGFloat = 1.5
    /// `.mk.pick` the chosen mark: a 2 pt background gap, then a 1.5 pt accent ring (3.5 − 2).
    public static let pickGap: CGFloat = 2
    public static let pickWidth: CGFloat = 1.5
    /// `.mk.pend` first-sync arc: clockwise from 12 o'clock over the `separator` track.
    public static let arcStartDegrees: Double = -90
    /// `.r .mk` a row's mark: x 16, y 5 inside the row.
    public static let rowOrigin = CGPoint(x: 16, y: 5)
    /// `.rings` gap between marks (compact 3).
    public static let gap: CGFloat = 4
    public static let gapCompact: CGFloat = 3

    /// Settings icon library (`.lib`): 10 columns of 36 pt tiles, 6 apart, 18 pt glyphs; colour swatches 20.
    public static let libraryColumns = 10
    public static let libraryTile: CGFloat = 36
    public static let libraryGap: CGFloat = 6
    public static let libraryGlyph: CGFloat = 18
    public static let swatch: CGFloat = 20
}
