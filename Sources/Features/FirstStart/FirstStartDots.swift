import SwiftUI

/// A small seeded random source: the boards' `rng(seed)`, so every launch draws the same dots and a loop is a loop.
struct FirstStartRng {
    private var s: UInt64
    init(_ seed: Int) { s = UInt64(seed) }
    mutating func next() -> Double {
        s = (s * 16807) % 2147483647
        return Double(s - 1) / 2147483646
    }
}

/// The dot art's vocabulary (2b `converge`, `shatter`, `stream`): each draws into a Canvas at loop time `t`, transform and opacity only,
/// every dot back where it started at t = 8.
enum FirstStartParticles {
    /// 2b `converge`: loose dots fly into one target at `tIn`, and re-emerge at `tOut` (each takes 260 ms, a little apart)
    static func converge(_ ctx: GraphicsContext, t: Double, colours: [Color], n: Int, seed: Int, area: CGRect, target: CGPoint, tIn: Double, tOut: Double) {
        var r = FirstStartRng(seed)
        for i in 0..<n {
            let px = area.minX + r.next() * area.width, py = area.minY + r.next() * area.height
            let d = (r.next() * 0.12 * 100).rounded() / 100
            // 0 = at rest, 1 = in the target
            let into = FirstStartLook.run(t, tIn + d, tIn + d + 0.26)
            let out = FirstStartLook.run(t, tOut, tOut + 0.26 + d)
            let k = into * (1 - out)
            let x = px + (target.x - px) * k, y = py + (target.y - py) * k
            let scale = 1 - 0.6 * k, alpha = 0.95 * (1 - k)
            dot(ctx, x: x, y: y, radius: 2 * scale, colour: colours[i % 4].opacity(alpha))
        }
    }

    /// 2b `shatter`: a card leaves as streaks flying away (280 ms) from a box, along `dir`
    static func shatter(_ ctx: GraphicsContext, t: Double, colours: [Color], n: Int, seed: Int, box: CGRect, at: Double, dir: CGPoint) {
        var r = FirstStartRng(seed)
        let ang = Angle(radians: Double(atan2(dir.y, dir.x)))
        for i in 0..<n {
            let sx = box.minX + r.next() * box.width, sy = box.minY + r.next() * box.height
            let ex = sx + dir.x * (0.6 + r.next() * 0.8), ey = sy + dir.y * (0.6 + r.next() * 0.8) + (r.next() * 30 - 15)
            let d = (r.next() * 0.09 * 100).rounded() / 100
            let run = (t - (at + d)) / 0.28
            guard run > 0 && run < 1 else { continue }
            let k = 1 - pow(1 - run, 3)
            let x = sx + (ex - sx) * k, y = sy + (ey - sy) * k
            let wide: CGFloat = i % 3 == 0 ? 5 : 10
            var c = ctx
            c.translateBy(x: x, y: y)
            c.rotate(by: ang)
            let rect = CGRect(x: -wide / 2, y: -1.25, width: wide, height: 2.5)
            c.fill(Path(roundedRect: rect, cornerRadius: 1.25), with: .color(colours[i % 4].opacity(1 - k)))
        }
    }

    /// 2b `stream`: a short line of dots running from one point to another, 50 ms apart, 240 ms each
    static func stream(_ ctx: GraphicsContext, t: Double, colours: [Color], n: Int, from: CGPoint, to: CGPoint, at: Double) {
        for i in 0..<n {
            let run = (t - (at + Double(i) * 0.05 + 0.02)) / 0.24
            guard run > 0 && run < 1 else { continue }
            let k = 1 - pow(1 - run, 2)
            dot(ctx, x: from.x + (to.x - from.x) * k, y: from.y + (to.y - from.y) * k, radius: 2, colour: colours[i % 4].opacity(1 - 0.8 * k))
        }
    }

    static func dot(_ ctx: GraphicsContext, x: Double, y: Double, radius: Double, colour: Color) {
        ctx.fill(Path(ellipseIn: CGRect(x: x - radius, y: y - radius, width: radius * 2, height: radius * 2)), with: .color(colour))
    }
}

/// The glass line of a sample message on the stage: sender and time, subject, and an optional third line (2b `.rc`, `.lc`).
struct FirstStartSampleLine: View {
    let line: FirstStartLine
    var selected = 0.0
    var third: AnyView? = nil

    var body: some View {
        ZStack(alignment: .topLeading) {
            Rectangle().fill(WhooshPalette.selection).opacity(selected).clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
            content(white: false).opacity(1 - selected)
            content(white: true).opacity(selected)
        }
        .firstStartSample(radius: 12)
    }

    private func content(white: Bool) -> some View {
        let primary: Color = white ? .white : Color.primary
        return VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline, spacing: 6) {
                Text(verbatim: line.sender).font(.system(size: 13, weight: .semibold)).foregroundStyle(white ? AnyShapeStyle(primary) : AnyShapeStyle(WhooshPalette.label))
                Spacer(minLength: 0)
                Text(verbatim: line.time).font(.system(size: 11)).foregroundStyle(white ? AnyShapeStyle(primary) : AnyShapeStyle(WhooshPalette.secondaryLabel))
            }
            Text(verbatim: line.subject).font(.system(size: 12.5)).lineLimit(1)
                .foregroundStyle(white ? AnyShapeStyle(primary) : AnyShapeStyle(WhooshPalette.label)).opacity(0.86)
            if let third { third.padding(.top, 8) }
        }
        .padding(.top, 10).padding(.leading, 16).padding(.trailing, 14)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

/// "SAMPLE MAIL": the tag on every stage that shows sample lines (2b `.tagp`).
struct FirstStartSampleTag: View {
    var body: some View {
        Text(FirstStartWords.sampleMail).textCase(.uppercase).font(.system(size: 10.5, weight: .semibold)).tracking(0.2)
            .foregroundStyle(WhooshPalette.secondaryLabel).padding(.horizontal, 7).padding(.vertical, 2).firstStartSample(radius: 6)
    }
}
