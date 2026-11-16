import SwiftUI

/// A small SVG path-data reader (M L H V C S Z, lower case too, and A), so the provider marks keep the board's drawing verbatim.
enum SVGPathData {
    static func path(_ d: String) -> Path {
        var s = Reader(Array(d)), p = Path()
        var cur = CGPoint.zero, start = CGPoint.zero, lastCtl: CGPoint?
        var cmd: Character = "M"
        while true {
            s.skip()
            guard s.i < s.c.count else { break }
            if s.c[s.i].isLetter { cmd = s.c[s.i]; s.i += 1 }
            let rel = cmd.isLowercase
            func abs(_ x: CGFloat, _ y: CGFloat) -> CGPoint { rel ? CGPoint(x: cur.x + x, y: cur.y + y) : CGPoint(x: x, y: y) }
            switch Character(cmd.uppercased()) {
            case "M":
                guard let x = s.num(), let y = s.num() else { return p }
                cur = abs(x, y); start = cur; p.move(to: cur); cmd = rel ? "l" : "L"; lastCtl = nil
            case "L":
                guard let x = s.num(), let y = s.num() else { return p }
                cur = abs(x, y); p.addLine(to: cur); lastCtl = nil
            case "H":
                guard let x = s.num() else { return p }
                cur = CGPoint(x: rel ? cur.x + x : x, y: cur.y); p.addLine(to: cur); lastCtl = nil
            case "V":
                guard let y = s.num() else { return p }
                cur = CGPoint(x: cur.x, y: rel ? cur.y + y : y); p.addLine(to: cur); lastCtl = nil
            case "C":
                guard let a = s.num(), let b = s.num(), let c = s.num(), let d = s.num(), let e = s.num(), let f = s.num() else { return p }
                let c1 = abs(a, b), c2 = abs(c, d), to = abs(e, f)
                p.addCurve(to: to, control1: c1, control2: c2); cur = to; lastCtl = c2
            case "S":
                guard let c = s.num(), let d = s.num(), let e = s.num(), let f = s.num() else { return p }
                let c1 = lastCtl.map { CGPoint(x: 2 * cur.x - $0.x, y: 2 * cur.y - $0.y) } ?? cur
                let c2 = abs(c, d), to = abs(e, f)
                p.addCurve(to: to, control1: c1, control2: c2); cur = to; lastCtl = c2
            case "A":
                guard let rx = s.num(), let ry = s.num(), let rot = s.num(), let large = s.num(), let sweep = s.num(), let x = s.num(), let y = s.num() else { return p }
                let to = abs(x, y)
                arc(&p, from: cur, rx: rx, ry: ry, phi: rot, large: large != 0, sweep: sweep != 0, to: to); cur = to; lastCtl = nil
            case "Z":
                p.closeSubpath(); cur = start; lastCtl = nil
            default: return p
            }
        }
        return p
    }

    private struct Reader {
        let c: [Character]; var i = 0
        init(_ c: [Character]) { self.c = c }
        mutating func skip() { while i < c.count, c[i] == " " || c[i] == "," || c[i] == "\n" { i += 1 } }
        mutating func num() -> CGFloat? {
            skip()
            var t = ""
            if i < c.count, c[i] == "-" || c[i] == "+" { t.append(c[i]); i += 1 }
            var dot = false
            while i < c.count {
                if c[i].isNumber { t.append(c[i]) } else if c[i] == ".", !dot { dot = true; t.append(c[i]) } else { break }
                i += 1
            }
            return Double(t).map { CGFloat($0) }
        }
    }

    /// The endpoint form of an elliptical arc (SVG 1.1 F.6.5), drawn as cubic curves of at most a quarter turn each.
    private static func arc(_ p: inout Path, from a: CGPoint, rx rx0: CGFloat, ry ry0: CGFloat, phi: CGFloat, large: Bool, sweep: Bool, to b: CGPoint) {
        var rx = Swift.abs(rx0), ry = Swift.abs(ry0)
        guard rx > 0, ry > 0, a != b else { p.addLine(to: b); return }
        let ph = phi * .pi / 180, cs = cos(ph), sn = sin(ph)
        let dx = (a.x - b.x) / 2, dy = (a.y - b.y) / 2
        let x1 = cs * dx + sn * dy, y1 = -sn * dx + cs * dy
        let lam = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry)
        if lam > 1 { rx *= lam.squareRoot(); ry *= lam.squareRoot() }
        let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1
        let den = rx * rx * y1 * y1 + ry * ry * x1 * x1
        let k = (large == sweep ? -1 : 1) * (max(0, num / den)).squareRoot()
        let cx1 = k * rx * y1 / ry, cy1 = -k * ry * x1 / rx
        let cx = cs * cx1 - sn * cy1 + (a.x + b.x) / 2, cy = sn * cx1 + cs * cy1 + (a.y + b.y) / 2
        func ang(_ ux: CGFloat, _ uy: CGFloat, _ vx: CGFloat, _ vy: CGFloat) -> CGFloat {
            let t = atan2(ux * vy - uy * vx, ux * vx + uy * vy); return t
        }
        let t1 = ang(1, 0, (x1 - cx1) / rx, (y1 - cy1) / ry)
        var dt = ang((x1 - cx1) / rx, (y1 - cy1) / ry, (-x1 - cx1) / rx, (-y1 - cy1) / ry)
        if !sweep, dt > 0 { dt -= 2 * .pi } else if sweep, dt < 0 { dt += 2 * .pi }
        let n = max(1, Int((Swift.abs(dt) / (.pi / 2)).rounded(.up))), step = dt / CGFloat(n)
        let t = 4 / 3 * tan(step / 4)
        func pt(_ a: CGFloat, _ dx: CGFloat = 0, _ dy: CGFloat = 0) -> CGPoint {
            let x = rx * (cos(a) + dx), y = ry * (sin(a) + dy)
            return CGPoint(x: cs * x - sn * y + cx, y: sn * x + cs * y + cy)
        }
        for i in 0..<n {
            let s0 = t1 + step * CGFloat(i), s1 = s0 + step
            p.addCurve(to: i == n - 1 ? b : pt(s1), control1: pt(s0, -t * sin(s0), t * cos(s0)), control2: pt(s1, t * sin(s1), -t * cos(s1)))
        }
    }
}

/// A path in a viewBox, scaled to fill the frame it is given.
struct SVGShape: Shape {
    let d: String
    let box: CGFloat
    func path(in rect: CGRect) -> Path {
        SVGPathData.path(d).applying(CGAffineTransform(scaleX: rect.width / box, y: rect.height / box))
    }
}
