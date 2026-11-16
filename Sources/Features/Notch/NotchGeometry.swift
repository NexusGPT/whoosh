import CoreGraphics
import Foundation
#if os(macOS)
import AppKit
#endif

/// The notch's layout sizes, derived from the design tokens where a token exists.
/// The three own values are listed in docs/lanes/notch.md as rows the Design lane may adopt.
public enum NotchMetrics {
    // Look A, "Continuum" (docs/design/boards/notch/a, the owner's night default 2026-11-08): the sizes the board draws,
    // each a docs/CONSTANTS.md row (the contracts lane adds the eight NOTCH_* rows; WhooshConstants is generated from the ledger).
    public static let cardWidth = CGFloat(WhooshConstants.NOTCH_ARRIVAL_W)
    public static let listWidth = CGFloat(WhooshConstants.NOTCH_LIST_W)
    public static let openWidth = CGFloat(WhooshConstants.NOTCH_OPEN_W)
    /// A new mail as one line (owner's picks of 14 Nov 2026): the notch this wide, the housing's height.
    public static let lineWidth = CGFloat(WhooshConstants.NOTCH_LINE_W)
    /// A new mail as a light: the height of the light on the notch's lower edge.
    public static let lightHeight = CGFloat(WhooshConstants.NOTCH_LIGHT_H)
    /// The search field on the list's first line.
    public static let searchField = CGFloat(WhooshConstants.NOTCH_SEARCH_H)
    /// A row of the search's answer: a list row with a third line (the passage that made the mail a result).
    public static let searchRow = CGFloat(WhooshConstants.NOTCH_SEARCH_ROW_H)
    /// The open mail's one-line strips: what is folded, and the line that holds Full mail or Its words.
    public static let foldLine = CGFloat(WhooshConstants.NOTCH_FOLD_LINE_H)
    /// Rest: the housing widens by this on each side (the account of the newest unread mail left, the unread count right).
    public static let restWing = CGFloat(WhooshConstants.NOTCH_REST_WING)
    public static let restShoulder = CGFloat(WhooshConstants.NOTCH_REST_SHOULDER_R)                                      // board a draws the wings with a smaller flare
    /// The band beside the camera: as tall as the housing; its content sits in the wings, never under the camera.
    public static let bandInset: CGFloat = WhooshSpacing.s16                           // the band's side padding (board: 16/18)
    public static let bodyInset: CGFloat = WhooshSpacing.s16 + WhooshSpacing.s2        // the body's side padding (board: 18)
    public static let bodyTop: CGFloat = WhooshSpacing.s8                              // between the band and the body (board: 40 - 32)
    public static let bodyBottom: CGFloat = WhooshSpacing.s14
    public static let row = CGFloat(WhooshConstants.NOTCH_ROW_H)                       // a list row: two lines and a pill (board)
    public static let rowCorner = CGFloat(WhooshConstants.NOTCH_ROW_R)
    public static let codeBlock = CGFloat(WhooshConstants.NOTCH_CODE_BLOCK_H)          // the arrival's code block (board)
    public static let codeCorner = CGFloat(WhooshConstants.NOTCH_CODE_R)
    public static let button = CGFloat(WhooshConstants.NOTCH_BUTTON_H)                                            // Copy and Fill (board)
    public static let smallButton: CGFloat = WhooshMetrics.actionButton                                       // Archive, Mark read, the attachment chip (board)
    public static let doneRow = CGFloat(WhooshConstants.NOTCH_DONE_ROW_H)                                           // "That's everything" under the Unread list (board)
    public static let replyCorner = CGFloat(WhooshConstants.NOTCH_REPLY_R)
    public static let segment = CGFloat(WhooshConstants.NOTCH_SEGMENT_H)                                           // Latest | Unread (board)
    static let expiryGrace = TimeInterval(WhooshConstants.NOTCH_UNDO_EXPIRY_GRACE_MS) / 1000                                     // how close to the window's end a cancelled wait still counts as the end
    public static let listMax = WhooshConstants.NOTCH_LIST_MAX                                                    // rows the hover list shows before "N more"
    public static let restStrip: CGFloat = WhooshSpacing.s8                           // hover strip under the menu bar, no-notch Macs
    public static let corner: CGFloat = WhooshRadius.window
    public static let edge: CGFloat = WhooshSpacing.s12
    public static let shoulder = CGFloat(WhooshConstants.NOTCH_SHOULDER_R)           // the concave flare into the top edge (board A)
    public static let openCorner = CGFloat(WhooshConstants.NOTCH_CORNER_R)           // board A's bottom corner
    public static let restCorner: CGFloat = WhooshRadius.hint                         // the housing's own corner; the wings' corner (board: 11)
    public static let lip: CGFloat = WhooshSpacing.s6                                 // light: the black lip under the housing
    public static let windowHeight = CGFloat(WhooshConstants.NOTCH_CANVAS_H)         // the panel's fixed canvas; the shape grows inside it
    public static let socketRows = WhooshConstants.NOTCH_SOCKET_ROWS                                                // row ids the socket can click (the list holds no more)
    /// A mail younger than this says "now" instead of its clock time.
    public static let nowWindowS = TimeInterval(WhooshConstants.NOTCH_NOW_WINDOW_S)
}

/// Where the panel goes on one screen. Written from AppKit's own screen facts: the top safe-area inset is the
/// housing's height, and the gap between the two auxiliary top areas is its width and position.
public struct NotchGeometry: Equatable, Sendable {
    public var shape: NotchShape
    /// The housing in screen coordinates (origin bottom-left); on a Mac without one, a zero-width line the menu bar's height.
    public var housing: CGRect

    public static func frame(width: CGFloat, height: CGFloat, under g: NotchGeometry, screenTop: CGFloat) -> CGRect {
        CGRect(x: (g.housing.midX - width / 2).rounded(), y: screenTop - height, width: width, height: height)
    }

    #if os(macOS)
    /// `fraction` places the shape along the top edge on a Mac without a housing (the owner's option-drag,
    /// NotchDrag); 0.5 is the centre, and the open card always stays whole on the screen. On a notched Mac the
    /// shape is centred on the housing and the fraction places nothing (the owner's fork refuses that drag too).
    public static func read(_ screen: NSScreen, override: NotchShape? = nil, fraction: Double = 0.5) -> NotchGeometry {
        let f = screen.frame
        let top = screen.safeAreaInsets.top
        if override != NotchShape.none, top > 0, let l = screen.auxiliaryTopLeftArea, let r = screen.auxiliaryTopRightArea, r.minX > l.maxX {
            return NotchGeometry(shape: .notch, housing: CGRect(x: l.maxX, y: f.maxY - top, width: r.minX - l.maxX, height: top))
        }
        let menuBar = f.maxY - screen.visibleFrame.maxY
        let half = NotchMetrics.openWidth / 2 + NotchMetrics.shoulder + NotchMetrics.edge
        let x = min(max(f.minX + f.width * CGFloat(fraction), f.minX + half), f.maxX - half)
        return NotchGeometry(shape: .none, housing: CGRect(x: x, y: f.maxY - menuBar, width: 0, height: menuBar))
    }

    /// The screen with the menu bar: the notch belongs to the built-in display, which is `NSScreen.screens[0]` when it leads.
    @MainActor public static var screen: NSScreen? {
        NSScreen.screens.first { $0.safeAreaInsets.top > 0 } ?? NSScreen.main ?? NSScreen.screens.first
    }
    #endif
}
