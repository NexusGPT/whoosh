import SwiftUI

/// One calm full screen: the app's glass mark, one title, one line, and at the foot the one main button (and a quiet second action).
/// Paper-white ground; nothing floats here except what the step puts at the foot.
struct WelcomeStepFrame<Art: View, Foot: View>: View {
    let title: String
    let line: String
    let art: Art
    let foot: Foot
    /// A list that takes the space between the line and the foot (the arriving step); nil leaves that space empty.
    let middle: AnyView?

    init(title: String, line: String, middle: AnyView? = nil, @ViewBuilder art: () -> Art, @ViewBuilder foot: () -> Foot) {
        self.title = title
        self.line = line
        self.middle = middle
        self.art = art()
        self.foot = foot()
    }

    var body: some View {
        ZStack {
            PhoneGround.list.ignoresSafeArea()
            VStack(spacing: 0) {
                Spacer(minLength: middle == nil ? WhooshSpacing.s32 : WhooshSpacing.s16)
                art.accessibilityHidden(true)
                VStack(spacing: WhooshSpacing.s8) {
                    Text(title).phoneText(PhoneType.largeTitle).foregroundStyle(WhooshPalette.label)
                        .multilineTextAlignment(.center).accessibilityAddTraits(.isHeader)
                        .accessibilityIdentifier("iphone-welcome.title")
                    Text(line).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.secondaryLabel)
                        .multilineTextAlignment(.center).fixedSize(horizontal: false, vertical: true)
                        .contentTransition(.numericText())
                        .accessibilityIdentifier("iphone-welcome.line")
                }
                .padding(.top, WelcomeMetrics.blockGap * 2)
                .padding(.horizontal, PhoneMetrics.gutter + WhooshSpacing.s8)
                .frame(maxWidth: WelcomeMetrics.contentMaxWidth)
                if let middle {
                    middle.frame(maxWidth: WelcomeMetrics.contentMaxWidth, maxHeight: .infinity).padding(.top, WhooshSpacing.s16)
                } else {
                    Spacer(minLength: WhooshSpacing.s32)
                }
                VStack(spacing: WhooshSpacing.s4) { foot }
                    .padding(.horizontal, PhoneMetrics.gutter)
                    .padding(.bottom, WelcomeMetrics.footBottom)
                    .frame(maxWidth: WelcomeMetrics.contentMaxWidth)
            }
            .frame(maxWidth: .infinity)
        }
    }
}

/// The glass mark every step shows (the Mac's first-start medallion, the shared BrandMark inside it).
struct WelcomeMark: View {
    var size = WelcomeMetrics.markSize
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    var body: some View { FirstStartMedallion(t: 0, size: size, still: reduceMotion) }
}

/// The one main button: a filled capsule at the foot. The press compresses it the same frame (PressStyle).
struct WelcomeMainButton: View {
    let control: WelcomeControl
    var enabled = true
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(Words.text(control.label)).phoneText(PhoneType.headline).foregroundStyle(WhooshPalette.onAccent)
                .frame(maxWidth: .infinity).frame(height: WelcomeMetrics.mainButton)
                .background(Capsule().fill(WhooshPalette.selection))
                .contentShape(Capsule())
        }
        .buttonStyle(PressStyle()).disabled(!enabled).opacity(enabled ? 1 : 0.4)
        .accessibilityIdentifier(control.ax)
    }
}

/// The quiet second action under the main button: words only, 44 pt tall.
struct WelcomeTextButton: View {
    let control: WelcomeControl
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Text(Words.text(control.label)).phoneText(PhoneType.body).foregroundStyle(WhooshPalette.selection)
                .frame(maxWidth: .infinity).frame(height: WelcomeMetrics.textButton)
                .contentShape(Rectangle())
        }
        .buttonStyle(PressStyle())
        .accessibilityIdentifier(control.ax)
    }
}

/// The four corner brackets of the viewfinder.
struct WelcomeBrackets: Shape {
    func path(in r: CGRect) -> Path {
        let l = WelcomeMetrics.frameBracket, c = WelcomeMetrics.frameRadius
        var p = Path()
        p.move(to: CGPoint(x: r.minX, y: r.minY + l)); p.addLine(to: CGPoint(x: r.minX, y: r.minY + c))
        p.addQuadCurve(to: CGPoint(x: r.minX + c, y: r.minY), control: CGPoint(x: r.minX, y: r.minY)); p.addLine(to: CGPoint(x: r.minX + l, y: r.minY))
        p.move(to: CGPoint(x: r.maxX - l, y: r.minY)); p.addLine(to: CGPoint(x: r.maxX - c, y: r.minY))
        p.addQuadCurve(to: CGPoint(x: r.maxX, y: r.minY + c), control: CGPoint(x: r.maxX, y: r.minY)); p.addLine(to: CGPoint(x: r.maxX, y: r.minY + l))
        p.move(to: CGPoint(x: r.maxX, y: r.maxY - l)); p.addLine(to: CGPoint(x: r.maxX, y: r.maxY - c))
        p.addQuadCurve(to: CGPoint(x: r.maxX - c, y: r.maxY), control: CGPoint(x: r.maxX, y: r.maxY)); p.addLine(to: CGPoint(x: r.maxX - l, y: r.maxY))
        p.move(to: CGPoint(x: r.minX + l, y: r.maxY)); p.addLine(to: CGPoint(x: r.minX + c, y: r.maxY))
        p.addQuadCurve(to: CGPoint(x: r.minX, y: r.maxY - c), control: CGPoint(x: r.minX, y: r.maxY)); p.addLine(to: CGPoint(x: r.minX, y: r.maxY - l))
        return p
    }
}
