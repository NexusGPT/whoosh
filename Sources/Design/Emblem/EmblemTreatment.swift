import Foundation

/// A calmer drawing of the quiet marks (list row, sidebar, compact) for the owner's emblem board. A Debug build reads
/// WHOOSH_EMBLEM_TREATMENT once at launch (faint | ring | neutral | glass1 | glass2 | glass3); absent or unknown, the
/// lit disc draws as before. Release compiles the override out, so `current` is always nil there.
/// Board round 4 (owner: "more neutral and classy and consistent (glass style consistent shade)"): the glass cases draw
/// the disc in the design's own glass tokens (EmblemGlassChip.swift), one shade for every account; only the glyph or a
/// dot carries the account colour.
enum EmblemTreatment: String, Sendable {
    case faint, ring, neutral, glass1, glass2, glass3

    static let current: EmblemTreatment? = {
        #if DEBUG
        if let text = ProcessInfo.processInfo.environment["WHOOSH_EMBLEM_TREATMENT"] { return EmblemTreatment(rawValue: text) }
        #endif
        return nil
    }()

    /// The account colour's saturation share this treatment draws with.
    var saturation: Double {
        switch self {
        case .faint: return 0.45
        case .ring: return 0.6
        case .neutral: return 0.7
        case .glass1: return 0.55
        case .glass2: return Self.glass2Saturation
        case .glass3: return EmblemListSaturation.value
        }
    }
    var isGlass: Bool { self == .glass1 || self == .glass2 || self == .glass3 }

    /// Faint: the disc fill's opacity over the surface.
    static let faintFillOpacity = 0.55
    static let ringWidth: CGFloat = 1
    /// G2: the hue keeps this share of its saturation, then mixes toward black (light) or white (dark).
    static let glass2Saturation = 0.4
    static let glass2ShadeLight = 0.5
    static let glass2ShadeDark = 0.55
    /// G3: the account-colour dot, its diameter in points and its centre as a share of the disc (lower right).
    static let glass3DotDiameter: CGFloat = CGFloat(WhooshConstants.EMBLEM_DOT_PT)
    static let glass3DotCentre: CGFloat = 0.78
    /// The glyph's second tone: the ink mixed this far toward the surface (all calm treatments).
    static let secondToneMix = 0.45
}
