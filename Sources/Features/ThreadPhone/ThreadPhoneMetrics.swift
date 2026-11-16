import SwiftUI

/// The thread screen's geometry. It reuses the inbox's metrics (`PhoneMetrics`: gutter, 44 pt targets, the bar buttons,
/// the card radius) and adds only what a conversation needs. Each number below is private to this screen and is listed
/// in docs/lanes/threadphone.md for the constants ledger.
enum ThreadMetrics {
    /// The subject, set as a large title one step under the inbox's 34 pt so a long subject keeps to two lines.
    static let subject = TextStyleToken(size: 28, lineHeight: 34, weight: .bold, emphasis: .bold, tracking: 0.36, monospaced: false)
    /// The code in the pill: one step up from the body, bold, in tabular figures (whooshText already sets them).
    static let code = TextStyleToken(size: 20, lineHeight: 25, weight: .semibold, emphasis: .semibold, tracking: 1.5, monospaced: false)
    /// Space between the cards of the conversation.
    static let cardGap = WhooshSpacing.s12
    /// Inner padding of a card.
    static let cardPad = WhooshSpacing.s16
    /// A folded earlier message shows its first two lines (the Mac's folded message shows two as well).
    static let foldedLines = 2
    /// The edge-back gesture starts inside this many points of the leading edge and commits past `edgeBackPast`.
    static let edgeBackStart: CGFloat = 24
    static let edgeBackPast: CGFloat = 80
    /// The code pill is one 44 pt target tall (Apple's minimum), the inbox's `roundButton`.
    static let pillHeight = PhoneMetrics.roundButton
    /// The attachment chip.
    static let chipHeight: CGFloat = 36
    /// The More panel's width (a glass card above the bar's trailing end).
    static let menuWidth: CGFloat = 230
    /// The More panel grows from this scale at its bottom-trailing corner.
    static let panelFrom: CGFloat = 0.9
    /// The edge-back gesture's own minimum drag before it reads a direction.
    static let edgeBackMin: CGFloat = 16
    /// The note sheet's editor minimum height.
    static let noteHeight: CGFloat = 160
}
