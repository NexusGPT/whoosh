import SwiftUI

/// The iPhone Settings screen's own sizes. Every number is private to this screen and named here once; the notes list them
/// for the ledger (docs/lanes/settingsphone.md "Constants"). Geometry shared with the inbox comes from `PhoneMetrics`.
enum SettingsPhoneMetrics {
    static let paneIcon: CGFloat = 30            // the coloured rounded square of a row (iOS Settings uses 29 to 30 pt)
    static let paneIconRadius: CGFloat = 7       // the same square's corner, continuous
    static let heroIcon: CGFloat = 56            // the Sync page's cloud square: the PhoneMetrics tile size, one rhythm
    static let heroIconRadius: CGFloat = 14      // PhoneMetrics.tileRadius
    static let row: CGFloat = 52                 // a row with an icon: 44 pt target plus 4 pt of air above and below
    static let choiceRow: CGFloat = 48           // a row of a choice list (language, appearance)
    static let markStack = 5                              // the summary card shows the first five account marks, as the Mac's does
    static let markOverlap: CGFloat = 12         // each mark of the stack covers 12 pt of the one before
    static let bar: CGFloat = 4                  // a download's bar, as the Mac's `.bar`
    static let titleBottom: CGFloat = 8          // between a large title and its first card
    static let retryHeight: CGFloat = 44         // Apple's target; the Retry capsule
    static let retryWidth: CGFloat = 120         // wide enough for "Réessayer" and "Tentar de novo" at 17 pt
    static let tick: TimeInterval = 30           // the Sync page redraws its "2 min ago" this often (seconds)
    static let pressDim: Double = 0.06           // a pressed row darkens by this much on the input frame
}
