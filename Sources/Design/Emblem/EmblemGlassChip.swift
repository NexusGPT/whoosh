import SwiftUI

extension EmblemRGB {
    /// An opaque token colour as emblem maths input (opacity dropped).
    init(_ c: RGBA) { self.init(c.red, c.green, c.blue) }
}

/// Board round 4: the glass chip. The disc is painted with the tokens of the design's glass fallback
/// (WhooshGlassModifier): WhooshGlass.tint, then the quiet chip fill WhooshPalette.fill (so a white row still shows the
/// shade), the WhooshGlass.rimHighlight top edge, the WhooshGlass.rim inner hairline and the WhooshGlass.hairline outer
/// edge. One shade for every account; light and dark from the theme. The drop shadow is left out: at 16 pt its 5 pt
/// blur would halo the row. A Canvas cannot host the system Liquid Glass, and sixteen live glass views per list
/// would cost frames, so the chip draws the fallback's tokens on every OS.
extension EmblemDisc {
    func drawGlass(_ ctx: inout GraphicsContext, _ d: CGFloat) {
        let s: ColorScheme = recipe.dark ? .dark : .light
        let disc = Path(ellipseIn: CGRect(x: 0, y: 0, width: d, height: d))
        ctx.fill(disc, with: .color(WhooshGlass.tint.color(s)))
        ctx.fill(disc, with: .color(WhooshPalette.fill.color(s)))
        let hi = WhooshGlass.rimHighlightOffset
        ctx.drawLayer { l in
            l.clip(to: disc)
            l.stroke(Path(ellipseIn: CGRect(x: hi / 2, y: hi / 2 + hi, width: d - hi, height: d - hi)),
                     with: .color(WhooshGlass.rimHighlight.color(s)), lineWidth: hi)
        }
        let w = WhooshGlass.rimWidth
        let edge = Path(ellipseIn: CGRect(x: w / 2, y: w / 2, width: d - w, height: d - w))
        ctx.stroke(edge, with: .color(WhooshGlass.rim.color(s)), lineWidth: w)
        ctx.stroke(edge, with: .color(WhooshGlass.hairline.color(s)), lineWidth: w)
    }

    /// G3: the account colour as one small dot at the disc's lower right (the status dot, when shown, covers it).
    func drawAccountDot(_ ctx: inout GraphicsContext, _ d: CGFloat) {
        let r = EmblemTreatment.glass3DotDiameter / 2, c = d * EmblemTreatment.glass3DotCentre
        ctx.fill(Path(ellipseIn: CGRect(x: c - r, y: c - r, width: r * 2, height: r * 2)), with: .color(recipe.colour.color))
    }
}
