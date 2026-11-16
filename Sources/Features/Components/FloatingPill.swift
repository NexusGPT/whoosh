import SwiftUI

/// The material of the undo capsule and the suggestion bar: one floating pill that always draws its own surface
/// (an opaque list-background underlay, the glass tint, the rim, the hairline, the shadow, all from WhooshGlass), so it is
/// never bare text over rows, in a still capture or on any macOS version. The system glass is not used here on purpose:
/// it is invisible to `shot`, which is how a line once drew over the list rows.
struct FloatingPill: ViewModifier {
    @Environment(\.colorScheme) private var scheme
    func body(content: Content) -> some View {
        let shape = RoundedRectangle(cornerRadius: WhooshRadius.capsule28, style: .continuous)
        content
            .background { shape.fill(WhooshPalette.background) }
            .background { shape.fill(WhooshGlass.tint).background(.ultraThinMaterial, in: shape) }
            .overlay { shape.strokeBorder(WhooshGlass.rim, lineWidth: WhooshGlass.rimWidth) }
            .overlay { shape.stroke(WhooshGlass.hairline, lineWidth: WhooshGlass.rimWidth * 2) }
            .shadow(color: WhooshGlass.shadow.color(scheme), radius: swiftUIShadowRadius(cssBlur: WhooshGlass.shadowBlur(scheme)), y: WhooshGlass.shadowY)
    }
}

extension View { func floatingPill() -> some View { modifier(FloatingPill()) } }
