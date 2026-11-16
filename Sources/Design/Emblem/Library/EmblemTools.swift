import CoreGraphics
// Tools family. gear: board glyph, verbatim (eight teeth at 56 pt, six at the simplified size).
enum EmblemTools {
    static let glyphs: [EmblemGlyph] = [
        EmblemGlyph("gear", .tools,
            detail: EmblemDrawing([G.c(12, 12, 7.1)] + stride(from: 0, to: 360, by: 45).map { G.f("M10.3 2.4 H13.7 L14.3 6.2 H9.7 Z", rot: CGFloat($0)) },
                                  [G.ring(12, 12, 4.3, 2.2)], cut: [G.c(12, 12, 2.6)]),
            small: EmblemDrawing([G.c(12, 12, 7.4)] + stride(from: 0, to: 360, by: 60).map { G.f("M10 2 H14 L14.6 6.2 H9.4 Z", rot: CGFloat($0)) },
                                 [G.ring(12, 12, 4.9, 2.2)], cut: [G.c(12, 12, 3.1)])),
        EmblemGlyph("wrench", .tools,
            detail: EmblemDrawing([G.c(12, 6.4, 4.8), G.r(10.4, 8, 3.2, 13.4, 1.6, rot: 45)].map(rot45), [G.r(10.4, 15, 3.2, 6.4, 1.6, rot: 45)],
                                  cut: [G.r(10.6, 0.6, 2.8, 6, 0.6, rot: 45)]),
            small: EmblemDrawing([G.c(12, 6.2, 5.4), G.r(10.2, 8, 3.6, 14, 1.8, rot: 45)].map(rot45), [G.r(10.2, 15.2, 3.6, 6.8, 1.8, rot: 45)],
                                 cut: [G.r(10.5, 0, 3, 6.4, 0.6, rot: 45)])),
        EmblemGlyph("hammer", .tools,
            detail: EmblemDrawing([G.r(4, 3.6, 12.4, 5.6, 1.6, rot: -30)], [G.r(10.4, 9, 3.2, 12.6, 1.6, rot: -30)], cut: [G.s("M6.6 6.4 H9", 1.2, rot: -30)]),
            small: EmblemDrawing([G.r(3.4, 3, 13.6, 6.4, 1.8, rot: -30)], [G.r(10.2, 9.2, 3.6, 13, 1.8, rot: -30)])),
        EmblemGlyph("pencil", .tools,
            detail: EmblemDrawing([G.r(9.2, 5, 5.6, 13, 1, rot: 45), G.f("M9.2 18 H14.8 L12 22.4 Z", rot: 45)], [G.r(9.2, 1.6, 5.6, 3.6, 1.4, rot: 45)],
                                  cut: [G.s("M12 7 V15.6", 1.1, rot: 45)]),
            small: EmblemDrawing([G.r(8.8, 4.8, 6.4, 13.4, 1.2, rot: 45), G.f("M8.8 18.2 H15.2 L12 23 Z", rot: 45)], [G.r(8.8, 1, 6.4, 3.8, 1.6, rot: 45)],
                                 cut: [G.s("M12 7 V15.6", 1.8, rot: 45)])),
        EmblemGlyph("ruler", .tools,
            detail: EmblemDrawing([G.r(2.4, 8.6, 19.2, 6.8, 1.4, rot: -45)], [G.r(18, 8.6, 3.6, 6.8, 1.4, rot: -45)],
                                  cut: [G.s("M6 8.6 V11.4 M9.6 8.6 V12.4 M13.2 8.6 V11.4 M16.8 8.6 V12.4", 1.1, rot: -45)]),
            small: EmblemDrawing([G.r(1.6, 8.2, 20.8, 7.6, 1.6, rot: -45)], [G.r(18.4, 8.2, 4, 7.6, 1.6, rot: -45)],
                                 cut: [G.s("M7 8.2 V12.2 M12 8.2 V11.4 M17 8.2 V12.2", 1.8, rot: -45)])),
        EmblemGlyph("toolbox", .tools,
            detail: EmblemDrawing([G.r(2.4, 8.4, 19.2, 12.6, 2.4), G.s("M8.4 8.4 V5.6 A1.4 1.4 0 0 1 9.8 4.2 H14.2 A1.4 1.4 0 0 1 15.6 5.6 V8.4", 2)],
                                  [G.f("M2.4 10.8 A2.4 2.4 0 0 1 4.8 8.4 H19.2 A2.4 2.4 0 0 1 21.6 10.8 V13 H2.4 Z")],
                                  cut: [G.r(10.4, 12, 3.2, 3, 0.8)], cutsBoth: true),
            small: EmblemDrawing([G.r(1.8, 8.2, 20.4, 13.4, 2.6), G.s("M8 8.2 V5.2 A1.6 1.6 0 0 1 9.6 3.6 H14.4 A1.6 1.6 0 0 1 16 5.2 V8.2", 2.6)],
                                 [G.f("M1.8 10.8 A2.6 2.6 0 0 1 4.4 8.2 H19.6 A2.6 2.6 0 0 1 22.2 10.8 V13.4 H1.8 Z")],
                                 cut: [G.r(10, 12.2, 4, 3.6, 0.8)], cutsBoth: true)),
    ]
    /// The wrench head turns with its handle.
    private static func rot45(_ s: EmblemShape) -> EmblemShape { s.rotation != 0 ? s : EmblemShape(d: s.d, stroke: s.stroke, rotation: 45) }
}
