import SwiftUI

/// The sidebar is the vibrant sidebar material (the desktop blurred behind the window), not glass.
/// Board: native-glass index.html `.sb .wall{filter:blur(28px) saturate(1.3)}` + `--sbtint`.
public enum WhooshSidebarMaterial {
    /// The board's desktop blur and saturation; the app gets them from the system sidebar material.
    public static let desktopBlur: CGFloat = 28
    public static let desktopSaturation: Double = 1.3
    public static var tint: ThemeColor { WhooshPalette.sidebarTint }
    public static var edge: ThemeColor { WhooshPalette.sidebarEdge }
    public static let edgeWidth: CGFloat = 1
}

/// `--wall` the board's stand-in for the owner's desktop behind the sidebar. On the Mac the real desktop
/// shows through; these stops serve only design-build captures and previews where no desktop exists.
public enum WhooshBoardWallpaper {
    public struct Stop: Sendable {
        public let center: UnitPoint
        public let radius: CGSize      // ellipse radii as fractions of the canvas
        public let color: ThemeColor
    }
    public static let blobs: [Stop] = [
        Stop(center: UnitPoint(x: 0.10, y: 0.20), radius: CGSize(width: 0.60, height: 0.50),
             color: ThemeColor(light: RGBA(hex: 0x9EC1FF), dark: RGBA(hex: 0x1B3A8A))),
        Stop(center: UnitPoint(x: 0.30, y: 0.85), radius: CGSize(width: 0.50, height: 0.60),
             color: ThemeColor(light: RGBA(hex: 0xF5B9D8), dark: RGBA(hex: 0x5B2463))),
        Stop(center: UnitPoint(x: 0.00, y: 0.60), radius: CGSize(width: 0.70, height: 0.70),
             color: ThemeColor(light: RGBA(hex: 0xB9E3F0), dark: RGBA(hex: 0x0E4B5E))),
    ]
    /// Each blob fades to clear at this share of its radius.
    public static let blobFalloff: Double = 0.70
    /// The base linear gradient, top to bottom.
    public static let baseTop = ThemeColor(light: RGBA(hex: 0xDCE6FA), dark: RGBA(hex: 0x0D1530))
    public static let baseBottom = ThemeColor(light: RGBA(hex: 0xEEDDF1), dark: RGBA(hex: 0x1A1024))
}
