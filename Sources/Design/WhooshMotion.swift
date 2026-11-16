import SwiftUI

/// Restrained motion: only state changes move, nothing breathes, glows or loops.
/// Board: native-glass v2 "Motion" table; docs/surfaces/mac-main.md `motion:`; reader/conversation "Motion".
public enum WhooshMotion {
    /// Seconds, read from docs/CONSTANTS.md (milliseconds there).
    private static func ms(_ v: Int) -> Double { Double(v) / 1000 }
    public static let readerCrossFade = ms(WhooshConstants.READER_CROSSFADE_MS)  // Enter swaps the reader in place
    public static let tabMorph = ms(WhooshConstants.TAB_MORPH_MS)               // the split thumb, the signature move
    public static let rowCollapse = ms(WhooshConstants.ROW_COLLAPSE_MS)         // E / H archive: the row collapses to 0 height
    public static let rowInsert = ms(WhooshConstants.ROW_INSERT_MS)             // a new message slides the list down one row
    public static let hoverTint = ms(WhooshConstants.HOVER_TINT_MS)             // hover tints a row, reveals its four actions
    /// A mail that leaves the list (E, H, L, P, a closing filter) slides left and dissolves, and the mails below close the gap
    /// (owner 2026-11-11: "on superhman they go on the left and dissolve gently", "but fast"). The four numbers are the ROW_EXIT_* rows of docs/CONSTANTS.md.
    /// The whole card slides `rowExitSlidePt` points left (a value of 0 or less means the row's own width: the card leaves the list).
    /// The defaults are the drawing L2 of round 3 (docs/lanes/done-motion.md); the owner picked L2 on 2026-11-11.
    public static let rowExitSlidePt = CGFloat(WhooshConstants.ROW_EXIT_SLIDE_PT)
    public static let rowExitSlideMs = WhooshConstants.ROW_EXIT_SLIDE_MS
    public static let rowExitCloseDelayMs = WhooshConstants.ROW_EXIT_CLOSE_DELAY_MS
    public static let rowExitCloseMs = WhooshConstants.ROW_EXIT_CLOSE_MS
    /// The whole exit: the slide, or the delay plus the gap closing, whichever ends last (160 ms by default).
    public static var rowExitTotalMs: Int { max(rowExitSlideMs, rowExitCloseDelayMs + rowExitCloseMs) }
    /// One linear clock over the whole exit; `RowExitEffect` shapes the slide and the gap-close curves from it by hand,
    /// so a leaving row carries ONE animation, not three.
    public static var rowExit: Animation { .linear(duration: ms(rowExitTotalMs)) }

    /// The split thumb's spring: response 0.22, damping 0.8 (board "Signature").
    public static let tabMorphDamping = WhooshConstants.TAB_MORPH_DAMPING
    /// The undo capsule rises this far into the foot strip.
    public static let undoRise = CGFloat(WhooshConstants.UNDO_RISE_PX)

    public static var readerSwap: Animation { .easeInOut(duration: readerCrossFade) }
    public static var tabThumb: Animation { .spring(response: tabMorph, dampingFraction: tabMorphDamping) }
    public static var archiveCollapse: Animation { .easeOut(duration: rowCollapse) }
    public static var newRow: Animation { .easeOut(duration: rowInsert) }
    public static var hover: Animation { .easeOut(duration: hoverTint) }
    /// Folded messages and "5 earlier messages" open by height on the system spring.
    public static var unfold: Animation { .spring }
    /// J / K selection moves with no animation at all: speed is the response.
    public static let selection: Animation? = nil

    /// The animation to use, or none when the person asked the system to reduce motion.
    public static func animation(_ animation: Animation?, reduceMotion: Bool) -> Animation? {
        reduceMotion ? nil : animation
    }
}
