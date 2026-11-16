import SwiftUI

/// The undo capsule (`.undo`): 28 pt, fully round, glass, rising 8 pt into the foot strip, draining while it counts.
/// Pattern: SwiftUI TimelineView + a trimmed Circle for the drain; Z is a plain-key keyboardShortcut on the button.
public struct UndoBar: View {
    let message: String
    let countdown: UndoCountdown
    var onUndo: () -> Void
    var onExpire: () -> Void = {}
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var risen = false

    public init(message: String, countdown: UndoCountdown, onUndo: @escaping () -> Void, onExpire: @escaping () -> Void = {}) {
        self.message = message; self.countdown = countdown; self.onUndo = onUndo; self.onExpire = onExpire
    }

    public var body: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 30)) { ctx in
            let now = ctx.date
            let left = countdown.secondsLeft(at: now)
            HStack(spacing: WhooshSpacing.s10) {
                Drain(fraction: countdown.fraction(at: now))
                Text(message).whooshText(WhooshTypography.subhead).foregroundStyle(WhooshPalette.label).lineLimit(1).truncationMode(.tail)
                Button(action: onUndo) {
                    HStack(spacing: WhooshSpacing.s6) {
                        Text(Words.text("Undo")).whooshText(WhooshTypography.subhead, emphasized: true).foregroundStyle(WhooshPalette.accent).fixedSize()
                        KeyCap("Z")
                    }
                }
                .buttonStyle(.plain)
                .layoutPriority(1)
                .keyboardShortcut("z", modifiers: [])
                .accessibilityLabel("Undo, key Z")
                // A fixed two-digit slot: the count changes without any layout change; the digit swaps by a numeric transition.
                Text("\(max(left, 0))").whooshText(WhooshTypography.caption).foregroundStyle(WhooshPalette.secondaryLabel)
                    .contentTransition(reduceMotion ? .opacity : .numericText(countsDown: true))
                    .animation(WhooshMotion.readerSwap, value: left)
                    .frame(width: WhooshSpacing.s16, alignment: .trailing).accessibilityLabel("\(left) seconds left")
            }
            .padding(.horizontal, WhooshSpacing.s16)
            .frame(height: WhooshMetrics.segmentHeight)
            .floatingPill()
            .offset(y: risen || reduceMotion ? 0 : WhooshMotion.undoRise)
            .opacity(risen ? 1 : 0)
            .onChange(of: countdown.isExpired(at: now)) { _, gone in if gone { onExpire() } }
        }
        .onAppear { withAnimation(reduceMotion ? WhooshMotion.readerSwap : WhooshMotion.newRow) { risen = true } }
    }

    /// The remaining share as an arc, clockwise from 12 o'clock like the account mark's first-sync arc.
    private struct Drain: View {
        let fraction: Double
        var body: some View {
            ZStack {
                Circle().stroke(WhooshPalette.separator, lineWidth: WhooshAccountMark.stroke)
                Circle().trim(from: 0, to: fraction).rotation(.degrees(WhooshAccountMark.arcStartDegrees))
                    .stroke(WhooshPalette.accent, style: StrokeStyle(lineWidth: WhooshAccountMark.stroke, lineCap: .round))
            }.frame(width: WhooshAccountMark.Size.row - 2, height: WhooshAccountMark.Size.row - 2).accessibilityHidden(true)
        }
    }
}
