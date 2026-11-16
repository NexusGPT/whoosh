import CoreGraphics

/// Which drawing rules apply at a size (board "Construction": detailed, simplified, 16 pt).
public enum EmblemTier: String, Sendable { case detailed, simplified, tiny
    public init(diameter: CGFloat) {
        self = diameter >= EmblemConstruction.detailedFrom ? .detailed : (diameter <= EmblemConstruction.tinyUpTo ? .tiny : .simplified)
    }
}

/// Everything the emblem draws, as data: the view paints only this, and the render check reads it.
public struct EmblemRecipe: Equatable, Sendable {
    public let tier: EmblemTier
    public let diameter: CGFloat
    public let dark: Bool
    /// The account colour after the list-row saturation cut (EMBLEM_LIST_SATURATION) when `listRow`.
    public let colour: EmblemRGB
    public let baseTop: EmblemRGB, baseBottom: EmblemRGB
    public let ink: EmblemRGB, secondTone: EmblemRGB, darkInk: Bool
    /// Layer opacities; nil = the layer is not drawn at this tier.
    public let innerShadow: Double?, specular: Double?, glyphShadow: Bool, rimLight: CGFloat?
    public let hairlineWhite: Bool, hairlineOpacity: Double
    public let glyphBox: CGFloat, soften: CGFloat
    /// A Debug-only calmer treatment (EmblemTreatment) for the quiet sizes; nil draws the lit disc.
    let treatment: EmblemTreatment?

    public init(colourHex: String, diameter: CGFloat, dark: Bool, listRow: Bool = false) {
        let C = EmblemConstruction.self
        let raw = EmblemRGB(hex: colourHex) ?? EmblemRGB(0.56, 0.56, 0.58)
        // Owner's pick 2026-11-08 (emblem board): the G3 glass chip with a grey glyph and no colour dot is the look;
        // a Debug override (WHOOSH_EMBLEM_TREATMENT) still selects another treatment for a board.
        let tr = listRow ? (EmblemTreatment.current ?? .glass3) : nil
        let c = tr.map { raw.saturation(times: $0.saturation) } ?? (listRow ? raw.saturation(times: EmblemListSaturation.value) : raw)
        treatment = tr
        let t = EmblemTier(diameter: diameter)
        tier = t; self.diameter = diameter; self.dark = dark; colour = c
        let ink = EmblemRecipe.inkIsDark(c)
        darkInk = ink
        let g = t == .tiny ? C.baseTiny : (dark ? C.baseDark : C.baseLight)
        let darken = ink ? C.darkInkDarkening : 1
        baseTop = c.mixed(.white, g.white); baseBottom = c.mixed(.black, g.black * darken)
        let surface: EmblemRGB = dark ? EmblemRGB(0.11, 0.11, 0.12) : .white
        let glyphInk: EmblemRGB
        switch tr {
        case .faint?: glyphInk = dark ? raw.saturation(times: 0.45).mixed(.white, 0.45) : raw.mixed(.black, 0.45)  // dark mode: the hue's light shade
        case .ring?, .neutral?, .glass1?: glyphInk = c
        case .glass2?: glyphInk = dark ? c.mixed(.white, EmblemTreatment.glass2ShadeDark) : c.mixed(.black, EmblemTreatment.glass2ShadeLight)
        case .glass3?: glyphInk = EmblemRGB(dark ? WhooshPalette.secondaryLabel.dark : WhooshPalette.secondaryLabel.light)
        case nil: glyphInk = ink ? c.mixed(.black, C.darkInk) : .white
        }
        self.ink = glyphInk
        let calm = EmblemTreatment.secondToneMix
        secondTone = tr != nil ? glyphInk.mixed(surface, calm) : (ink ? c.mixed(.black, C.secondToneDark) : c.mixed(.white, C.secondToneWhite))
        innerShadow = tr == nil && t == .detailed ? (dark ? C.innerShadowDark : C.innerShadowLight) : nil
        specular = tr == nil && t == .detailed ? (dark ? C.specularDark : C.specularLight) : nil
        glyphShadow = tr == nil && t == .detailed
        rimLight = tr != nil ? nil : (t == .detailed ? C.rimLightDetailed * diameter : (t == .simplified ? C.rimLightSmall : nil))
        hairlineWhite = dark; hairlineOpacity = dark ? C.hairlineDark : C.hairlineLight
        glyphBox = diameter * CGFloat(t == .detailed ? C.boxDetailed : (t == .simplified ? C.boxSimplified : C.boxTiny))
        soften = t == .detailed ? C.softenDetailed : C.softenSmall
    }

    /// Ink rule: white when white keeps 3:1 over the glyph span of the base in both schemes, else the dark ink.
    public static func inkIsDark(_ c: EmblemRGB) -> Bool {
        let C = EmblemConstruction.self
        for g in [C.baseLight, C.baseDark] {
            let top = c.mixed(.white, g.white), bottom = c.mixed(.black, g.black)
            for s in [C.glyphSpan.0, C.glyphSpan.1] where EmblemRGB.white.contrast(top.mixed(bottom, s)) < C.inkMin { return true }
        }
        return false
    }

    /// The base colour at a share of the disc height.
    public func base(at s: Double) -> EmblemRGB { baseTop.mixed(baseBottom, s) }
    /// Mean of the base over the disc: what a row reads as the mark's colour.
    public var baseMid: EmblemRGB { base(at: 0.5) }
    public var shadowColour: EmblemRGB { colour.mixed(.black, EmblemConstruction.shadowBlack) }
}
