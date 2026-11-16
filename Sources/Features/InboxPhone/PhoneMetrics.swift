import SwiftUI
#if canImport(UIKit)
import UIKit
#elseif canImport(AppKit)
import AppKit
#endif

/// iPhone inbox geometry, read off the picked boards: layout iphone-layout-3/mail-26 (boxes) and style
/// iphone-style/glass-light (CSS `.rb` `.bbar` `.spill` `.r` `.tile` `.track` `.undo` `.edgeB`). 1 CSS px = 1 pt.
/// Finding for the root: these belong in a phone extension file under Sources/Design (frozen for this lane).
enum PhoneMetrics {
    static let gutter = WhooshSpacing.s16
    static let roundButton: CGFloat = 44          // `.rb`, Apple's 44 pt target
    static let selectWidth: CGFloat = 86          // `.pb` Select / Edit
    static let barButton: CGFloat = 52            // `.bbar .rb`
    static let barGap = WhooshSpacing.s8          // 16 + 52 + 8 = the pill's left edge, 76
    static let searchPill: CGFloat = 250          // `.spill` width at rest
    static let searchPillOpen: CGFloat = 310      // `.qpill` width docked over the keyboard
    static let mic: CGFloat = 36
    static let tile: CGFloat = 56                 // `.tile`
    static let tileSmall: CGFloat = 40            // `.tile.s40` on the Mailboxes screen
    static let tileRadius: CGFloat = 14
    static let tileRadiusSmall: CGFloat = 10
    static let tileStroke: CGFloat = 2
    static let tileStrokeSmall: CGFloat = 1.5
    static let tileGlyph: CGFloat = 27
    static let tileGlyphSmall: CGFloat = 20
    static let tileBadge: CGFloat = 10            // `.tile.bad::after`
    static let rowLeading: CGFloat = 90           // `.r .in` padding-left: tile 22 + 56 + 12
    static let tileX: CGFloat = 22
    static let tileY: CGFloat = 12
    static let rowTop: CGFloat = 11
    static let rowBottom: CGFloat = 13
    static let unreadDot: CGFloat = 10            // `.r .ud`
    static let unreadX: CGFloat = 7
    static let unreadY: CGFloat = 20
    static let swipeWidth: CGFloat = 110          // `.r .sw`: the word sits inside it; release past it commits
    static let previewLines = 3
    static let trackHeight: CGFloat = 36          // `.track`
    static let segmentHeight: CGFloat = 32
    static let segmentPad: CGFloat = 13
    static let titleTop: CGFloat = 50             // large title at y 112 under the 62 pt safe area
    static let edgeFade: CGFloat = 150            // `.edgeB` scroll-edge fade to paper
    static let undoWidth: CGFloat = 270           // `.undo`
    static let groupRow: CGFloat = 52             // `.gr`
    static let accountRow: CGFloat = 56           // `.ar`
    static let mailboxRow: CGFloat = 44           // `.mr`
    static let cardRadius: CGFloat = 24           // `.cardL`
    static let tick: CGFloat = 24                 // `.tick`
    static let listBottomRoom: CGFloat = 120      // the last row clears the bar and the undo capsule
}

/// The phone's type roles: each is the iOS Dynamic Type default (Large) for the Mac role it twins
/// (glass-light board "Type scale": Mac 13 body = phone 17 body).
enum PhoneType {
    static let largeTitle = TextStyleToken(size: 34, lineHeight: 41, weight: .bold, emphasis: .bold, tracking: 0.37, monospaced: false)
    static let title3 = TextStyleToken(size: 20, lineHeight: 25, weight: .semibold, emphasis: .semibold, tracking: 0, monospaced: false)
    static let headline = TextStyleToken(size: 17, lineHeight: 22, weight: .semibold, emphasis: .semibold, tracking: 0, monospaced: false)
    static let body = TextStyleToken(size: 17, lineHeight: 22, weight: .regular, emphasis: .semibold, tracking: 0, monospaced: false)
    static let subhead = TextStyleToken(size: 15, lineHeight: 20, weight: .regular, emphasis: .semibold, tracking: 0, monospaced: false)
    static let footnote = TextStyleToken(size: 13, lineHeight: 18, weight: .regular, emphasis: .semibold, tracking: 0, monospaced: false)
}

/// The phone's grounds. iOS: the system's own semantic colours, which are the board's values (white / system black list,
/// F2F2F7 / black grouped, the 12 % tertiary fill). The Mac-hosted preview falls back to the nearest Whoosh token.
/// Finding for the root: a phone ground token belongs in Sources/Design.
enum PhoneGround {
    static var list: Color {
        #if canImport(UIKit)
        Color(uiColor: .systemBackground)
        #else
        Color(nsColor: NSColor(name: nil) { $0.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua ? .black : .white })
        #endif
    }
    /// The list ground resolved for one scheme (gradients resolve their stops once, so they take this form).
    static func list(_ scheme: ColorScheme) -> Color {
        #if canImport(UIKit)
        Color(uiColor: UIColor.systemBackground.resolvedColor(with: UITraitCollection(userInterfaceStyle: scheme == .dark ? .dark : .light)))
        #else
        scheme == .dark ? Color(nsColor: .black) : WhooshPalette.background.color(.light)
        #endif
    }
    static var grouped: Color {
        #if canImport(UIKit)
        Color(uiColor: .systemGroupedBackground)
        #else
        Color(nsColor: NSColor(name: nil) { $0.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua ? .black : .windowBackgroundColor })
        #endif
    }
    static var card: Color {
        #if canImport(UIKit)
        Color(uiColor: .secondarySystemGroupedBackground)
        #else
        Color(nsColor: .controlBackgroundColor)
        #endif
    }
    static var trackFill: Color {
        #if canImport(UIKit)
        Color(uiColor: .tertiarySystemFill)
        #else
        Color(nsColor: .quaternaryLabelColor)
        #endif
    }
    /// `--snz` graphite, the same in both schemes: the Whoosh key-cap dark value.
    static let snooze = ThemeColor(light: WhooshPalette.keyCap.dark, dark: WhooshPalette.keyCap.dark)
}
