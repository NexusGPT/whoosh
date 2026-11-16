import SwiftUI

/// The glass emblem chip (owner pick 11:07): the dropdown's glass tokens at chip size. A known provider puts its mark inside, in its own
/// colours, under a faint glass sheen so it reads as set into the chip; before the mark is known the chip holds a grey glyph.
struct MarkChip: View {
    var kind: ProviderKind?
    var glyph = "envelope"
    let diameter: CGFloat
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let size = (diameter * (kind?.share ?? 0.5) * (diameter >= 32 ? 0.92 : 1)).rounded()
        ZStack {
            Circle().fill(WhooshGlass.tint)
            Circle().fill(WhooshPalette.fill)
            if let kind, kind != .unknown {
                ProviderMarkShape(kind: kind).frame(width: size, height: size)
                    .shadow(color: .black.opacity(scheme == .dark ? 0.40 : 0.12), radius: 0.3, y: 0.5)
                    .id(kind).transition(reduceMotion ? .opacity : .markArrive)
                Circle().fill(LinearGradient(stops: [.init(color: .white.opacity(scheme == .dark ? 0.09 : 0.26), location: 0), .init(color: .white.opacity(0), location: 0.52)],
                                             startPoint: .top, endPoint: .bottom))
                    .allowsHitTesting(false).transition(.opacity).animation(AddAccountMetrics.sheen, value: kind)
            } else if kind == .unknown {
                BrandMark(size: diameter * 0.6, surface: "add-account-chip").transition(.opacity)   // A86: the glass mark, never a globe
            } else {
                Image(systemName: glyph).font(.system(size: diameter * 0.46, weight: .regular)).foregroundStyle(WhooshPalette.secondaryLabel)
                    .transition(.opacity)
            }
        }
        .frame(width: diameter, height: diameter)
        .overlay(Circle().strokeBorder(WhooshGlass.rim, lineWidth: WhooshGlass.rimWidth))
        .overlay(Circle().strokeBorder(WhooshGlass.rimHighlight, lineWidth: 1).offset(y: 1).mask(Circle()))
        .overlay(Circle().strokeBorder(WhooshGlass.hairline, lineWidth: WhooshGlass.rimWidth))
        .clipShape(Circle())
        .animation(AddAccountMetrics.motion(AddAccountMetrics.arrive, reduceMotion), value: kind)
        .accessibilityHidden(true)
    }
}

/// The mark's arrival: scale 0.7 to 1, blur 3 pt to 0, opacity 0 to 1 (the board's `pmIn`).
private struct MarkArrive: ViewModifier {
    let hidden: Bool
    func body(content: Content) -> some View {
        content.opacity(hidden ? 0 : 1).scaleEffect(hidden ? 0.7 : 1).blur(radius: hidden ? 3 : 0)
    }
}
extension AnyTransition {
    static var markArrive: AnyTransition { .modifier(active: MarkArrive(hidden: true), identity: MarkArrive(hidden: false)) }
}
