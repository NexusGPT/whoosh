import SwiftUI

/// An SVG path's `d`, drawn in its own viewBox and scaled to fit the frame (centred). It reads M L H V C S Q T A Z,
/// absolute and relative: enough for the vendors' official marks (ProviderMarks.swift), which are drawn from their own data.
struct SVGPathShape: Shape {
    let d: String
    let viewBox: CGSize

    func path(in rect: CGRect) -> Path {
        let k = min(rect.width / viewBox.width, rect.height / viewBox.height)
        let dx = rect.minX + (rect.width - viewBox.width * k) / 2, dy = rect.minY + (rect.height - viewBox.height * k) / 2
        return SVGPathParser.parse(d).applying(CGAffineTransform(a: k, b: 0, c: 0, d: k, tx: dx, ty: dy))
    }
}

enum SVGPathParser {
    enum Token { case cmd(Character), num(CGFloat) }

    static func parse(_ d: String) -> Path {
        var p = Path()
        let t = tokens(d)
        var i = 0, cmd: Character = "M"
        var cur = CGPoint.zero, start = CGPoint.zero, cubic: CGPoint?, quad: CGPoint?
        func num() -> CGFloat { defer { i += 1 }; if i < t.count, case .num(let v) = t[i] { return v }; return 0 }
        while i < t.count {
            let before = i
            if case .cmd(let c) = t[i] { cmd = c; i += 1 }
            let rel = cmd.isLowercase
            func pt() -> CGPoint { let x = num(), y = num(); return rel ? CGPoint(x: cur.x + x, y: cur.y + y) : CGPoint(x: x, y: y) }
            func mirror(_ c: CGPoint?) -> CGPoint { c.map { CGPoint(x: 2 * cur.x - $0.x, y: 2 * cur.y - $0.y) } ?? cur }
            switch Character(cmd.uppercased()) {
            case "M": cur = pt(); start = cur; p.move(to: cur); cmd = rel ? "l" : "L"; cubic = nil; quad = nil
            case "L": cur = pt(); p.addLine(to: cur); cubic = nil; quad = nil
            case "H": let x = num(); cur = CGPoint(x: rel ? cur.x + x : x, y: cur.y); p.addLine(to: cur); cubic = nil; quad = nil
            case "V": let y = num(); cur = CGPoint(x: cur.x, y: rel ? cur.y + y : y); p.addLine(to: cur); cubic = nil; quad = nil
            case "C": let c1 = pt(), c2 = pt(), e = pt(); p.addCurve(to: e, control1: c1, control2: c2); cubic = c2; quad = nil; cur = e
            case "S": let c1 = mirror(cubic), c2 = pt(), e = pt(); p.addCurve(to: e, control1: c1, control2: c2); cubic = c2; quad = nil; cur = e
            case "Q": let c = pt(), e = pt(); p.addQuadCurve(to: e, control: c); quad = c; cubic = nil; cur = e
            case "T": let c = mirror(quad), e = pt(); p.addQuadCurve(to: e, control: c); quad = c; cubic = nil; cur = e
            case "A":
                let rx = num(), ry = num(), rot = num(), large = num() != 0, sweep = num() != 0, e = pt()
                arc(&p, from: cur, to: e, rx: rx, ry: ry, degrees: rot, large: large, sweep: sweep); cur = e; cubic = nil; quad = nil
            case "Z": p.closeSubpath(); cur = start; cubic = nil; quad = nil
            default: break
            }
            if i == before { i += 1 }   // a stray token never stops the walk
        }
        return p
    }

    /// Numbers and command letters; "-13.56-5.84" and ".5.5" split as SVG does.
    static func tokens(_ d: String) -> [Token] {
        let s = Array(d.utf8)
        var out: [Token] = [], i = 0
        func digit(_ c: UInt8) -> Bool { c >= 48 && c <= 57 }
        while i < s.count {
            let c = s[i]
            if ((c >= 65 && c <= 90) || (c >= 97 && c <= 122)) && c != 69 && c != 101 { out.append(.cmd(Character(UnicodeScalar(c)))); i += 1; continue }
            guard c == 45 || c == 43 || c == 46 || digit(c) else { i += 1; continue }
            var j = i + ((c == 45 || c == 43) ? 1 : 0), dot = false
            while j < s.count, digit(s[j]) || (s[j] == 46 && !dot) { if s[j] == 46 { dot = true }; j += 1 }
            if j < s.count, s[j] == 69 || s[j] == 101 {
                j += 1
                if j < s.count, s[j] == 45 || s[j] == 43 { j += 1 }
                while j < s.count, digit(s[j]) { j += 1 }
            }
            out.append(.num(CGFloat(Double(String(decoding: s[i..<j], as: UTF8.self)) ?? 0)))
            i = j
        }
        return out
    }

    /// The SVG elliptical arc (spec F.6.5, endpoint to centre), as cubic segments of at most a quarter turn.
    static func arc(_ p: inout Path, from p0: CGPoint, to p1: CGPoint, rx rx0: CGFloat, ry ry0: CGFloat, degrees: CGFloat, large: Bool, sweep: Bool) {
        var rx = abs(rx0), ry = abs(ry0)
        guard rx > 0, ry > 0, p0 != p1 else { p.addLine(to: p1); return }
        let phi = degrees * .pi / 180, cp = cos(phi), sp = sin(phi)
        let hx = (p0.x - p1.x) / 2, hy = (p0.y - p1.y) / 2
        let x1 = cp * hx + sp * hy, y1 = -sp * hx + cp * hy
        let lam = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry)
        if lam > 1 { rx *= lam.squareRoot(); ry *= lam.squareRoot() }
        let top = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1, bottom = rx * rx * y1 * y1 + ry * ry * x1 * x1
        let co = max(0, top / bottom).squareRoot() * (large == sweep ? -1 : 1)
        let ccx = co * rx * y1 / ry, ccy = -co * ry * x1 / rx
        let cx = cp * ccx - sp * ccy + (p0.x + p1.x) / 2, cy = sp * ccx + cp * ccy + (p0.y + p1.y) / 2
        func angle(_ ux: CGFloat, _ uy: CGFloat, _ vx: CGFloat, _ vy: CGFloat) -> CGFloat { atan2(ux * vy - uy * vx, ux * vx + uy * vy) }
        let ux = (x1 - ccx) / rx, uy = (y1 - ccy) / ry
        var t = angle(1, 0, ux, uy), dt = angle(ux, uy, (-x1 - ccx) / rx, (-y1 - ccy) / ry)
        if !sweep && dt > 0 { dt -= 2 * .pi } else if sweep && dt < 0 { dt += 2 * .pi }
        let n = max(1, Int((abs(dt) / (.pi / 2)).rounded(.up))), step = dt / CGFloat(n), k = 4 / 3 * tan(step / 4)
        func at(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: cx + rx * x * cp - ry * y * sp, y: cy + rx * x * sp + ry * y * cp) }
        for _ in 0..<n {
            let (c1, s1, c2, s2) = (cos(t), sin(t), cos(t + step), sin(t + step))
            p.addCurve(to: at(c2, s2), control1: at(c1 - k * s1, s1 + k * c1), control2: at(c2 + k * s2, s2 - k * c2))
            t += step
        }
    }
}
