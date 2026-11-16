import Foundation

/// Every number the shockwave reads, from the ledger (docs/CONSTANTS.md, generated into WhooshConstants).
enum SendWaveTuning {
    static let durationS = Double(WhooshConstants.SENDWAVE_MS) / 1000
    static let rings = WhooshConstants.SENDWAVE_RINGS
    static let channelOffset = WhooshConstants.SENDWAVE_CHANNEL_OFFSET
    static let frameMs = Double(WhooshConstants.FRAME_MS)

    static let lensPt = Double(WhooshConstants.SENDWAVE_LENS_PT)
    static let band = WhooshConstants.SENDWAVE_BAND
    static let strengthLight = WhooshConstants.SENDWAVE_TINT_LIGHT
    static let strengthDark = WhooshConstants.SENDWAVE_SCREEN_DARK
    static let edgeLight = WhooshConstants.SENDWAVE_EDGE_LIGHT
    static let edgeDark = WhooshConstants.SENDWAVE_EDGE_DARK
    /// The send wave fades over its last 20 % (lead ruling 2026-11-08).
    static let fadeFrom = 0.8

    /// The landing (owner 00:35, board cards-s3 "The landing animation"): its ring crosses the card over
    /// LAND_AURORA_MS, LAND_BAND of the card's diagonal, fading over its last 40 % (the board's LAND_AURORA_MS row).
    static let landingDurationS = Double(WhooshConstants.LAND_AURORA_MS) / 1000
    static let landingBand = WhooshConstants.LAND_BAND
    static let landingFadeFrom = 0.6

    /// Snapshot pixels per point: 2 = the screen's Retina resolution, 1 = half (the trade if the budget is missed).
    static let snapshotScale = Double(WhooshConstants.SENDWAVE_SNAPSHOT_SCALE)
}
