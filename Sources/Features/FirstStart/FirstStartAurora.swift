import SwiftUI

/// The loop clock: `t` in seconds from the step's own start, 0 to 8, so the last frame is the first. A frozen loop (Reduced Motion,
/// Low Power, the rig's switch) holds the board's key frame and the timeline stops, so nothing redraws.
struct FirstStartClock<Content: View>: View {
    let model: FirstStartModel
    let step: Int
    @ViewBuilder let content: (Double) -> Content
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let frozen = reduceMotion || model.frozen
        let start = model.stepStart
        TimelineView(.animation(minimumInterval: 1.0 / 60.0, paused: frozen)) { ctx in
            let raw = ctx.date.timeIntervalSince(start)
            content(frozen ? FirstStartLook.keyTime(step) : raw.truncatingRemainder(dividingBy: FirstStartLook.loopS))
        }
    }
}

/// A static grid of 7 pt dots, the aurora's mask (2b `.ad`): drawn once, never per frame.
struct FirstStartDotMask: View, Equatable {
    let radius: CGFloat
    var body: some View {
        Canvas { ctx, size in
            let pitch: CGFloat = 7
            var path = Path()
            var y = pitch / 2
            while y < size.height {
                var x = pitch / 2
                while x < size.width {
                    path.addEllipse(in: CGRect(x: x - radius, y: y - radius, width: radius * 2, height: radius * 2))
                    x += pitch
                }
                y += pitch
            }
            ctx.fill(path, with: .color(.black))
        }
    }
}

/// The aurora ground of a lit pane (2b `aur()`): the send wave's four colours in four blurred radial layers drifting on the 8 s loop
/// (transform only), seen through the static dot mask, with a faint unmasked glow under them. Light and dark use the same tokens.
struct FirstStartAuroraGround: View {
    let t: Double
    @Environment(\.colorScheme) private var scheme

    /// 2b `.b1` to `.b4`: left and top as a share of the box, and the drift `[x %, y %, scale]` at the middle of the loop
    private static let layout: [(left: CGFloat, top: CGFloat)] = [(0, 0.04), (0.34, -0.06), (0.44, 0.40), (0.06, 0.46)]
    private static let drift: [(x: CGFloat, y: CGFloat, s: CGFloat)] = [(0.09, 0.06, 1.12), (-0.08, 0.10, 1.08), (-0.10, -0.07, 1.15), (0.08, -0.09, 1.10)]

    var body: some View {
        let dark = scheme == .dark
        let p = 0.5 - 0.5 * cos(2 * Double.pi * t / FirstStartLook.loopS)
        ZStack {
            Rectangle().fill(FirstStartLook.stage.color(scheme))
            blobs(p).opacity(dark ? FirstStartLook.glow.dark : FirstStartLook.glow.light)
            blobs(p).mask(FirstStartDotMask(radius: CGFloat(dark ? FirstStartLook.dotRadius.dark : FirstStartLook.dotRadius.light)).equatable())
        }
        .clipped()
    }

    private func blobs(_ p: Double) -> some View {
        GeometryReader { g in
            let w = g.size.width * 1.2, h = g.size.height * 1.2
            ZStack(alignment: .topLeading) {
                ForEach(0..<4, id: \.self) { i in blob(i, w: w, h: h, p: CGFloat(p)) }
            }
            .frame(width: w, height: h, alignment: .topLeading)
            .offset(x: -g.size.width * 0.1, y: -g.size.height * 0.1)
        }
    }

    private func blob(_ i: Int, w: CGFloat, h: CGFloat, p: CGFloat) -> some View {
        let bw = w * 0.56, bh = h * 0.56
        let l = Self.layout[i], d = Self.drift[i]
        let c = FirstStartLook.aurora[i].color(scheme)
        return Circle()
            .fill(RadialGradient(colors: [c, c.opacity(0)], center: .center, startRadius: 0, endRadius: min(bw, bh) / 2))
            .frame(width: min(bw, bh), height: min(bw, bh))
            .scaleEffect(1 + (d.s - 1) * p)
            .offset(x: l.left * w + d.x * bw * p, y: l.top * h + d.y * bh * p)
    }
}

/// 3a's halftone halo: dots on a 7 pt hexagonal grid that shrink with distance from the medallion, in the aurora's four colours; the
/// colours swing 40 degrees and back and the halo breathes 3.5 % on the 8 s loop; below the card's edge the same colours are a soft glow.
struct FirstStartHalo: View {
    let t: Double
    let size: CGFloat
    let inner: CGFloat
    @Environment(\.colorScheme) private var scheme

    /// 3a `.hc i.b1` to `.b4`: left, top as a share of the 64 % blobs' box
    private static let layout: [(left: CGFloat, top: CGFloat)] = [(0.18, -0.06), (0.42, 0.18), (0.18, 0.42), (-0.06, 0.18)]

    var body: some View {
        let p = 0.5 - 0.5 * cos(2 * Double.pi * t / FirstStartLook.loopS)
        ZStack {
            blobs(p).mask(RadialGradient(stops: [.init(color: .clear, location: 0.28), .init(color: .black, location: 0.42), .init(color: .clear, location: 0.96)],
                                         center: .center, startRadius: 0, endRadius: size / 2)).opacity(scheme == .dark ? 0.34 : 0.30)
            blobs(p).mask(FirstStartHalftone(size: size, inner: inner).equatable())
        }
        .frame(width: size, height: size)
        .scaleEffect(1 + 0.035 * p)
        .opacity(0.86 + 0.14 * p)
        .allowsHitTesting(false)
        .accessibilityHidden(true)
    }

    private func blobs(_ p: Double) -> some View {
        let box = size * 1.12, b = box * 0.64
        return ZStack(alignment: .topLeading) {
            ForEach(0..<4, id: \.self) { i in
                let c = FirstStartLook.aurora[i].color(scheme)
                Circle().fill(RadialGradient(colors: [c, c.opacity(0)], center: .center, startRadius: 0, endRadius: b / 2))
                    .frame(width: b, height: b)
                    .offset(x: Self.layout[i].left * box, y: Self.layout[i].top * box)
            }
        }
        .frame(width: box, height: box, alignment: .topLeading)
        .rotationEffect(.degrees(40 * p))
        .frame(width: size, height: size)
    }
}

/// The halo's mask: 3a `halftone()`: radius `2.25 (1 - u)^1.25` clipped to 0 at the inner edge, no dot under 0.32 pt. Static, drawn once.
struct FirstStartHalftone: View, Equatable {
    let size: CGFloat
    let inner: CGFloat
    var body: some View {
        Canvas { ctx, sz in
            let pitch: CGFloat = 7, cx = sz.width / 2, cy = sz.height / 2, outer = size / 2
            var path = Path()
            let rows = Int((sz.height / (pitch * 0.866)).rounded(.up))
            for j in -1...rows {
                let y = CGFloat(j) * pitch * 0.866
                let off: CGFloat = j % 2 == 0 ? 0 : pitch / 2
                var x = -pitch
                while x <= sz.width + pitch {
                    let px = x + off
                    let d = ((px - cx) * (px - cx) + (y - cy) * (y - cy)).squareRoot()
                    let u = (d - inner) / (outer - inner)
                    if u > 0 && u < 1 {
                        let r = 2.25 * pow(1 - u, 1.25) * min(1, u / 0.07)
                        if r >= 0.32 { path.addEllipse(in: CGRect(x: px - r, y: y - r, width: r * 2, height: r * 2)) }
                    }
                    x += pitch
                }
            }
            ctx.fill(path, with: .color(.black))
        }
    }
}
