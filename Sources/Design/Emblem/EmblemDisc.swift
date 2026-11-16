import SwiftUI

extension EmblemRGB { var color: Color { Color(.sRGB, red: r, green: g, blue: b, opacity: 1) } }

/// The lit disc: base, inner shadow, specular, pictogram (or uploaded image), rim. Paints only its EmblemRecipe.
struct EmblemDisc: View {
    let recipe: EmblemRecipe
    let glyph: EmblemGlyph
    var image: Image?

    var body: some View {
        Canvas { ctx, size in
            let d = min(size.width, size.height)
            let disc = Path(ellipseIn: CGRect(x: 0, y: 0, width: d, height: d))
            if let t = recipe.treatment { drawTreatment(&ctx, t, d) } else {
                ctx.fill(disc, with: .linearGradient(Gradient(colors: [recipe.baseTop.color, recipe.baseBottom.color]),
                                                     startPoint: .zero, endPoint: CGPoint(x: 0, y: d)))
            }
            if let image { drawImage(&ctx, image, d) } else { drawGlyph(&ctx, d) }
            if recipe.treatment == .glass3 { drawAccountDot(&ctx, d) }
            if recipe.treatment == nil { drawLight(&ctx, disc, d) }
        }
        .frame(width: recipe.diameter, height: recipe.diameter)
    }

    /// The calmer treatments (Debug board only): a faint fill, a hue ring, a neutral chip, or the glass chip.
    private func drawTreatment(_ ctx: inout GraphicsContext, _ t: EmblemTreatment, _ d: CGFloat) {
        let disc = Path(ellipseIn: CGRect(x: 0, y: 0, width: d, height: d))
        switch t {
        case .faint: ctx.fill(disc, with: .color(recipe.colour.color.opacity(EmblemTreatment.faintFillOpacity)))
        case .ring:
            let w = EmblemTreatment.ringWidth
            ctx.stroke(Path(ellipseIn: CGRect(x: w / 2, y: w / 2, width: d - w, height: d - w)), with: .color(recipe.colour.color), lineWidth: w)
        case .neutral: ctx.fill(disc, with: .color((recipe.dark ? Color.white : Color.black).opacity(recipe.dark ? 0.12 : 0.07)))
        case .glass1, .glass2, .glass3: drawGlass(&ctx, d)
        }
    }

    private func drawImage(_ ctx: inout GraphicsContext, _ image: Image, _ d: CGFloat) {
        let C = EmblemConstruction.self
        let inset = d * (recipe.tier == .detailed ? C.imageInsetDetailed : (recipe.tier == .tiny ? C.imageInsetTiny : C.imageInsetSmall))
        let r = CGRect(x: inset, y: inset, width: d - inset * 2, height: d - inset * 2)
        ctx.drawLayer { l in l.clip(to: Path(ellipseIn: r)); l.draw(image, in: r) }
    }

    /// Layer 4: the glyph box centred, the ink tone with its cut-out, then the second tone; shadow at the detailed tier.
    private func drawGlyph(_ ctx: inout GraphicsContext, _ d: CGFloat) {
        let box = recipe.glyphBox, k = box / 24
        let drawing = glyph.drawing(recipe.tier)
        ctx.drawLayer { g in
            if recipe.glyphShadow {
                g.addFilter(.shadow(color: recipe.shadowColour.color.opacity(EmblemConstruction.shadowOpacity),
                                    radius: EmblemConstruction.shadowBlur * d, x: 0, y: EmblemConstruction.shadowDy * d))
            }
            g.translateBy(x: (d - box) / 2, y: (d - box) / 2 + d * EmblemConstruction.glyphDrop)
            g.scaleBy(x: k, y: k)
            g.drawLayer { p in
                paint(&p, drawing.primary, recipe.ink.color)
                cut(&p, drawing.cut)
            }
            g.drawLayer { s in
                paint(&s, drawing.secondary, recipe.secondTone.color)
                if drawing.cutsBoth { cut(&s, drawing.cut) }
            }
        }
    }

    private func paint(_ ctx: inout GraphicsContext, _ shapes: [EmblemShape], _ color: Color) {
        for s in shapes {
            let path = Path(s.path)
            if let w = s.stroke {
                let dash = s.dash.map { [$0, $0] } ?? []
                ctx.stroke(path, with: .color(color), style: StrokeStyle(lineWidth: w, lineCap: s.dash == nil ? .round : .butt,
                                                                       lineJoin: .round, dash: dash, dashPhase: (s.dash ?? 0) / 2))
            } else {
                ctx.fill(path, with: .color(color))
                ctx.stroke(path, with: .color(color), style: StrokeStyle(lineWidth: recipe.soften, lineJoin: .round))
            }
        }
    }

    private func cut(_ ctx: inout GraphicsContext, _ shapes: [EmblemShape]) {
        guard !shapes.isEmpty else { return }
        ctx.blendMode = .destinationOut
        paintCut(&ctx, shapes)
        ctx.blendMode = .normal
    }

    private func paintCut(_ ctx: inout GraphicsContext, _ shapes: [EmblemShape]) {
        for s in shapes {
            if let w = s.stroke {
                ctx.stroke(Path(s.path), with: .color(.black), style: StrokeStyle(lineWidth: w, lineCap: .round, lineJoin: .round))
            } else { ctx.fill(Path(s.path), with: .color(.black)) }
        }
    }

    /// Layers 2, 3 and 5: inner shadow and specular (detailed tier only), then the rim light and the hairline.
    private func drawLight(_ ctx: inout GraphicsContext, _ disc: Path, _ d: CGFloat) {
        let C = EmblemConstruction.self
        if let a = recipe.innerShadow {
            ctx.fill(disc, with: .radialGradient(Gradient(stops: [
                .init(color: .black.opacity(0), location: C.innerShadowClearTo), .init(color: .black.opacity(a), location: 1)]),
                center: CGPoint(x: d / 2, y: d * C.innerShadowCentreY), startRadius: 0, endRadius: d * C.innerShadowRadius))
        }
        if let a = recipe.specular {
            let w = d * C.specularWidth, h = d * C.specularHeight, cy = d * C.specularCentreY
            let e = CGRect(x: (d - w) / 2, y: cy - h / 2, width: w, height: h)
            ctx.drawLayer { l in
                l.clip(to: disc)
                l.fill(Path(ellipseIn: e), with: .linearGradient(Gradient(colors: [.white.opacity(a), .white.opacity(0)]),
                                                                 startPoint: CGPoint(x: 0, y: e.minY), endPoint: CGPoint(x: 0, y: e.maxY)))
            }
        }
        if let w = recipe.rimLight {
            let r = CGRect(x: w / 2, y: w / 2, width: d - w, height: d - w)
            ctx.stroke(Path(ellipseIn: r), with: .linearGradient(Gradient(stops: [
                .init(color: .white.opacity(C.rimLightOpacity), location: 0), .init(color: .white.opacity(0), location: C.rimFadeBy)]),
                startPoint: .zero, endPoint: CGPoint(x: 0, y: d)), lineWidth: w)
        }
        let h = C.hairline
        let hair = Path(ellipseIn: CGRect(x: h / 2, y: h / 2, width: d - h, height: d - h))
        ctx.stroke(hair, with: .color((recipe.hairlineWhite ? Color.white : .black).opacity(recipe.hairlineOpacity)), lineWidth: h)
    }
}
