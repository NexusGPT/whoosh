import SwiftUI

/// Searching: soft bars in the inbox row's own anatomy (44 pt, mark, sender, time, subject), one sheen crossing them (1,100 ms loop).
/// They appear on the frame Enter lands and give way to the results by a cross-fade; the sidebar, the reader and the field stay still.
struct SearchSkeletonList: View {
    let height: CGFloat
    @State private var sheen = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.colorScheme) private var scheme

    /// bar widths (sender, time, subject) per row, the board's own set
    private static let widths: [(CGFloat, CGFloat, CGFloat)] = [(96, 38, 232), (118, 34, 196), (84, 40, 248), (132, 36, 170), (104, 38, 214), (90, 34, 236),
                                                                (124, 40, 184), (100, 36, 222), (112, 38, 200), (86, 34, 240), (120, 40, 176), (98, 36, 212)]

    var body: some View {
        let count = max(1, min(18, Int(height / WhooshMetrics.listRow)))
        VStack(spacing: 0) {
            ForEach(0..<count, id: \.self) { i in
                let w = Self.widths[i % Self.widths.count]
                VStack(alignment: .leading, spacing: 0) {
                    HStack { bar(w.0); Spacer(minLength: 0); bar(w.1) }.frame(height: WhooshMetrics.listRowLine)
                    bar(w.2).opacity(0.8).frame(height: WhooshMetrics.listRowLine)
                }
                .padding(.leading, WhooshMetrics.listRowLeading + WhooshSpacing.s4).padding(.trailing, WhooshSpacing.s12)
                .frame(maxWidth: .infinity, minHeight: WhooshMetrics.listRow, alignment: .leading)
                .overlay(alignment: .topLeading) { bar(16, circle: true).padding(.leading, WhooshSpacing.s4 + WhooshMetrics.listRowInset + WhooshMetrics.unreadDot + WhooshSpacing.s4).padding(.top, WhooshSpacing.s6) }
                .padding(.horizontal, WhooshMetrics.listRowInset)
            }
        }
        .clipped()
        .onAppear { if !reduceMotion { withAnimation(.timingCurve(0.4, 0, 0.2, 1, duration: 1.1).repeatForever(autoreverses: false)) { sheen = true } } }
        .whooshAx("mac-search.skeleton")   // the frame `measure mac-search.skeleton` reads (J2.1 skeleton-rows-fill-the-pane)
    }

    private func bar(_ w: CGFloat, circle: Bool = false) -> some View {
        let shape = RoundedRectangle(cornerRadius: circle ? w / 2 : 5, style: .continuous)
        return shape.fill(WhooshPalette.fill).frame(width: w, height: circle ? w : 9)
            .overlay {
                GeometryReader { g in
                    LinearGradient(colors: [.clear, Color.white.opacity(scheme == .dark ? 0.07 : 0.75), .clear], startPoint: .leading, endPoint: .trailing)
                        .frame(width: g.size.width).offset(x: sheen ? g.size.width : -g.size.width)
                }
                .clipShape(shape).allowsHitTesting(false)
            }
    }
}
