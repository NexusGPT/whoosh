import CoreGraphics

/// The board's construction values (docs/design/boards/badges/emblem, "Construction: five layers, drawn at 2.5×"
/// and "6 · Written parts"). Shares of the diameter, so they scale with it. Not yet rows of docs/CONSTANTS.md: a finding.
enum EmblemConstruction {
    /// Tier edges: the detailed drawing from 40 pt, the 16 pt rules at 18 pt and below, the simplified drawing between.
    static let detailedFrom: CGFloat = 40
    static let tinyUpTo: CGFloat = 18
    /// Glyph box share of the diameter per tier; the glyph sits 0.5% of the diameter below centre.
    static let boxDetailed = 0.60, boxSimplified = 0.68, boxTiny = 0.72, glyphDrop = 0.005
    /// Layer 1, base gradient: colour + white at the top, + black at the bottom (light, dark, 16 pt).
    static let baseLight = (white: 0.20, black: 0.16), baseDark = (white: 0.12, black: 0.26), baseTiny = (white: 0.14, black: 0.12)
    /// Under dark ink the base darkens this share of its usual amount ("60% less").
    static let darkInkDarkening = 0.4
    /// Layer 2, inner shadow: radial centre at 36% height, radius 66%, clear to 74%, then black.
    static let innerShadowCentreY = 0.36, innerShadowRadius = 0.66, innerShadowClearTo = 0.74
    static let innerShadowLight = 0.20, innerShadowDark = 0.30
    /// Layer 3, specular: an ellipse 72% × 42% of the disc centred at 17% height, white to clear.
    static let specularWidth = 0.72, specularHeight = 0.42, specularCentreY = 0.17
    static let specularLight = 0.50, specularDark = 0.34
    /// Layer 4, pictogram: second tone = colour + 60% white (white ink) or + 34% black (dark ink); ink rule 3:1 over the
    /// glyph span 20% to 80% of the disc height; dark ink = colour + 74% black. Glyph shadow 0.9 pt down, 0.5 pt blur,
    /// colour + 60% black at 45% (values at 56 pt).
    static let secondToneWhite = 0.60, secondToneDark = 0.34, inkMin = 3.0, glyphSpan = (0.2, 0.8), darkInk = 0.74
    static let shadowDy: CGFloat = 0.9 / 56, shadowBlur: CGFloat = 0.5 / 56, shadowBlack = 0.60, shadowOpacity = 0.45
    /// Same-colour round-join stroke that softens the drawing's corners, grid units (detailed, simplified).
    static let softenDetailed: CGFloat = 0.9, softenSmall: CGFloat = 0.6
    /// Layer 5, rim: a top light (0.75 pt at 56, 0.5 pt simplified) of white 55% fading out by 42% height, and a
    /// 0.5 pt hairline of black 14% (white 16% in dark). The 16 pt tier keeps the hairline only.
    static let rimLightDetailed: CGFloat = 0.75 / 56, rimLightSmall: CGFloat = 0.5, rimLightOpacity = 0.55, rimFadeBy = 0.42
    static let hairline: CGFloat = 0.5, hairlineLight = 0.14, hairlineDark = 0.16
    /// Status dot on the bottom-trailing edge: radius share by tier, centre `dotInset` radii in from the corner, cut out
    /// by a ring of the background (2.5 pt at 56, 1.5 pt at 20, 1.25 pt at 16).
    static let dotDetailed: CGFloat = 8 / 56, dotSmall: CGFloat = 0.2, dotTiny: CGFloat = 3.25 / 16, dotInset: CGFloat = 0.76
    static let dotRingDetailed: CGFloat = 2.5 / 56, dotRingSmall: CGFloat = 1.5 / 20, dotRingTiny: CGFloat = 1.25 / 16
    /// Uploaded image inset, the disc showing as a bezel: 2 pt at 56, 1.25 at 20, 1.1 at 16.
    static let imageInsetDetailed: CGFloat = 2 / 56, imageInsetSmall: CGFloat = 1.25 / 20, imageInsetTiny: CGFloat = 1.1 / 16
}
