import SwiftUI

/// The sidebar's motions (docs/MOTION.md): state lands on the input frame, motion follows, nothing gates the next click.
/// Every value comes from WhooshMotion; the popover entrance is the shared Components one; Reduced Motion turns movement into cross-fades.
enum SidebarMotion {
    /// Group and label disclosure reflow: the system spring at the tab-morph response and damping (no bounce).
    static func disclosure(reduce: Bool) -> Animation { reduce ? WhooshMotion.readerSwap : WhooshMotion.tabThumb }
    /// Digits cross-fade when a count changes.
    static var count: Animation { WhooshMotion.readerSwap }
    /// First-sync progress animates only while the account is still syncing.
    static func progress(_ p: Double) -> Animation? { p < 1 ? WhooshMotion.readerSwap : nil }
}

/// A press darkens the control by one step on the frame it lands; no animation on the press itself.
struct SidebarPressStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .background(RoundedRectangle(cornerRadius: WhooshRadius.control)
                .fill(configuration.isPressed ? AnyShapeStyle(WhooshPalette.hover) : AnyShapeStyle(Color.clear)))
    }
}

/// A count whose digits cross-fade on change.
struct SidebarCount: View {
    let value: Int
    var body: some View {
        Text("\(value)").whooshText(WhooshTypography.caption)
            .contentTransition(.opacity)
            .animation(SidebarMotion.count, value: value)
    }
}
