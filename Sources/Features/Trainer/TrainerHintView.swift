import SwiftUI

/// The hint that follows a mouse action: "Next time, press [E] to archive", in glass, rising UNDO_RISE_PX like the
/// undo capsule. It draws after the action has already landed (feedback first); nothing waits for it, and it leaves
/// the moment the key is used, the owner dismisses it, or the overlay opens (docs/MOTION.md laws 1 to 3 and 5).
public struct TrainerHintView: View {
    @State private var model = AppModel.shared
    @State private var trainer = TrainerModel.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    public init() {}

    public var body: some View {
        ZStack {
            if let hint = trainer.visibleHint(model) {
                HintCard(hint: hint) { trainer.dismiss() }
                    .transition(reduceMotion ? .opacity : .opacity.combined(with: .offset(y: WhooshMotion.undoRise)))
                    .id(hint)
            }
        }
        .animation(WhooshMotion.animation(WhooshMotion.newRow, reduceMotion: reduceMotion), value: trainer.visibleHint(model))
    }
}

struct HintCard: View {
    let hint: TrainerHint
    let onDismiss: () -> Void

    var body: some View {
        HStack(spacing: WhooshSpacing.s6) {
            Text("Next time, press").whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label)
            KeyCap(hint.chord)
            Text("to \(hint.verb)").whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label)
            Progress(learned: min(hint.usesAtShow, TrainerModel.stopAfter), of: TrainerModel.stopAfter)
                .padding(.leading, WhooshSpacing.s4)
            Button(action: onDismiss) {
                Image(systemName: "xmark").font(WhooshTypography.caption.font(emphasized: true))
                    .foregroundStyle(WhooshPalette.secondaryLabel)
                    .frame(width: WhooshMetrics.keyCap, height: WhooshMetrics.keyCap)
                    .hoverAckCircle()
            }
            .buttonStyle(.plain).help("Dismiss")
            .accessibilityLabel("Dismiss the hint")
            .whooshAx("mac-trainer.hint.dismiss")   // the x has its own id and a frame `measure` reads (the door of the same id runs the same dismiss)
        }
        .padding(.horizontal, WhooshSpacing.s12).padding(.vertical, WhooshSpacing.s8)
        .fixedSize()
        .whooshGlass(in: RoundedRectangle(cornerRadius: WhooshRadius.hint, style: .continuous))
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("mac-trainer.hint")
    }

    /// Three quiet dots: how many key presses of this action the owner has made out of the number that ends the hints.
    private struct Progress: View {
        let learned: Int
        let of: Int
        var body: some View {
            HStack(spacing: WhooshSpacing.s4) {
                ForEach(0..<of, id: \.self) { i in
                    Circle().fill(i < learned ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.separator))
                        .frame(width: WhooshSpacing.s6, height: WhooshSpacing.s6)
                }
            }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("\(learned) of \(of) key presses")
        }
    }
}

private extension View {
    /// Hover tints the close mark within HOVER_TINT_MS and never moves it.
    func hoverAckCircle() -> some View { modifier(CircleHover()) }
}

private struct CircleHover: ViewModifier {
    @State private var hovering = false
    func body(content: Content) -> some View {
        content
            .background(Circle().fill(WhooshPalette.hover).opacity(hovering ? 1 : 0))
            .onHover { hovering = $0 }
            .animation(WhooshMotion.hover, value: hovering)
    }
}
