import CoreGraphics
import Foundation

/// SVG path data (M L H V C Q A Z, absolute and relative) turned into a CGPath. The emblem board is drawn in SVG on a
/// 24-unit grid; reading its path strings verbatim keeps the Swift drawings identical to the owner's picked board.
enum EmblemPath {
    private static let token = try! NSRegularExpression(pattern: "[MmLlHhVvCcQqAaZz]|[-+]?(?:\\d+\\.?\\d*|\\.\\d+)(?:[eE][-+]?\\d+)?")

    static func make(_ d: String) -> CGPath {
        let ns = d as NSString
        let tokens = token.matches(in: d, range: NSRange(location: 0, length: ns.length)).map { ns.substring(with: $0.range) }
        let path = CGMutablePath()
        var i = 0, cmd: Character = "M", cur = CGPoint.zero, start = CGPoint.zero
        func num() -> CGFloat { defer { i += 1 }; return i < tokens.count ? CGFloat(Double(tokens[i]) ?? 0) : 0 }
        func isNum() -> Bool { i < tokens.count && !(tokens[i].first?.isLetter ?? false) }
        while i < tokens.count {
            if let c = tokens[i].first, c.isLetter { cmd = c; i += 1 }
            let rel = cmd.isLowercase
            let o = rel ? cur : .zero
            switch cmd.uppercased().first! {
            case "M":
                cur = CGPoint(x: o.x + num(), y: o.y + num()); start = cur; path.move(to: cur); cmd = rel ? "l" : "L"
            case "L": cur = CGPoint(x: o.x + num(), y: o.y + num()); path.addLine(to: cur)
            case "H": cur.x = (rel ? cur.x : 0) + num(); path.addLine(to: cur)
            case "V": cur.y = (rel ? cur.y : 0) + num(); path.addLine(to: cur)
            case "C":
                let c1 = CGPoint(x: o.x + num(), y: o.y + num()), c2 = CGPoint(x: o.x + num(), y: o.y + num())
                cur = CGPoint(x: o.x + num(), y: o.y + num()); path.addCurve(to: cur, control1: c1, control2: c2)
            case "Q":
                let c1 = CGPoint(x: o.x + num(), y: o.y + num())
                cur = CGPoint(x: o.x + num(), y: o.y + num()); path.addQuadCurve(to: cur, control: c1)
            case "A":
                let rx = num(), ry = num(), rot = num(), large = num() != 0, sweep = num() != 0
                let end = CGPoint(x: o.x + num(), y: o.y + num())
                arc(path, from: cur, rx: rx, ry: ry, degrees: rot, large: large, sweep: sweep, to: end); cur = end
            case "Z": path.closeSubpath(); cur = start; if isNum() { cmd = "L" } else { continue }
            default: i += 1
            }
            if !isNum() { continue }
        }
        return path
    }

    /// The SVG endpoint arc as cubic segments of at most 90 degrees.
    static func arc(_ p: CGMutablePath, from p0: CGPoint, rx rx0: CGFloat, ry ry0: CGFloat, degrees: CGFloat,
                    large: Bool, sweep: Bool, to p1: CGPoint) {
        if p0 == p1 { return }
        var rx = abs(rx0), ry = abs(ry0)
        if rx == 0 || ry == 0 { p.addLine(to: p1); return }
        let phi = degrees * .pi / 180, cs = cos(phi), sn = sin(phi)
        let dx = (p0.x - p1.x) / 2, dy = (p0.y - p1.y) / 2
        let x1 = cs * dx + sn * dy, y1 = -sn * dx + cs * dy
        let lam = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry)
        if lam > 1 { rx *= lam.squareRoot(); ry *= lam.squareRoot() }
        let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1
        let den = rx * rx * y1 * y1 + ry * ry * x1 * x1
        var co = max(0, num / den).squareRoot()
        if large == sweep { co = -co }
        let cxp = co * rx * y1 / ry, cyp = -co * ry * x1 / rx
        let cx = cs * cxp - sn * cyp + (p0.x + p1.x) / 2, cy = sn * cxp + cs * cyp + (p0.y + p1.y) / 2
        func ang(_ ux: CGFloat, _ uy: CGFloat, _ vx: CGFloat, _ vy: CGFloat) -> CGFloat { atan2(ux * vy - uy * vx, ux * vx + uy * vy) }
        var a = ang(1, 0, (x1 - cxp) / rx, (y1 - cyp) / ry)
        var dt = ang((x1 - cxp) / rx, (y1 - cyp) / ry, (-x1 - cxp) / rx, (-y1 - cyp) / ry)
        if !sweep && dt > 0 { dt -= 2 * .pi } else if sweep && dt < 0 { dt += 2 * .pi }
        let n = max(1, Int((abs(dt) / (.pi / 2)).rounded(.up)))
        let seg = dt / CGFloat(n), k = 4 / 3 * tan(seg / 4)
        func pt(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: cx + rx * x * cs - ry * y * sn, y: cy + rx * x * sn + ry * y * cs) }
        for _ in 0..<n {
            let b = a + seg
            p.addCurve(to: pt(cos(b), sin(b)), control1: pt(cos(a) - k * sin(a), sin(a) + k * cos(a)),
                       control2: pt(cos(b) + k * sin(b), sin(b) - k * cos(b)))
            a = b
        }
    }
}
