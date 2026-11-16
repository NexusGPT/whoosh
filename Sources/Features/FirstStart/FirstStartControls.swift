import SwiftUI

/// A key cap inside a flow button: the boards' `kbd` (10.5 pt semibold, 16 pt tall, 4 pt radius).
struct FirstStartKbd: View {
    let text: String
    var onAccent = false
    var body: some View {
        Text(text)
            .font(.system(size: 10.5, weight: .semibold))
            .foregroundStyle(onAccent ? WhooshPalette.onAccent : WhooshPalette.label)
            .padding(.horizontal, 5)
            .frame(minWidth: 16, minHeight: 16)
            .background(onAccent ? AnyShapeStyle(WhooshPalette.onAccent.opacity(0.24)) : AnyShapeStyle(WhooshPalette.keyCap),
                        in: RoundedRectangle(cornerRadius: 4, style: .continuous))
    }
}

/// The boards' `.btn`: 28 pt tall, 7 pt radius, the label then its key; the primary one is the accent. Press: 80 ms, scale 0.96.
struct FirstStartButton: View {
    let id: String
    let title: String
    let key: String
    var primary = false
    var enabled = true
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Text(title).font(.system(size: 13))
                FirstStartKbd(text: key, onAccent: primary)
            }
            .padding(.leading, 11).padding(.trailing, 6)
            .frame(height: 28)
            .foregroundStyle(primary ? WhooshPalette.onAccent : WhooshPalette.label)
            .background(primary ? AnyShapeStyle(WhooshPalette.accent) : AnyShapeStyle(WhooshPalette.fill),
                        in: RoundedRectangle(cornerRadius: 7, style: .continuous))
            .overlay { if !primary { RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(WhooshPalette.separator, lineWidth: 0.5) } }
            .opacity(enabled ? 1 : 0.45)
            .contentShape(RoundedRectangle(cornerRadius: 7, style: .continuous))
        }
        .buttonStyle(FirstStartPressStyle())
        .disabled(!enabled)
        .whooshAx("\(FirstStartFeature.X).\(id)")
        .accessibilityLabel(Text(title))
    }
}

struct FirstStartPressStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed ? 0.96 : 1)
            .brightness(configuration.isPressed ? -0.08 : 0)
            .animation(.easeOut(duration: FirstStartLook.keyPressS), value: configuration.isPressed)
    }
}

/// The eight step dots: 6 pt, the current one in the accent, sliding to the next in the morph's 200 ms (2b `dotsHTML`).
/// Step 1 (3a) draws the current one as a 14 x 6 pill and centres the row.
struct FirstStartDots: View {
    let current: Int
    var pill = false
    var still = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        ZStack(alignment: .leading) {
            HStack(spacing: 6) {
                ForEach(1...FirstStartModel.stepCount, id: \.self) { n in
                    Circle().fill(WhooshPalette.tertiaryLabel).opacity(0.55).frame(width: 6, height: 6)
                        .opacity(n == current ? 0 : 1)
                }
            }
            Capsule().fill(WhooshPalette.accent).frame(width: pill ? 14 : 6, height: 6)
                .offset(x: CGFloat(current - 1) * 12)
                .animation(WhooshMotion.animation(.timingCurve(0.22, 1, 0.36, 1, duration: FirstStartLook.morphS), reduceMotion: reduceMotion || still), value: current)
        }
        .frame(height: 6)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(FirstStartWords.stepOf(current)))
    }
}

/// A small glass pill on the stage: the sample cards' lighter glass (2b `.gl`).
struct FirstStartSampleGlass: ViewModifier {
    let radius: CGFloat
    @Environment(\.colorScheme) private var scheme
    func body(content: Content) -> some View {
        let shape = RoundedRectangle(cornerRadius: radius, style: .continuous)
        content
            .background(shape.fill(FirstStartLook.sampleTint.color(scheme)))
            .whooshGlass(in: shape)
    }
}

extension View {
    func firstStartSample(radius: CGFloat = 12) -> some View { modifier(FirstStartSampleGlass(radius: radius)) }
}

/// 3a's entrance, one piece of it: the view arrives from `scale` and `rise` points below, fading in, after `delay`. It plays once, when the
/// view first appears (the flow opening, or Back to step 1); under Reduced Motion or Low Power it is a plain fade of 160 ms (3a).
struct FirstStartEntrance: ViewModifier {
    let from: CGFloat
    let rise: CGFloat
    let duration: Double
    let delay: Double
    let still: Bool
    @State private var entered = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func body(content: Content) -> some View {
        let cut = still || reduceMotion
        content
            .scaleEffect(entered || cut ? 1 : from)
            .offset(y: entered || cut ? 0 : rise)
            .opacity(entered ? 1 : 0)
            .onAppear {
                withAnimation(.timingCurve(0.2, 0.8, 0.2, 1, duration: cut ? 0.16 : duration).delay(cut ? 0 : delay)) { entered = true }
            }
    }
}

extension View {
    func firstStartEntrance(from: CGFloat, rise: CGFloat = 0, duration: Double, delay: Double, still: Bool) -> some View {
        modifier(FirstStartEntrance(from: from, rise: rise, duration: duration, delay: delay, still: still))
    }
}
