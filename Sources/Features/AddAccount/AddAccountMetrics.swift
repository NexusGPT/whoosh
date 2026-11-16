import SwiftUI

/// The sheet's geometry and pacing, from the confirmed board (docs/design/boards/addaccount-r3/mix, aas1). Geometry is the
/// board's CSS in points; the pacing numbers are docs/CONSTANTS.md rows (ADD_ACCOUNT_*).
enum AddAccountMetrics {
    /// A window narrower than this draws the board's 960 x 632 sizes.
    static let narrowBelow: CGFloat = 1100
    static func narrow(_ windowWidth: CGFloat) -> Bool { windowWidth < narrowBelow }
    static func width(_ narrow: Bool) -> CGFloat { narrow ? 520 : 560 }
    static func top(_ narrow: Bool) -> CGFloat { narrow ? 62 : 72 }
    static func fieldHeight(_ narrow: Bool) -> CGFloat { narrow ? 42 : 48 }
    static let radius = WhooshRadius.sheet
    static let paddingTop: CGFloat = 24, paddingSide: CGFloat = 24, paddingBottom: CGFloat = 20
    static let headChip: CGFloat = 36, addedChip: CGFloat = 44, fieldChip: CGFloat = 20, foundChip: CGFloat = 16, sessionChip: CGFloat = 18, rowChip: CGFloat = 20
    static let fieldRadius = WhooshRadius.field, fieldGap: CGFloat = 8
    static let smallField: CGFloat = 30, portWidth: CGFloat = 66, portWidthNarrow: CGFloat = 58
    static let buttonHeight: CGFloat = 28, defaultButtonMin: CGFloat = 96
    static let hairline: CGFloat = 1, segmentGap: CGFloat = 3

    // Pacing: every number is a docs/CONSTANTS.md row.
    static var popover: Double { Double(WhooshConstants.POPOVER_MS) / 1000 }
    static var crossfade: Double { Double(WhooshConstants.READER_CROSSFADE_MS) / 1000 }
    static var sheenS: Double { Double(WhooshConstants.ADD_ACCOUNT_MARK_SHEEN_MS) / 1000 }
    static var unfoldResponse: Double { WhooshConstants.ADD_ACCOUNT_UNFOLD_RESPONSE_S }
    static var unfoldDamping: Double { WhooshConstants.ADD_ACCOUNT_UNFOLD_DAMPING }
    static var connectingMs: Int64 { Int64(WhooshConstants.ADD_ACCOUNT_CONNECTING_MS) }

    static var arrive: Animation { .timingCurve(0.2, 0.8, 0.2, 1, duration: popover) }
    static var sheen: Animation { .easeOut(duration: sheenS) }
    static var swap: Animation { .easeInOut(duration: crossfade) }
    static var unfold: Animation { .spring(response: unfoldResponse, dampingFraction: unfoldDamping) }
    static func motion(_ a: Animation, _ reduce: Bool) -> Animation? { reduce ? .easeInOut(duration: crossfade) : a }
}

/// The field's own border (the board's `--fldborder`): a hair darker than the separator.
extension WhooshPalette {
    static let sheetFieldBorder = ThemeColor(light: RGBA(0, 0, 0, 0.11), dark: RGBA(255, 255, 255, 0.10))
}
