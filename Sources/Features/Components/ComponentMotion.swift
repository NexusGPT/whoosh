import SwiftUI

/// The shared controls' motion (docs/MOTION.md): transform and opacity only, every animation interruptible, no state
/// waits for one, and Reduced Motion turns each movement into a cross-fade. Durations come from WhooshConstants and
/// WhooshMotion; the popover has no token yet (token request in docs/lanes/components.md).
enum ComponentMotion {
    static var popoverScaleFrom: CGFloat { CGFloat(WhooshConstants.POPOVER_SCALE_FROM) }
    static func popoverIn(reduceMotion: Bool) -> Animation {
        .easeOut(duration: reduceMotion ? WhooshMotion.readerCrossFade : Double(WhooshConstants.POPOVER_MS) / 1000)
    }
}

/// A popover scales from its anchor (POPOVER_SCALE_FROM to 1 over POPOVER_MS, ease-out) while it fades in.
struct PopoverEntrance: ViewModifier {
    var anchor: UnitPoint = .topLeading
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var shown = false
    func body(content: Content) -> some View {
        content
            .scaleEffect(shown || reduceMotion ? 1 : ComponentMotion.popoverScaleFrom, anchor: anchor)
            .opacity(shown ? 1 : 0)
            .onAppear { withAnimation(ComponentMotion.popoverIn(reduceMotion: reduceMotion)) { shown = true } }
    }
}

/// Hover acknowledges within HOVER_TINT_MS: the hover tint fades in over the control, never moving it.
struct HoverAck: ViewModifier {
    var radius: CGFloat
    @State private var hovering = false
    func body(content: Content) -> some View {
        content
            .overlay {
                RoundedRectangle(cornerRadius: radius, style: .continuous).fill(WhooshPalette.hover)
                    .opacity(hovering ? 1 : 0).allowsHitTesting(false)
            }
            .onHover { hovering = $0 }
            .animation(WhooshMotion.hover, value: hovering)
    }
}

/// A view that fades in (and rises UNDO_RISE_PX unless Reduced Motion) the first time it appears.
struct FadeIn: ViewModifier {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var shown = false
    func body(content: Content) -> some View {
        content
            .opacity(shown ? 1 : 0)
            .offset(y: shown || reduceMotion ? 0 : WhooshMotion.undoRise)
            .onAppear { withAnimation(WhooshMotion.readerSwap) { shown = true } }
    }
}

extension View {
    func popoverEntrance(anchor: UnitPoint = .topLeading) -> some View { modifier(PopoverEntrance(anchor: anchor)) }
    func hoverAck(radius: CGFloat) -> some View { modifier(HoverAck(radius: radius)) }
    func fadeIn() -> some View { modifier(FadeIn()) }
}
