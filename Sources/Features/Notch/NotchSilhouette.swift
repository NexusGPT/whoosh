import SwiftUI

/// The notch's one shape: it hangs from the top edge, flares out into the screen's top edge with two concave
/// shoulders, and rounds its bottom corners. At rest it is exactly the camera housing (no shoulders, the
/// housing's own corner); growing animates the frame, the shoulders and the corners together.
/// The look is the one the owner shipped in cue-notch (docs/screenshots/rest-sizes.png, hero.png); the path
/// is written here from that picture, because cue-notch's own path sits in a file that cites GPL code.
struct NotchSilhouette: Shape {
    var shoulder: CGFloat
    var corner: CGFloat

    var animatableData: AnimatablePair<CGFloat, CGFloat> {
        get { AnimatablePair(shoulder, corner) }
        set { shoulder = newValue.first; corner = newValue.second }
    }

    func path(in r: CGRect) -> Path {
        let s = min(shoulder, r.width / 2)
        let c = min(corner, (r.width - 2 * s) / 2, r.height / 2)
        var p = Path()
        p.move(to: CGPoint(x: r.minX, y: r.minY))
        p.addQuadCurve(to: CGPoint(x: r.minX + s, y: r.minY + s), control: CGPoint(x: r.minX + s, y: r.minY))
        p.addLine(to: CGPoint(x: r.minX + s, y: r.maxY - c))
        p.addQuadCurve(to: CGPoint(x: r.minX + s + c, y: r.maxY), control: CGPoint(x: r.minX + s, y: r.maxY))
        p.addLine(to: CGPoint(x: r.maxX - s - c, y: r.maxY))
        p.addQuadCurve(to: CGPoint(x: r.maxX - s, y: r.maxY - c), control: CGPoint(x: r.maxX - s, y: r.maxY))
        p.addLine(to: CGPoint(x: r.maxX - s, y: r.minY + s))
        p.addQuadCurve(to: CGPoint(x: r.maxX, y: r.minY), control: CGPoint(x: r.maxX - s, y: r.minY))
        p.closeSubpath()
        return p
    }
}

/// Section label, from cue-notch, by the owner (Notch/DesignSystem.swift `Label`): small caps, tracked, quiet.
/// Re-expressed in Whoosh tokens (the caption size, the tertiary tone).
struct NotchSectionLabel: View {
    let text: String
    var body: some View {
        Text(text.uppercased())
            .font(WhooshTypography.caption.font(emphasized: true))
            .tracking(1.2)
            .foregroundStyle(WhooshPalette.tertiaryLabel)
    }
}
