import CoreGraphics
import Foundation

/// Every number of the mac-main window chrome, in ONE place until docs/CONSTANTS.md carries them
/// (rows SIDEBAR_W, SIDEBAR_MIN_W, LIST_W, LIST_MIN_W, READER_MIN_W, SIDEBAR_MAX_W, LIST_MAX_W were asked of
/// the lead on 2026-11-07). Each line names its source; the day the ledger lands, these read WhooshConstants.
enum MacShellGeometry {
    /// Mutant switch for the checks (docs/TESTING.md: a check is green only after it is seen red): WHOOSH_MUTANT_R8 names one break.
    static let mutant = ProcessInfo.processInfo.environment["WHOOSH_MUTANT_R8"] ?? ""

    // docs/design/geometry/mac-main.json: window 1280x820, min 960x600
    static let windowW: CGFloat = 1280
    static let windowH: CGFloat = 820
    static let windowMinW: CGFloat = 960
    static let windowMinH: CGFloat = 600

    // native-glass v2 board, index.html:1382: sidebar 220 at 1280 and 200 at 960; list 400 at 1280 and 320 at 960
    // docs/CONSTANTS.md rows SIDEBAR_W_PX, SIDEBAR_W_COMPACT_PX, LIST_W_PX, LIST_W_COMPACT_PX
    static let sidebarW = CGFloat(WhooshConstants.SIDEBAR_W_PX)
    static let sidebarCompactW = CGFloat(WhooshConstants.SIDEBAR_W_COMPACT_PX)
    static let listW = CGFloat(WhooshConstants.LIST_W_PX)
    static let listCompactW = CGFloat(WhooshConstants.LIST_W_COMPACT_PX)
    /// The list column while the sidebar is hidden (owner 2026-11-08 14:52): LIST_W_WIDE_PX, or LIST_W_WIDE_COMPACT_PX at the compact window.
    static let listWideW = CGFloat(WhooshConstants.LIST_W_WIDE_PX)
    static let listWideCompactW = CGFloat(WhooshConstants.LIST_W_WIDE_COMPACT_PX)
    static let sidebarMinW = sidebarCompactW
    static let listMinW = listCompactW
    /// Below this window width the compact widths apply (docs/CONSTANTS.md COMPACT_BELOW_W_PX).
    static let compactBelowW = CGFloat(WhooshConstants.COMPACT_BELOW_W_PX)

    // macshell's own: 960 - 200 - 320 - two 1 px dividers leaves 438; 400 keeps a margin
    static let readerMinW: CGFloat = 400
    static let sidebarMaxW: CGFloat = 320
    static let listMaxW: CGFloat = 560

    // native-glass v2 board .syncl: left 16, right 8, bottom 12, two caption lines
    static let syncLeading: CGFloat = WhooshSpacing.s16
    static let syncTrailing: CGFloat = WhooshSpacing.s8
    static let syncBottom: CGFloat = WhooshSpacing.s12

    // text size steps for ⌘+ and ⌘− (Apple Mail's View > Bigger / Smaller); 1 = the locked ramp. docs/CONSTANTS.md TEXT_SIZE_MIN, _MAX, _STEP
    static let textScales: [CGFloat] = {
        var out: [CGFloat] = [], v = WhooshConstants.TEXT_SIZE_MIN
        while v < WhooshConstants.TEXT_SIZE_MAX - 0.001 { out.append(CGFloat((v * 100).rounded() / 100)); v += WhooshConstants.TEXT_SIZE_STEP }
        out.append(CGFloat(WhooshConstants.TEXT_SIZE_MAX))
        return out
    }()
}
