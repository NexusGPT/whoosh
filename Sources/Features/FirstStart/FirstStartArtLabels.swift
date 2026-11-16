import SwiftUI

/// Step 4 (2b): two dotted sources on the stage, "Your rules" and "The model on this Mac". A stream of 7 dots runs from the source that
/// decided into the sample line's label chip, and the chip pops in; "decided by" fades in beside it. The sidebar label lights at the
/// same moment (FirstStartSidebarLights). 8 s, last frame = first.
struct FirstStartLabelsArt: View {
    let geo: FirstStartGeometry
    let t: Double
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let w = geo.listW
        let top = FirstStartGeometry.stageTop
        let a = CGPoint(x: (w * 0.27).rounded(), y: top + 58), b = CGPoint(x: (w * 0.73).rounded(), y: top + 58)
        let yA = top + 150, yB = top + 236
        ZStack(alignment: .topLeading) {
            node(a, "line.3.horizontal.decrease", FirstStartWords.yourRules, FirstStartWords.nodeFirst, at: 0.6)
            node(b, "sparkles", FirstStartWords.nodeModel, FirstStartWords.nodeOnThisMac, at: 2.4)
            line("m9", y: yA, label: FirstStartSample.chipUpdates, decided: FirstStartWords.decidedByRule(FirstStartSample.ruleName), icon: "line.3.horizontal.decrease", at: 0.6, w: w)
            line("m4", y: yB, label: FirstStartSample.chipInvoices, decided: FirstStartWords.decidedByModel, icon: "sparkles", at: 2.4, w: w)
            Canvas { ctx, _ in
                let c = FirstStartLook.aurora.map { $0.color(scheme) }
                FirstStartParticles.stream(ctx, t: t, colours: c, n: 7, from: CGPoint(x: a.x, y: a.y + 4), to: CGPoint(x: 66, y: yA + 56), at: 0.6)
                FirstStartParticles.stream(ctx, t: t, colours: c, n: 7, from: CGPoint(x: b.x, y: b.y + 4), to: CGPoint(x: 66, y: yB + 56), at: 2.4)
            }
            FirstStartSampleTag().offset(x: w - 16 - 84, y: geo.size.height - 14 - 22)
        }
        .allowsHitTesting(false)
    }

    /// a dotted ring with an icon in it, and its two-line name under it (2b `node`); it pulses when its stream starts
    private func node(_ at: CGPoint, _ icon: String, _ title: String, _ small: String, at t0: Double) -> some View {
        let pulse = 1 + 0.08 * (FirstStartLook.run(t, t0, t0 + 0.08) * (1 - FirstStartLook.run(t, t0 + 0.08, t0 + 0.3)))
        return VStack(spacing: 6) {
            ZStack {
                Circle().fill(Color.clear).frame(width: 56, height: 56).firstStartSampleCircle()
                Circle().stroke(AngularGradient(colors: FirstStartLook.aurora.map { $0.color(scheme) } + [FirstStartLook.aurora[0].color(scheme)], center: .center),
                                style: StrokeStyle(lineWidth: 2.6, lineCap: .round, dash: [0, 5.6])).frame(width: 50, height: 50)
                Image(systemName: icon).font(.system(size: 18)).foregroundStyle(WhooshPalette.label)
            }
            .scaleEffect(pulse)
            VStack(spacing: 0) {
                Text(title).font(.system(size: 11.5, weight: .medium)).foregroundStyle(WhooshPalette.label)
                Text(small).font(.system(size: 11)).foregroundStyle(WhooshPalette.secondaryLabel)
            }
            .padding(.horizontal, 8).padding(.vertical, 3).firstStartSample(radius: 8)
        }
        .frame(width: 120)
        .offset(x: at.x - 60, y: at.y - 28)
    }

    /// a sample line with its label chip and "decided by" (2b `lc`): the chip pops in 300 ms after the stream starts
    private func line(_ id: String, y: CGFloat, label: String, decided: String, icon: String, at t0: Double, w: CGFloat) -> some View {
        let chipOn = FirstStartLook.run(t, t0 + 0.3, t0 + 0.46) * (1 - FirstStartLook.run(t, 6.4, 6.55))
        let decOn = FirstStartLook.win(t, t0 + 0.5, 6.4, 0.15)
        let third = AnyView(HStack(spacing: 8) {
            HStack(spacing: 4) {
                Image(systemName: "tag").font(.system(size: 11)).foregroundStyle(WhooshPalette.secondaryLabel)
                Text(label).font(.system(size: 11)).foregroundStyle(WhooshPalette.label)
            }
            .padding(.horizontal, 7).frame(height: 18).background(WhooshPalette.background, in: Capsule())
            .scaleEffect(0.9 + 0.1 * chipOn).opacity(chipOn)
            HStack(spacing: 5) {
                Image(systemName: icon).font(.system(size: 11)).foregroundStyle(WhooshPalette.accent)
                Text(decided).font(.system(size: 11.5)).foregroundStyle(WhooshPalette.secondaryLabel).lineLimit(1)
            }.opacity(decOn)
        }.frame(height: 20))
        return Group {
            if let l = FirstStartSample.line(id) {
                FirstStartSampleLine(line: l, third: third).frame(width: w - 32, height: 76).offset(x: 16, y: y)
            }
        }
    }
}

extension View {
    /// the lighter glass in a circle (the node's ring ground)
    func firstStartSampleCircle() -> some View { modifier(FirstStartSampleGlass(radius: 28)) }
}

/// The sidebar label that lights as the stream arrives: "Updates" from 0.9 to 2.2 s, "Invoices" from 2.7 to 4.0 s (2b `.sbt`), drawn over
/// the real sidebar at the board's row positions (the shell's rows are 24 pt from y 294; re-measured on the rig, see mac-first-start.json).
struct FirstStartSidebarLights: View {
    let geo: FirstStartGeometry
    let t: Double
    var body: some View {
        let compactRows = geo.size.height < 700
        let updates: CGFloat = FirstStartGeometry.toolbar + 342
        let invoices: CGFloat = FirstStartGeometry.toolbar + (compactRows ? 462 : 486)
        ZStack(alignment: .topLeading) {
            light(y: updates, on: FirstStartLook.win(t, 0.9, 2.2, 0.12))
            light(y: invoices, on: FirstStartLook.win(t, 2.7, 4.0, 0.12))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .allowsHitTesting(false)
    }
    private func light(y: CGFloat, on: Double) -> some View {
        RoundedRectangle(cornerRadius: 7, style: .continuous).fill(WhooshPalette.accent.opacity(0.16))
            .frame(width: geo.sidebarW - 20, height: 24).offset(x: 10, y: y).opacity(on)
    }
}
