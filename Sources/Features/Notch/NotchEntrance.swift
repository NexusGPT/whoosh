import SwiftUI

/// The arrival's one motion (owner: "no latency, no heaviness"; docs/MOTION.md rules 4 and 5): the shape is at its full
/// width on the frame the mail lands, and the card inside fades in and rises UNDO_RISE_PX. Opacity and a short offset
/// only, ROW_INSERT_MS long, nothing waits on it and nothing is queued behind it. Reduce Motion shows the card at once.
struct NotchEntrance: ViewModifier {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var risen = false

    func body(content: Content) -> some View {
        content
            .opacity(risen ? 1 : 0)
            .offset(y: risen ? 0 : -WhooshMotion.undoRise)
            .onAppear {
                if let a = WhooshMotion.animation(WhooshMotion.newRow, reduceMotion: reduceMotion) {
                    withAnimation(a) { risen = true }
                } else {
                    risen = true
                }
            }
    }
}
