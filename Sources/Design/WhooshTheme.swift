import SwiftUI

/// The one entry point to Native Glass. A surface reads every value through here (or the family type it
/// names); no colour, size or duration is typed anywhere else. Light and dark follow the system.
public enum WhooshTheme {
    public static let direction = "native-glass"
    public typealias Palette = WhooshPalette
    public typealias SystemHue = WhooshSystemHue
    public typealias Glass = WhooshGlass
    public typealias Lift = WhooshGlass.Lift
    public typealias Sidebar = WhooshSidebarMaterial
    public typealias Typography = WhooshTypography
    public typealias Radius = WhooshRadius
    public typealias Spacing = WhooshSpacing
    public typealias Metrics = WhooshMetrics
    public typealias Motion = WhooshMotion
    public typealias AccountMark = WhooshAccountMark
}
