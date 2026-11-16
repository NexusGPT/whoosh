import SwiftUI

/// 3a's icon hero: the app icon object, the deep blue field with the envelope as Liquid Glass on it (the shared BrandMark, A86), a hairline
/// edge light and a soft blue drop shadow, floating 3 pt on the 8 s loop with one sheen crossing the envelope in 900 ms.
/// Entrance: it rises from 16 pt below and 0.70 scale in 360 ms on a soft spring (overshoot under 1 %) starting at 90 ms.
/// The field is the icon's own gradient (apps/mac/App/AppIcon.icon field layer, sampled: #2960BC top left to #06194B bottom right);
/// when AppIcon.icon carries the glass glyph, this view swaps to `NSApp.applicationIconImage` in one line.
struct FirstStartMedallion: View {
    let t: Double
    let size: CGFloat
    let still: Bool
    @State private var entered = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.colorScheme) private var scheme

    private var cut: Bool { still || reduceMotion }

    var body: some View {
        let p = 0.5 - 0.5 * cos(2 * Double.pi * t / FirstStartLook.loopS)
        let r = size * 0.225
        let shape = RoundedRectangle(cornerRadius: r, style: .continuous)
        let markW = size * 0.74
        ZStack {
            shape.fill(field)
                .shadow(color: Color(.sRGB, red: 8 / 255, green: 34 / 255, blue: 110 / 255, opacity: scheme == .dark ? 0.0 : 0.55), radius: 23, y: 12)
                .shadow(color: scheme == .dark ? Color(.sRGB, red: 60 / 255, green: 120 / 255, blue: 1, opacity: 0.22) : Color(.sRGB, red: 8 / 255, green: 34 / 255, blue: 110 / 255, opacity: 0.22), radius: 6, y: 2)
            BrandMark(size: markW / EnvelopeShape.aspect, surface: "first-start")
                .overlay { sheen(markW: markW) }
                .offset(y: size * (0.505 - 0.5))
            shape.strokeBorder(Color.white.opacity(0.28), lineWidth: 0.5)
            shape.fill(LinearGradient(stops: [.init(color: .white.opacity(0.14), location: 0), .init(color: .white.opacity(0), location: 0.38)], startPoint: .top, endPoint: .bottom))
        }
        .frame(width: size, height: size)
        .offset(y: cut ? 0 : -3 * p)
        .scaleEffect(entered || cut ? 1 : 0.7)
        .offset(y: entered || cut ? 0 : 16)
        .opacity(entered || cut ? 1 : 0)
        .onAppear {
            guard !cut else { entered = true; return }
            withAnimation(.timingCurve(0.34, 1.2, 0.64, 1, duration: FirstStartLook.medallionSettleS).delay(FirstStartLook.medallionDelayS)) { entered = true }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(verbatim: FirstStartWords.appName))
        .accessibilityIdentifier("\(FirstStartFeature.X).medallion")
    }

    private var field: LinearGradient {
        LinearGradient(colors: [Color(.sRGB, red: 41 / 255, green: 96 / 255, blue: 188 / 255), Color(.sRGB, red: 23 / 255, green: 58 / 255, blue: 128 / 255),
                                Color(.sRGB, red: 6 / 255, green: 25 / 255, blue: 75 / 255)], startPoint: .topLeading, endPoint: .bottomTrailing)
    }

    /// one light crosses the glass envelope in 900 ms (30 % to 41.25 % of the 8 s loop), clipped to the envelope's shape
    private func sheen(markW: CGFloat) -> some View {
        let run = min(max((t / FirstStartLook.loopS - 0.30) / 0.1125, 0), 1)
        let h = markW / EnvelopeShape.aspect
        return Rectangle()
            .fill(LinearGradient(colors: [.white.opacity(0), .white.opacity(0.55), .white.opacity(0)], startPoint: .leading, endPoint: .trailing))
            .frame(width: markW * 0.26, height: h * 1.6)
            .rotationEffect(.degrees(12))
            .offset(x: -markW * 0.9 + markW * 1.8 * CGFloat(run))
            .opacity(run > 0 && run < 1 ? 1 : 0)
            .frame(width: markW, height: h)
            .clipShape(EnvelopeShape())
            .allowsHitTesting(false)
    }
}
