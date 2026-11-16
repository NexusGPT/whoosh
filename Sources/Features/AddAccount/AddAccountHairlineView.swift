import SwiftUI

/// The 1 pt step line under the fields: grey, in `count` segments (five for detection, two for the check); one segment fills per step.
/// With no steps it is one plain hairline. It is the only moving part of the sheet while the app works.
struct AddAccountHairlineView: View {
    let count: Int
    let done: Int
    let partial: Double
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(spacing: count == 0 ? 0 : AddAccountMetrics.segmentGap) {
            ForEach(0..<max(count, 1), id: \.self) { i in
                let fill = count == 0 ? 0 : (i < done ? 1.0 : (i == done ? partial : 0))
                Rectangle().fill(WhooshPalette.separator)
                    .overlay(alignment: .leading) {
                        GeometryReader { g in Rectangle().fill(WhooshPalette.accent).frame(width: g.size.width * fill) }
                    }
                    .clipped()
            }
        }
        .frame(height: AddAccountMetrics.hairline)
        .animation(AddAccountMetrics.motion(AddAccountMetrics.arrive, reduceMotion), value: done)
        .animation(AddAccountMetrics.motion(AddAccountMetrics.arrive, reduceMotion), value: partial)
        .accessibilityHidden(true)
    }
}
