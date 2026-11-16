import CoreGraphics
// Places family. mountain: board glyph, verbatim (both peaks take the ridge cut-out).
enum EmblemPlaces {
    static let glyphs: [EmblemGlyph] = [
        EmblemGlyph("mountain", .places,
            detail: EmblemDrawing([G.f("M10.5 20.5 L16 9.8 L22.6 20.5 Z")], [G.f("M1.4 20.5 L9.4 6.2 L17.4 20.5 Z")],
                                  cut: [G.s("M5.6 13 L7.6 14.6 L9.4 12.8 L11.2 14.6 L13.2 13", 1.3)], cutsBoth: true),
            small: EmblemDrawing([G.f("M10.4 21 L16.2 9.6 L23 21 Z")], [G.f("M1 21 L9.4 5.6 L17.8 21 Z")],
                                 cut: [G.s("M5.4 13.4 L7.8 15.2 L9.4 13.4 L11 15.2 L13.4 13.4", 1.9)], cutsBoth: true)),
        EmblemGlyph("pin", .places,
            detail: EmblemDrawing([G.f("M12 22 C12 22 4.4 14.6 4.4 9.6 A7.6 7.6 0 0 1 19.6 9.6 C19.6 14.6 12 22 12 22 Z")],
                                  [G.f("M12 2 A7.6 7.6 0 0 1 19.6 9.6 C19.6 14.6 12 22 12 22 Z")], cut: [G.c(12, 9.6, 2.8)], cutsBoth: true),
            small: EmblemDrawing([G.f("M12 22.8 C12 22.8 3.6 15 3.6 9.6 A8.4 8.4 0 0 1 20.4 9.6 C20.4 15 12 22.8 12 22.8 Z")],
                                 [G.f("M12 1.2 A8.4 8.4 0 0 1 20.4 9.6 C20.4 15 12 22.8 12 22.8 Z")], cut: [G.c(12, 9.6, 3.3)], cutsBoth: true)),
        EmblemGlyph("flag", .places,
            detail: EmblemDrawing([G.r(4, 2.6, 2.2, 19, 1.1)], [G.f("M6.2 3.6 H19.6 L16.6 8.4 L19.6 13.2 H6.2 Z")]),
            small: EmblemDrawing([G.r(3.4, 2, 2.8, 20.2, 1.4)], [G.f("M6.2 3 H20.8 L17.4 8.4 L20.8 13.8 H6.2 Z")])),
        EmblemGlyph("tent", .places,
            detail: EmblemDrawing([G.f("M12 3.4 L22 20.6 H2 Z")], [G.f("M12 10.4 L16.2 20.6 H7.8 Z")], cut: [G.s("M12 4.6 V9", 1.2)]),
            small: EmblemDrawing([G.f("M12 2.4 L22.8 21.2 H1.2 Z")], [G.f("M12 10 L16.8 21.2 H7.2 Z")])),
        EmblemGlyph("lighthouse", .places,
            detail: EmblemDrawing([G.f("M9 8 H15 L16.6 20.4 H7.4 Z"), G.r(9.2, 3.6, 5.6, 4.4, 1), G.r(5, 19.4, 14, 2.6, 1.2)],
                                  [G.f("M8.4 12.2 H15.6 L15.9 14.6 H8.1 Z")], cut: [G.r(11, 4.6, 2, 2.4, 0.5)]),
            small: EmblemDrawing([G.f("M8.8 8 H15.2 L17 20.4 H7 Z"), G.r(8.8, 2.6, 6.4, 5, 1.2), G.r(4.4, 19.2, 15.2, 3.2, 1.4)],
                                 [G.f("M8.2 12.2 H15.8 L16.2 15.2 H7.8 Z")])),
        EmblemGlyph("signpost", .places,
            detail: EmblemDrawing([G.r(10.8, 2.6, 2.4, 19, 1)], [G.f("M4 5 H17 L19.6 7.6 L17 10.2 H4 Z"), G.f("M20 12 H7 L4.4 14.6 L7 17.2 H20 Z")]),
            small: EmblemDrawing([G.r(10.6, 2, 2.8, 20.2, 1.2)], [G.f("M3.4 4.4 H17.4 L20.4 7.6 L17.4 10.8 H3.4 Z"), G.f("M20.6 12 H6.6 L3.6 15.2 L6.6 18.4 H20.6 Z")])),
    ]
}
