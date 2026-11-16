import SwiftUI

/// Step 5 (2b): the reader becomes a stage. A dotted Mac holds your mail, the index and the model as three rings of dots orbiting
/// inside it; one dot runs to the edge, ripples and turns back (0.9 s each way, loop content that answers nothing). The two optional
/// parts, the Jev key and the push relay, sit outside, dotted, unconnected and off. 8 s, last frame = first.
struct FirstStartPrivacyArt: View {
    let geo: FirstStartGeometry
    let t: Double
    @Environment(\.colorScheme) private var scheme

    /// the board's numbers for the Mac's drawing (2b `'5'`), in the reader pane's own coordinates
    private struct Plan {
        let sx: CGFloat, sy: CGFloat, sw: CGFloat, sh: CGFloat, cy: CGFloat, r: CGFloat
        let cxs: [CGFloat], lx: CGFloat, rx: CGFloat
    }

    private func plan() -> Plan {
        let rw = geo.readerW, H = geo.size.height
        let ch: CGFloat = H >= 800 ? 318 : 330
        let top: CGFloat = 48
        let ah = H - ch - 16 - 12 - top
        let sw = (min(rw * 0.5, ah * 1.3)).rounded(), sh = (sw * 0.6).rounded()
        let sx = ((rw - sw) / 2).rounded(), sy = (top + (ah - sh - 18) / 2).rounded()
        let r = max(12, min(22, (sh * 0.16).rounded()))
        let cxs = [0.24, 0.5, 0.76].map { sx + (sw * $0).rounded() }
        return Plan(sx: sx, sy: sy, sw: sw, sh: sh, cy: sy + (sh * 0.44).rounded(), r: r, cxs: cxs, lx: (sx / 2).rounded(), rx: (sx + sw + (rw - sx - sw) / 2).rounded())
    }

    var body: some View {
        let p = plan()
        let colours = FirstStartLook.aurora.map { $0.color(scheme) }
        let grad = LinearGradient(colors: colours, startPoint: .topLeading, endPoint: .bottomTrailing)
        ZStack(alignment: .topLeading) {
            drawing(p, grad)
            Canvas { ctx, _ in orbits(ctx, p, colours); traveller(ctx, p, colours) }
            labels(p)
        }
        .frame(width: geo.readerW, height: geo.size.height, alignment: .topLeading)
        .allowsHitTesting(false)
    }

    // the dotted Mac, its foot, the two lines and the two optional circles
    private func drawing(_ p: Plan, _ grad: LinearGradient) -> some View {
        let dots = StrokeStyle(lineWidth: 4.2, lineCap: .round, dash: [0, 7])
        let fine = StrokeStyle(lineWidth: 2.6, lineCap: .round, dash: [0, 5])
        let mid = p.sy + p.sh / 2
        var foot = Path()
        foot.move(to: CGPoint(x: p.sx - 22, y: p.sy + p.sh + 14)); foot.addLine(to: CGPoint(x: p.sx + p.sw + 22, y: p.sy + p.sh + 14))
        foot.addLine(to: CGPoint(x: p.sx + p.sw + 8, y: p.sy + p.sh + 4)); foot.addLine(to: CGPoint(x: p.sx - 8, y: p.sy + p.sh + 4)); foot.closeSubpath()
        var l1 = Path(); l1.move(to: CGPoint(x: p.lx + 22, y: mid)); l1.addLine(to: CGPoint(x: p.sx - 14, y: mid))
        var l2 = Path(); l2.move(to: CGPoint(x: p.sx + p.sw + 14, y: mid)); l2.addLine(to: CGPoint(x: p.rx - 22, y: mid))
        return ZStack(alignment: .topLeading) {
            RoundedRectangle(cornerRadius: 12, style: .continuous).fill(FirstStartLook.stage.color(scheme).opacity(0.94))
                .frame(width: p.sw, height: p.sh).offset(x: p.sx, y: p.sy)
            RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(grad, style: dots).frame(width: p.sw, height: p.sh).offset(x: p.sx, y: p.sy)
            foot.fill(FirstStartLook.stage.color(scheme).opacity(0.94))
            foot.stroke(grad, style: dots)
            l1.stroke(grad, style: StrokeStyle(lineWidth: 3, lineCap: .round, dash: [0, 7])).opacity(0.7)
            l2.stroke(grad, style: StrokeStyle(lineWidth: 3, lineCap: .round, dash: [0, 7])).opacity(0.7)
            ForEach([p.lx, p.rx], id: \.self) { x in
                Circle().fill(FirstStartLook.stage.color(scheme)).frame(width: 34, height: 34).offset(x: x - 17, y: mid - 17)
                Circle().stroke(grad, style: fine).frame(width: 34, height: 34).offset(x: x - 17, y: mid - 17).opacity(0.8)
            }
        }
    }

