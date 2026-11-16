import SwiftUI

/// The first-start flow's look and geometry, every number from the three picked boards (read whole, 2026-11-10):
/// 1a Staged inbox (docs/design/boards/first-start-r1/fs-staged-inbox), 2b Aurora stage (first-start-r2/fs-look-aurora-stage,
/// its keys step also 2a first-start-r2/fs-look-living-panes) and 3a Icon hero (first-start-r3/wc-icon-hero). The boards live
/// outside the repo; the values below are copied, never recomputed. Response animations are capped by MOTION_MAX_MS (200); the
/// boards' 240/260 ms card morph is 200 ms here; a loop is content and gates nothing.
enum FirstStartLook {
    // MARK: colours (2b `.s-light` / `.s-dark`)
    /// `--veil`
    static let veil = ThemeColor(light: RGBA(244, 244, 247, 0.82), dark: RGBA(12, 12, 14, 0.76))
    /// `--stage`
    static let stage = ThemeColor(light: RGBA(250, 250, 252, 1), dark: RGBA(7, 8, 12, 1))
    /// `--gtint` the coach card's glass tint (2b `.g2b`)
    static let cardTint = ThemeColor(light: RGBA(255, 255, 255, 0.74), dark: RGBA(56, 56, 60, 0.74))
    /// `--g2` the lighter glass of the sample cards on the stage (2b `.gl`)
    static let sampleTint = ThemeColor(light: RGBA(255, 255, 255, 0.66), dark: RGBA(36, 36, 42, 0.58))
    /// the aurora's four colours: the send wave's own (SendWaveScheme.aurora), through its tokens, never retyped
    static var aurora: [ThemeColor] { Array(SendWaveScheme.aurora.rings.prefix(4)) }
    /// 2b `--glow` and `--dotr`
    static let glow = (light: 0.22, dark: 0.28)
    static let dotRadius = (light: 1.3, dark: 1.15)

    // MARK: time
    /// every loop of steps 1 to 5 (2b "the loops"; 3a "ambient loop 8 s")
    static let loopS: Double = 8
    /// the frame a frozen loop (Reduced Motion, Low Power) shows, per step: the boards' `keyTime`
    static func keyTime(_ step: Int) -> Double { [1: 2.0, 2: 3.2, 3: 2.86, 4: 3.4, 5: 1.62][step] ?? 2.0 }
    /// the card morph and the veil: MOTION_MAX_MS (the boards' 240/260 ms become the ledger's cap)
    static var morphS: Double { Double(WhooshConstants.MOTION_MAX_MS) / 1000 }
    static var veilS: Double { Double(WhooshConstants.MOTION_MAX_MS) / 1000 }
    /// the ring that crosses the window at step 10: LAND_AURORA_MS (decorative, gates nothing; 2b "the ring follows the precedent")
    static var ringS: Double { Double(WhooshConstants.LAND_AURORA_MS) / 1000 }
    /// key press 80 ms, focus slide 80 ms (2a/2b motion lines)
    static var keyPressS: Double { Double(WhooshConstants.FOCUS_SLIDE_MS) / 1000 }
    static var leaveS: Double { Double(WhooshConstants.ROW_COLLAPSE_MS) / 1000 }
    static var closeUpS: Double { Double(WhooshConstants.ROW_INSERT_MS) / 1000 }
    static var popoverS: Double { Double(WhooshConstants.POPOVER_MS) / 1000 }
    /// the welcome card's entrance (3a): veil 200, card rise 12 pt from 0.98 in 200 ms (the cap), the medallion's own settle 360 ms
    static let medallionSettleS: Double = 0.36
    static let welcomeEntranceMs = 550
    static let medallionDelayS: Double = 0.09
    static let haloS: Double = 0.42
    static let haloDelayS: Double = 0.13

    /// Low Power freezes the loops on their key frame, like Reduced Motion.
    static var lowPower: Bool { ProcessInfo.processInfo.isLowPowerModeEnabled }

    // MARK: ease
    /// the boards' `cubic-bezier(.2,.8,.2,1)` shape without a solver: ease-out cubic of a clamped 0...1 run between two moments
    static func run(_ t: Double, _ a: Double, _ b: Double) -> Double {
        guard b > a else { return t >= b ? 1 : 0 }
        let x = min(max((t - a) / (b - a), 0), 1)
        return 1 - pow(1 - x, 3)
    }
    /// the boards' `OW`: 1 inside [a, b], with an 80 ms linear edge at each end (no edge at 0 or at the loop's end)
    static func win(_ t: Double, _ a: Double, _ b: Double, _ f: Double = 0.08) -> Double {
        let up = a <= 0 ? 1 : min(max((t - a) / f, 0), 1)
        let down = b >= loopS ? 1 : 1 - min(max((t - b) / f, 0), 1)
        return up * down
    }
    /// several windows at once: the largest
    static func wins(_ t: Double, _ list: [(Double, Double)], _ f: Double = 0.08) -> Double {
        list.map { win(t, $0.0, $0.1, f) }.max() ?? 0
    }
    /// a keyframed value: `points` are (time, value), each leg eased out; before the first and after the last it holds
    static func track(_ t: Double, _ points: [(Double, Double)]) -> Double {
        guard let first = points.first, let last = points.last else { return 0 }
        if t <= first.0 { return first.1 }
        if t >= last.0 { return last.1 }
        for i in 1..<points.count where t <= points[i].0 {
            let a = points[i - 1], b = points[i]
            return a.1 + (b.1 - a.1) * run(t, a.0, b.0)
        }
        return last.1
    }
    /// a value that rises over [a, a+d], holds, then falls over [b, b+d]
    static func window(_ t: Double, _ a: Double, _ b: Double, _ d: Double = 0.14) -> Double {
        run(t, a, a + d) * (1 - run(t, b, b + d))
    }
}

