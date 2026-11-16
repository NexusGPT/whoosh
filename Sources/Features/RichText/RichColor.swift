import Foundation

/// The text colours the Colour key offers. Names of palette tokens, never raw values: the text view draws the token
/// (dark and light), the HTML part carries its light value as a hex string. `nil` on a run is the default colour.
public enum RichColor: String, CaseIterable, Sendable {
    case secondary, accent, warning

    public var token: ThemeColor {
        switch self {
        case .secondary: return WhooshPalette.secondaryLabel
        case .accent: return WhooshPalette.accent
        case .warning: return WhooshPalette.warning
        }
    }

    /// "#RRGGBB" of the light value: mail clients show the colour as sent, not as the app's theme.
    public var hex: String {
        let c = token.light
        func b(_ v: Double) -> Int { Int((min(max(v, 0), 1) * 255).rounded()) }
        return String(format: "#%02X%02X%02X", b(c.red), b(c.green), b(c.blue))
    }
}