    /// three rings of ten dots, turning once per loop (the middle one the other way), in the Mac
    private func orbits(_ ctx: GraphicsContext, _ p: Plan, _ colours: [Color]) {
        for k in 0..<3 {
            let turn = (k % 2 == 0 ? 1.0 : -1.0) * 2 * Double.pi * t / FirstStartLook.loopS
            for j in 0..<10 {
                let a = Double(j) * Double.pi / 5 + turn
                FirstStartParticles.dot(ctx, x: Double(p.cxs[k]) + cos(a) * Double(p.r), y: Double(p.cy) + sin(a) * Double(p.r), radius: 2.5, colour: colours[(j + k) % 4])
            }
        }
    }

    /// 2b `b5t` and `b5r`: a dot leaves the model ring at 0.6 s, runs to the Mac's edge by 1.5 s, ripples, and is back by 2.4 s
    private func traveller(_ ctx: GraphicsContext, _ p: Plan, _ colours: [Color]) {
        let dx = Double(p.sx + p.sw - 14 - p.cxs[2])
        let x = FirstStartLook.track(t, [(0, 0), (0.5, 0), (0.62, 0), (1.5, dx), (2.4, 0)])
        let alpha = FirstStartLook.track(t, [(0, 0), (0.5, 0), (0.62, 1), (2.4, 1), (2.52, 0)])
        FirstStartParticles.dot(ctx, x: Double(p.cxs[2]) + x, y: Double(p.cy), radius: 3, colour: colours[2].opacity(alpha))
        let scale = FirstStartLook.track(t, [(0, 0.4), (1.45, 0.4), (1.5, 0.6), (2.0, 1.6), (2.001, 0.4)])
        let ring = FirstStartLook.track(t, [(0, 0), (1.45, 0), (1.5, 1), (2.0, 0), (2.001, 0)])
        let rr = 13.0 * scale
        ctx.stroke(Path(ellipseIn: CGRect(x: Double(p.sx + p.sw - 14) - rr, y: Double(p.cy) - rr, width: rr * 2, height: rr * 2)), with: .color(colours[2].opacity(ring)), lineWidth: 1.5)
    }

    private func labels(_ p: Plan) -> some View {
        ZStack(alignment: .topLeading) {
            ForEach(0..<3, id: \.self) { i in
                small([FirstStartWords.artMail, FirstStartWords.artIndex, FirstStartWords.artModel][i]).position(x: p.cxs[i], y: p.cy + p.r + 8 + 7)
            }
            small(FirstStartWords.thisMac, bold: true).position(x: geo.readerW / 2, y: p.sy + p.sh + 24 + 7)
            off(FirstStartWords.jevKey).position(x: p.lx, y: p.sy + p.sh / 2 + 26 + 15)
            off(FirstStartWords.pushRelay).position(x: p.rx, y: p.sy + p.sh / 2 + 26 + 15)
        }
    }

    private func small(_ text: String, bold: Bool = false) -> some View {
        Text(text).font(.system(size: 11, weight: bold ? .semibold : .regular)).foregroundStyle(bold ? WhooshPalette.label : WhooshPalette.secondaryLabel).fixedSize()
    }

    private func off(_ title: String) -> some View {
        VStack(spacing: 4) {
            Text(title).font(.system(size: 11)).foregroundStyle(WhooshPalette.secondaryLabel)
            Text(FirstStartWords.off).font(.system(size: 11, weight: .semibold)).foregroundStyle(WhooshPalette.secondaryLabel)
        }.opacity(0.75).fixedSize()
    }
}
