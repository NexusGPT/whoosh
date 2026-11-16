import SwiftUI

/// The guide's geometry and pacing, from the picked board (docs/design/boards/entra-guide-r1/eg-screenshot-stage). Geometry is the board's
/// CSS in points. Pacing: transform and opacity only, every move at most 260 ms (the MOTION rules); the numbers wait in
/// docs/lanes/entra-guide-build.md "pending constants" for a docs/CONSTANTS.md row.
enum EntraGuideMetrics {
    static let width: CGFloat = 880
    static func top(_ narrow: Bool) -> CGFloat { narrow ? 16 : 36 }
    static let padTop: CGFloat = 20, padSide: CGFloat = 24, padBottom: CGFloat = 18
    static let radius = WhooshRadius.sheet
    static let whereBar: CGFloat = 28
    static let textHeight: CGFloat = 164
    static let valueHeight: CGFloat = 46, pasteHeight: CGFloat = 48, buttonHeight: CGFloat = 28
    static let ringPad: CGFloat = 7, slideBelow: Double = 140, segmentGap: CGFloat = 3
    /// the add-account sheet the guide grows out of (560 wide, 72 from the top) for the entrance
    static let ghostWidth: CGFloat = 560, ghostHeight: CGFloat = 484

    static let enterS = 0.24, fadeS = 0.12, stepHalfS = 0.12, cameraS = 0.26, slideS = 0.22, ringInS = 0.22
    static let haloS = 0.26, tickS = 0.16, auroraS = 0.26, layerFadeS = 0.20, copiedS = 1.6, capInS = 0.18, capDelayS = 0.2

    static func ease(_ s: Double) -> Animation { .timingCurve(0.2, 0.8, 0.2, 1, duration: s) }
    /// Reduced Motion: every move becomes a 1 ms cross-fade.
    static func motion(_ a: Animation, _ reduce: Bool) -> Animation { reduce ? .easeInOut(duration: 0.001) : a }
    static func narrow(_ windowWidth: CGFloat) -> Bool { windowWidth < 1100 }
}

// guideOK, guidePaper, guideAurora and the chrome greys live in Sources/Design/WhooshPalette.swift (the token file).
