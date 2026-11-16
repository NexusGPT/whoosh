import SwiftUI

/// The welcome screens' geometry. Every number is private to this folder and listed in docs/lanes/welcomephone.md for the ledger.
/// Widths, gutters, type roles and the 44 pt target come from InboxPhone's `PhoneMetrics` and `PhoneType`, never retyped here.
enum WelcomeMetrics {
    static let markSize: CGFloat = 112             // the glass mark on a one-step screen
    static let markSmall: CGFloat = 72             // the same mark above the arriving list
    static let mainButton = PhoneMetrics.barButton // 52: the one main button
    static let textButton = PhoneMetrics.roundButton // 44: the quiet second action
    static let frameSide: CGFloat = 248            // the viewfinder square
    static let frameBracket: CGFloat = 40          // the length of one corner bracket
    static let frameStroke: CGFloat = 4            // the corner bracket's line
    static let frameRadius: CGFloat = 28           // the viewfinder's corner
    static let blockGap = WhooshSpacing.s12        // mark, title and line spacing unit
    static let footBottom = WhooshSpacing.s24      // under the main button
    static let arrivalStepMs = 45                  // one account lands every 45 ms (30 accounts: about 1.4 s)
    static let arrivalRise: CGFloat = 8            // a landing row rises this far
    static let contentMaxWidth: CGFloat = 480      // text and list width on a wide phone
}
