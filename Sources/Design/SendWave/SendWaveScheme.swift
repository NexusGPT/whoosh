import SwiftUI

/// The four colour schemes of the send shockwave (docs/PRD.md "## Send shockwave"). The rings run leading edge
/// first; a scheme with fewer than four colours repeats them. Default: SENDWAVE_SCHEME (docs/CONSTANTS.md, aurora).
public enum SendWaveScheme: String, CaseIterable, Sendable {
    case aurora, halo, frost, spark

    /// The ledger's SENDWAVE_SCHEME (text, so the generated constants skip it; par-sendwave reads both and
    /// goes red when they differ).
    public static let ledgerDefault: SendWaveScheme = .aurora
    /// The scheme in use: the ledger's, or WHOOSH_SENDWAVE_SCHEME for the owner's preview strips.
    public static let current: SendWaveScheme =
        ProcessInfo.processInfo.environment["WHOOSH_SENDWAVE_SCHEME"].flatMap(SendWaveScheme.init(rawValue:)) ?? ledgerDefault

    /// Ring colours, leading edge first. Aurora: blue, purple, pink, peach; Halo: orange into yellow; Frost: mint,
    /// from a cool aqua-mint lead to a pale ice-mint tail; Spark: purple into gold. Each hue has a light-mode value
    /// (light tints by multiplying, so its rings are pale and airy) and a dark-mode value (dark screen-blends, so
    /// its rings are rich mid-tones that glow without washing the text out).
    public var rings: [ThemeColor] {
        switch self {
        case .aurora: return [Self.blue, Self.purple, Self.pink, Self.peach]
        case .halo:   return [Self.ember, Self.orange, Self.amber, Self.yellow]
        case .frost:  return [Self.aqua, Self.mint, Self.mintPale, Self.ice]
        case .spark:  return [Self.violet, Self.orchid, Self.goldDeep, Self.gold]
        }
    }

    private static let blue = ThemeColor(light: RGBA(hex: 0x6E9BFF), dark: RGBA(hex: 0x4F7BFF))
    private static let purple = ThemeColor(light: RGBA(hex: 0xA98BFF), dark: RGBA(hex: 0x8A5CFF))
    private static let pink = ThemeColor(light: RGBA(hex: 0xFF9CCB), dark: RGBA(hex: 0xF0559E))
    private static let peach = ThemeColor(light: RGBA(hex: 0xFFC6A1), dark: RGBA(hex: 0xFF9E6B))

    private static let ember = ThemeColor(light: RGBA(hex: 0xFFA36B), dark: RGBA(hex: 0xFF7A2E))
    private static let orange = ThemeColor(light: RGBA(hex: 0xFFB877), dark: RGBA(hex: 0xFFA040))
    private static let amber = ThemeColor(light: RGBA(hex: 0xFFD27F), dark: RGBA(hex: 0xFFC94D))
    private static let yellow = ThemeColor(light: RGBA(hex: 0xFFE79A), dark: RGBA(hex: 0xFFE27A))

    private static let aqua = ThemeColor(light: RGBA(hex: 0x7FE3D8), dark: RGBA(hex: 0x22C9B8))
    private static let mint = ThemeColor(light: RGBA(hex: 0x9BEFCF), dark: RGBA(hex: 0x2FE0A4))
    private static let mintPale = ThemeColor(light: RGBA(hex: 0xC2F6DF), dark: RGBA(hex: 0x5CEBC0))
    private static let ice = ThemeColor(light: RGBA(hex: 0xDDFBEE), dark: RGBA(hex: 0x8FF2D4))

    private static let violet = ThemeColor(light: RGBA(hex: 0x9D84FF), dark: RGBA(hex: 0x7A4DFF))
    private static let orchid = ThemeColor(light: RGBA(hex: 0xC59BF5), dark: RGBA(hex: 0xA65BE0))
    private static let goldDeep = ThemeColor(light: RGBA(hex: 0xFFCF73), dark: RGBA(hex: 0xE9A72A))
    private static let gold = ThemeColor(light: RGBA(hex: 0xFFE3A0), dark: RGBA(hex: 0xFFD066))
}
