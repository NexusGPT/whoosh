import SwiftUI

/// The elastic row (docs/MOTION.md "Elastic card"): the row follows the finger 1:1, the action's fill grows under
/// it with its icon and word, one haptic tick when the release would commit, and a release past the word
/// (PhoneMetrics.swipeWidth) commits. The model changes on the release frame; nothing waits for an animation.
struct SwipeRow<Content: View>: View {
    let leading: SwipeAction
    let trailing: SwipeAction
    let onCommit: (SwipeAction) -> Void
    @ViewBuilder var content: Content

    @State private var dx: CGFloat = 0
    @State private var horizontal: Bool?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var armed: Bool { abs(dx) >= PhoneMetrics.swipeWidth }

    var body: some View {
        ZStack {
            if dx > 0 { fill(leading, edge: .leading) }
            if dx < 0 { fill(trailing, edge: .trailing) }
            content.offset(x: dx)
        }
        .clipped()
        .contentShape(Rectangle())
        .simultaneousGesture(drag)
        .sensoryFeedback(.impact(weight: .light), trigger: armed) { _, now in now }
        .accessibilityActions {
            Button(leading.title) { onCommit(leading) }
            Button(trailing.title) { onCommit(trailing) }
        }
    }

    private var drag: some Gesture {
        DragGesture(minimumDistance: WhooshSpacing.s12)
            .onChanged { v in
                if horizontal == nil { horizontal = abs(v.translation.width) > abs(v.translation.height) }
                guard horizontal == true else { return }
                dx = v.translation.width          // 1:1 with the finger, no animation on the tracking path
            }
            .onEnded { v in
                defer { horizontal = nil }
                guard horizontal == true else { return }
                let projected = v.predictedEndTranslation.width
                let commit = abs(dx) >= PhoneMetrics.swipeWidth || abs(projected) >= PhoneMetrics.swipeWidth * 2
                if commit {
                    let action = dx > 0 ? leading : trailing
                    onCommit(action)                // state first, on this frame
                    if !action.removesRow { settle() } else { dx = 0 }
                } else {
                    settle()
                }
            }
    }

    private func settle() {
        withAnimation(reduceMotion ? nil : .spring(response: WhooshMotion.tabMorph, dampingFraction: WhooshMotion.tabMorphDamping)) { dx = 0 }
    }

    private func fill(_ action: SwipeAction, edge: HorizontalEdge) -> some View {
        let colour: ThemeColor = action == .snooze ? PhoneGround.snooze : WhooshPalette.selection
        return HStack {
            if edge == .trailing { Spacer(minLength: 0) }
            VStack(spacing: WhooshSpacing.s4) {
                Image(systemName: action.symbol).font(.system(size: PhoneType.title3.size + WhooshSpacing.s4, weight: .medium))
                    .scaleEffect(armed ? 1.08 : 1)
                Text(action.title).phoneText(PhoneType.footnote, emphasized: true)
            }
            .foregroundStyle(WhooshPalette.onAccent)
            .frame(width: max(abs(dx), PhoneMetrics.swipeWidth))
            .animation(reduceMotion ? nil : WhooshMotion.hover, value: armed)
            if edge == .leading { Spacer(minLength: 0) }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(colour)
        .accessibilityHidden(true)
    }
}
