import SwiftUI

/// One colour channel set in sRGB, 0...1, with its opacity. The only way a colour value enters the app:
/// every literal lives in a `Whoosh*` token file of this folder, next to the board value it mirrors.
public struct RGBA: Sendable, Equatable {
    public let red: Double
    public let green: Double
    public let blue: Double
    public let opacity: Double

    /// `hex` is the board's `#RRGGBB`; `opacity` its alpha.
    public init(hex: UInt32, opacity: Double = 1) {
        red = Double((hex >> 16) & 0xFF) / 255
        green = Double((hex >> 8) & 0xFF) / 255
        blue = Double(hex & 0xFF) / 255
        self.opacity = opacity
    }

    /// The board's `rgba(r,g,b,a)` with 0...255 channels.
    public init(_ r: Int, _ g: Int, _ b: Int, _ a: Double) {
        red = Double(r) / 255
        green = Double(g) / 255
        blue = Double(b) / 255
        opacity = a
    }

    public var color: Color { Color(.sRGB, red: red, green: green, blue: blue, opacity: opacity) }

    /// WCAG relative luminance of the opaque colour.
    public var luminance: Double {
        func lin(_ c: Double) -> Double { c <= 0.03928 ? c / 12.92 : pow((c + 0.055) / 1.055, 2.4) }
        return 0.2126 * lin(red) + 0.7152 * lin(green) + 0.0722 * lin(blue)
    }

    /// This colour composited over an opaque `base` (how a translucent token reads on its surface).
    public func over(_ base: RGBA) -> RGBA {
        let a = opacity
        return RGBA(red: red * a + base.red * (1 - a), green: green * a + base.green * (1 - a),
                    blue: blue * a + base.blue * (1 - a))
    }

    /// WCAG contrast ratio between two opaque colours.
    public static func contrast(_ a: RGBA, _ b: RGBA) -> Double {
        let (hi, lo) = (max(a.luminance, b.luminance), min(a.luminance, b.luminance))
        return (hi + 0.05) / (lo + 0.05)
    }

    private init(red: Double, green: Double, blue: Double) {
        self.red = red; self.green = green; self.blue = blue; opacity = 1
    }
}

/// A colour token with a light and a dark value. It resolves itself from the view's colour scheme, so a
/// call site writes `.foregroundStyle(WhooshPalette.label)` and both appearances follow the system.
public struct ThemeColor: ShapeStyle, Sendable, Equatable {
    public let light: RGBA
    public let dark: RGBA

    public init(light: RGBA, dark: RGBA) {
        self.light = light
        self.dark = dark
    }

    public func value(_ scheme: ColorScheme) -> RGBA { scheme == .dark ? dark : light }
    public func color(_ scheme: ColorScheme) -> Color { value(scheme).color }
    public func resolve(in environment: EnvironmentValues) -> Color { color(environment.colorScheme) }
}