/// Where each coach card and each lit pane sits, from the 1a layout board and the 2b look board (1280 x 820 and 960 x 632).
struct FirstStartGeometry: Equatable {
    let size: CGSize
    static let toolbar: CGFloat = 32

    var compact: Bool { size.width < MacShellGeometry.compactBelowW }
    var sidebarW: CGFloat { compact ? MacShellGeometry.sidebarCompactW : MacShellGeometry.sidebarW }
    var listW: CGFloat { compact ? MacShellGeometry.listCompactW : MacShellGeometry.listW }
    var readerX: CGFloat { sidebarW + listW }
    var readerW: CGFloat { size.width - readerX }
    /// the list pane's stage starts below the real search field and header (2b `T = 118`)
    static let stageTop: CGFloat = 118

    /// the lit panes by name (2b `veils()`)
    func pane(_ name: String) -> CGRect {
        switch name {
        case "toolbar": return CGRect(x: 0, y: 0, width: size.width, height: Self.toolbar)
        case "sidebar": return CGRect(x: 0, y: Self.toolbar, width: sidebarW, height: size.height - Self.toolbar)
        case "list": return CGRect(x: sidebarW, y: Self.toolbar, width: listW, height: size.height - Self.toolbar)
        default: return CGRect(x: readerX, y: Self.toolbar, width: readerW, height: size.height - Self.toolbar)
        }
    }
    static let paneNames = ["toolbar", "sidebar", "list", "reader"]
    /// 2b `lit`: which panes stay at full contrast on each step (8 and the skip state lift everything)
    static func lit(step: Int, lifted: Bool) -> Set<String> {
        if lifted { return Set(paneNames) }
        switch step {
        case 2, 3: return ["list"]
        case 4: return ["sidebar", "list"]
        case 5: return ["reader"]
        case FirstStartModel.aiStep, FirstStartModel.repliesStep: return ["list"]
        default: return []
        }
    }

    /// the coach card of a step, its top-left corner and size (2b `card`); steps 6 and 8 hug their content below `top`.
    func card(step: Int) -> CGRect {
        let cw = min(360, readerW - 40)
        switch step {
        case 1: return welcomeCard
        case 2: return CGRect(x: readerX + 20, y: 82, width: cw, height: 232)
        case 3: return CGRect(x: readerX + 20, y: 118, width: cw, height: 214)
        case 4: return CGRect(x: readerX + 20, y: 118, width: cw, height: 300)
        case FirstStartModel.aiStep:   // design Q1: the same wide card as Réponses, a little taller for its three lines
            return CGRect(x: readerX + 20, y: 104, width: min(560, readerW - 40), height: 412)
        case FirstStartModel.repliesStep:   // the Réponses board's card (560 wide at 1280) beside the list's stage
            return CGRect(x: readerX + 20, y: 104, width: min(560, readerW - 40), height: 372)
        case 5:
            let h: CGFloat = size.height >= 800 ? 318 : 330
            return CGRect(x: readerX + 16, y: size.height - h - 16, width: readerW - 32, height: h)
        case 6:
            let w: CGFloat = compact ? 520 : 560
            return CGRect(x: (size.width - w) / 2, y: (size.height - 336) / 2 - 18, width: w, height: 336)
        default:
            let w: CGFloat = size.width >= 1280 ? 640 : 600
            let h: CGFloat = size.height >= 800 ? 392 : 380
            return CGRect(x: (size.width - w) / 2, y: (size.height - h) / 2 - 18, width: w, height: h)
        }
    }

    // MARK: 3a Icon hero
    var medallion: CGFloat { size.width >= 1280 ? 120 : 108 }
    /// the welcome card: 420 x 280 at 1280, 392 x 272 at 960, its top edge crowned by the medallion
    var welcomeCard: CGRect {
        let big = size.width >= 1280
        let w: CGFloat = big ? 420 : 392, h: CGFloat = big ? 280 : 272
        let comp = medallion / 2 + h
        let iconY = ((size.height - comp) / 2 - 8).rounded()
        return CGRect(x: (size.width - w) / 2, y: iconY + medallion / 2, width: w, height: h)
    }
    var medallionOrigin: CGPoint { CGPoint(x: (size.width - medallion) / 2, y: welcomeCard.minY - medallion / 2) }
    var haloSize: CGFloat { (medallion * 3.7).rounded() }
}
