import SwiftUI

/// The lift material: what is still yours to change (the reply box, a reply in its undo window, a draft,
/// a failed send) floats as one frosted slab; what is on the record is flat. Native Glass family, no new
/// colour family. Board: docs/design/boards/thread-cards-r2/cards-s3/index.html `--lift` `--liftsh`
/// `.mine` `.qr` `.pop`; owner pick "Lifted until it lands", 2026-11-08 00:35. Geometry from docs/CONSTANTS.md.
public extension WhooshGlass {
    enum Lift {
        /// `--lift` the frost tint over the blurred backdrop.
        public static let tint = ThemeColor(light: RGBA(255, 255, 255, 0.72), dark: RGBA(58, 58, 63, 0.62))

        /// `--liftsh` ambient shadow: offset y 10, CSS blur 30 light / 28 dark.
        public static let shadow = ThemeColor(light: RGBA(0, 0, 0, 0.10), dark: RGBA(0, 0, 0, 0.42))
        public static let shadowY: CGFloat = 10
        public static func shadowBlur(_ scheme: ColorScheme) -> CGFloat { scheme == .dark ? 28 : 30 }
        /// `--liftsh` light only: the contact shadow, offset y 1, CSS blur 2 (dark draws none).
        public static let contactShadow = ThemeColor(light: RGBA(0, 0, 0, 0.05), dark: RGBA(0, 0, 0, 0))
        public static let contactShadowY: CGFloat = 1
        public static let contactShadowBlur: CGFloat = 2
        /// `--liftsh` dark only: the inset top highlight, 1 pt, no blur (light draws none).
        public static let topHighlight = ThemeColor(light: RGBA(255, 255, 255, 0), dark: RGBA(255, 255, 255, 0.07))
        public static let topHighlightWidth: CGFloat = 1
        /// `--liftsh` the 0.5 pt outer hairline: dark on light, light on dark.
        public static let hairline = ThemeColor(light: RGBA(0, 0, 0, 0.08), dark: RGBA(255, 255, 255, 0.10))
        public static var hairlineWidth: CGFloat { WhooshGlass.rimWidth }

        /// `.qr` the floating reply box.
        public static let panelInset = CGFloat(WhooshConstants.LIFT_PANEL_INSET_PT)
        public static let panelBottom = CGFloat(WhooshConstants.LIFT_PANEL_BOTTOM_PT)
        public static let panelRadius = CGFloat(WhooshConstants.LIFT_PANEL_RADIUS_PT)
        /// The reply box keeps the glass backdrop (`.qr.glass`: blur 16, saturation 1.8).
        public static var panelBlur: CGFloat { WhooshGlass.blurRadius }
        /// `.mine::before` a lifted in-flight card.
        public static let cardRadius = CGFloat(WhooshConstants.LIFT_CARD_RADIUS_PT)
        public static let cardBlur = CGFloat(WhooshConstants.LIFT_CARD_BLUR_PT)
        public static let cardSaturation = WhooshConstants.LIFT_CARD_SATURATION
        /// `.pop` the From picker: lift material, blur 22, plus one deeper drop (y 18, CSS blur 50).
        public static let popoverBlur = CGFloat(WhooshConstants.LIFT_POPOVER_BLUR_PT)
        public static let popoverShadow = ThemeColor(light: RGBA(0, 0, 0, 0.18), dark: RGBA(0, 0, 0, 0.18))
        public static let popoverShadowY: CGFloat = 18
        public static let popoverShadowBlur: CGFloat = 50
    }
}

public extension View {
    /// The lift material on any shape. SwiftUI materials take no blur radius, so the frost is the system's
    /// thin material plus the board's tint; the blur tokens record the board's values (as WhooshGlass F2).
    func whooshLift<S: InsettableShape>(in shape: S) -> some View { modifier(WhooshLiftModifier(shape: shape)) }
}

struct WhooshLiftModifier<S: InsettableShape>: ViewModifier {
    let shape: S
    @Environment(\.colorScheme) private var scheme
    private typealias L = WhooshGlass.Lift

    func body(content: Content) -> some View {
        content
            .background { shape.fill(L.tint).whooshSystemMaterial(.ultraThinMaterial, in: shape) }
            .overlay {
                shape.strokeBorder(L.topHighlight, lineWidth: L.topHighlightWidth)
                    .offset(y: L.topHighlightWidth).mask(shape)
            }
            .overlay { shape.stroke(L.hairline, lineWidth: L.hairlineWidth * 2) }
            .shadow(color: L.contactShadow.color(scheme),
                    radius: swiftUIShadowRadius(cssBlur: L.contactShadowBlur), y: L.contactShadowY)
            .shadow(color: L.shadow.color(scheme),
                    radius: swiftUIShadowRadius(cssBlur: L.shadowBlur(scheme)), y: L.shadowY)
    }
}
