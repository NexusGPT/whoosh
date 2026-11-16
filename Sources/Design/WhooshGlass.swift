import SwiftUI

/// Native Glass material: exactly four tokens (tint, blur, rim, shadow) and four floating objects
/// (the split control, the command field, the undo capsule, the trainer hint). Board: native-glass
/// index.html:29,31 (`--gtint` `--grim` `--gshadow`) and `.glass` (blur 16 px, saturate 1.8).
/// macOS 26 / iOS 26 draw the system's own Liquid Glass; macOS 15 / iOS 18 draw the vibrant fallback; the hidden
/// rig draws the fallback on an opaque underlay (WhooshRigStandIn), so `shot` shows the panel.
public enum WhooshGlass {
    /// `--gtint` the fallback's tint over the blurred backdrop.
    public static let tint = ThemeColor(light: RGBA(255, 255, 255, 0.62), dark: RGBA(72, 72, 78, 0.48))
    /// `.glass` backdrop blur radius and saturation. SwiftUI's materials take neither as a parameter: the
    /// fallback uses `.ultraThinMaterial`, the closest system blur; these record the board's values.
    public static let blurRadius: CGFloat = 16
    public static let saturation: Double = 1.8

    /// `--grim` part 1: the 0.5 px inner specular hairline.
    public static let rim = ThemeColor(light: RGBA(255, 255, 255, 0.95), dark: WhooshPalette.sharedDarkWhite16)
    public static let rimWidth: CGFloat = 0.5
    /// `--grim` part 2: the inset top highlight, offset 1 px, blur 1 px.
    public static let rimHighlight = ThemeColor(light: RGBA(255, 255, 255, 0.9), dark: RGBA(255, 255, 255, 0.12))
    public static let rimHighlightOffset: CGFloat = 1

    /// `--gshadow` part 1: the soft drop shadow (offset y 2; CSS blur 10 light, 12 dark).
    public static let shadow = ThemeColor(light: RGBA(0, 0, 0, 0.10), dark: RGBA(0, 0, 0, 0.45))
    public static let shadowY: CGFloat = 2
    public static func shadowBlur(_ scheme: ColorScheme) -> CGFloat { scheme == .dark ? 12 : 10 }
    /// `--gshadow` part 2: the 0.5 px outer hairline.
    public static let hairline = ThemeColor(light: RGBA(0, 0, 0, 0.06), dark: RGBA(0, 0, 0, 0.6))

    /// `--thumbsh` the selected segment's shadow (light only; dark draws none): offset y 1, CSS blur 3.
    public static let thumbShadow = ThemeColor(light: RGBA(0, 0, 0, 0.14), dark: RGBA(0, 0, 0, 0))
    public static let thumbShadowY: CGFloat = 1
    public static let thumbShadowBlur: CGFloat = 3
}

/// CSS `box-shadow` blur is twice SwiftUI's `shadow(radius:)` (a Gaussian's sigma): a unit conversion.
@inline(__always) func swiftUIShadowRadius(cssBlur: CGFloat) -> CGFloat { cssBlur / 2 }

public extension View {
    /// The glass of the four floating objects. Never on the sidebar, the list or the reader.
    func whooshGlass<S: InsettableShape>(in shape: S) -> some View { modifier(WhooshGlassModifier(shape: shape)) }

    /// The same glass; `report` is called on appear with the material this modifier actually drew: "glass-26" (the system's
    /// Liquid Glass) or "glass-15" (the fallback recipe). BrandMark publishes it on the socket, so a mark whose glass is
    /// removed reports nothing and reads as flat.
    func whooshGlass<S: InsettableShape>(in shape: S, report: @escaping @MainActor (String) -> Void) -> some View {
        modifier(WhooshGlassModifier(shape: shape, report: report))
    }
}

struct WhooshGlassModifier<S: InsettableShape>: ViewModifier {
    let shape: S
    var report: (@MainActor (String) -> Void)? = nil
    @Environment(\.colorScheme) private var scheme

    func body(content: Content) -> some View {
        if #available(macOS 26.0, iOS 26.0, *), !WhooshRigStandIn.isOn {
            content.glassEffect(.regular, in: shape)
                .onAppear { report?(BrandMaterial.glass26.rawValue) }
        } else {
            content
                .background { shape.fill(WhooshGlass.tint).whooshSystemMaterial(.ultraThinMaterial, in: shape) }
                .overlay { shape.strokeBorder(WhooshGlass.rim, lineWidth: WhooshGlass.rimWidth) }
                .overlay {
                    shape.strokeBorder(WhooshGlass.rimHighlight, lineWidth: WhooshGlass.rimHighlightOffset)
                        .offset(y: WhooshGlass.rimHighlightOffset).mask(shape)
                }
                .overlay { shape.stroke(WhooshGlass.hairline, lineWidth: WhooshGlass.rimWidth * 2) }
                .shadow(color: WhooshGlass.shadow.color(scheme),
                        radius: swiftUIShadowRadius(cssBlur: WhooshGlass.shadowBlur(scheme)),
                        y: WhooshGlass.shadowY)
                .onAppear { report?(BrandMaterial.glass15.rawValue) }
        }
    }
}
