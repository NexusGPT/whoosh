import Foundation

/// sRGB colour maths for the emblem: mixes, WCAG contrast, the list-row saturation cut.
public struct EmblemRGB: Equatable, Sendable {
    public var r: Double, g: Double, b: Double
    public init(_ r: Double, _ g: Double, _ b: Double) { self.r = r; self.g = g; self.b = b }
    public init?(hex: String) {
        let t = hex.trimmingCharacters(in: CharacterSet(charactersIn: "#"))
        guard t.count == 6, let v = UInt32(t, radix: 16) else { return nil }
        self.init(Double((v >> 16) & 255) / 255, Double((v >> 8) & 255) / 255, Double(v & 255) / 255)
    }
    public static let white = EmblemRGB(1, 1, 1), black = EmblemRGB(0, 0, 0)

    public func mixed(_ o: EmblemRGB, _ t: Double) -> EmblemRGB {
        EmblemRGB(r + (o.r - r) * t, g + (o.g - g) * t, b + (o.b - b) * t)
    }
    public var luminance: Double {
        func l(_ v: Double) -> Double { v <= 0.04045 ? v / 12.92 : pow((v + 0.055) / 1.055, 2.4) }
        return 0.2126 * l(r) + 0.7152 * l(g) + 0.0722 * l(b)
    }
    public func contrast(_ o: EmblemRGB) -> Double {
        let a = luminance, b = o.luminance
        return (max(a, b) + 0.05) / (min(a, b) + 0.05)
    }
    /// HSB saturation times `k`, hue and brightness kept.
    public func saturation(times k: Double) -> EmblemRGB {
        let mx = max(r, g, b), mn = min(r, g, b)
        guard mx > 0, mx > mn else { return self }
        let s = (mx - mn) / mx * k, newMin = mx * (1 - s)
        func f(_ v: Double) -> Double { mx - (mx - v) / (mx - mn) * (mx - newMin) }
        return EmblemRGB(f(r), f(g), f(b))
    }
    public var saturation: Double { let mx = max(r, g, b); return mx > 0 ? (mx - min(r, g, b)) / mx : 0 }
    public var hex: String {
        String(format: "#%02X%02X%02X", Int((r * 255).rounded()), Int((g * 255).rounded()), Int((b * 255).rounded()))
    }
}
