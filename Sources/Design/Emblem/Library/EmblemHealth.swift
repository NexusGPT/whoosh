import CoreGraphics
// Health family. lifebuoy: board glyph, verbatim.
enum EmblemHealth {
    static let glyphs: [EmblemGlyph] = [
        EmblemGlyph("lifebuoy", .health,
            detail: EmblemDrawing([G.c(12, 12, 9.2)], [G.ring(12, 12, 6.65, 5.1, dash: 5.22)], cut: [G.c(12, 12, 4.1)]),
            small: EmblemDrawing([G.c(12, 12, 9.8)], [G.ring(12, 12, 7.05, 5.5, dash: 5.54)], cut: [G.c(12, 12, 4.3)])),
        EmblemGlyph("medical", .health,
            detail: EmblemDrawing([G.c(12, 12, 9.4)], [G.f("M10 5.4 H14 V10 H18.6 V14 H14 V18.6 H10 V14 H5.4 V10 H10 Z")]),
            small: EmblemDrawing([G.c(12, 12, 10)], [G.f("M9.8 4.6 H14.2 V9.8 H19.4 V14.2 H14.2 V19.4 H9.8 V14.2 H4.6 V9.8 H9.8 Z")])),
        EmblemGlyph("pill", .health,
            detail: EmblemDrawing([G.r(3, 8, 18, 8, 4, rot: -45)], [G.f("M12 8 H17 A4 4 0 0 1 17 16 H12 Z", rot: -45)], cut: [G.s("M6.4 10.6 H9", 1.2, rot: -45)]),
            small: EmblemDrawing([G.r(2.2, 7.4, 19.6, 9.2, 4.6, rot: -45)], [G.f("M12 7.4 H17.4 A4.6 4.6 0 0 1 17.4 16.6 H12 Z", rot: -45)])),
        EmblemGlyph("dumbbell", .health,
            detail: EmblemDrawing([G.r(5, 10.8, 14, 2.4, 1, rot: -30), G.r(5.2, 8, 2.6, 8, 1, rot: -30), G.r(16.2, 8, 2.6, 8, 1, rot: -30)],
                                  [G.r(2.2, 6.6, 3.4, 10.8, 1.4, rot: -30), G.r(18.4, 6.6, 3.4, 10.8, 1.4, rot: -30)]),
            small: EmblemDrawing([G.r(5, 10.6, 14, 2.8, 1.2, rot: -30), G.r(5.4, 7.4, 3.2, 9.2, 1.2, rot: -30), G.r(15.4, 7.4, 3.2, 9.2, 1.2, rot: -30)],
                                 [G.r(1.6, 6, 3.8, 12, 1.6, rot: -30), G.r(18.6, 6, 3.8, 12, 1.6, rot: -30)])),
        EmblemGlyph("pulse", .health,
            detail: EmblemDrawing([G.r(2.6, 2.6, 18.8, 18.8, 5)], [G.f("M2.6 12 H21.4 V16.4 A5 5 0 0 1 16.4 21.4 H7.6 A5 5 0 0 1 2.6 16.4 Z")],
                                  cut: [G.s("M5 12.4 H8.4 L10.2 8 L13.2 17 L15.2 11.6 L16.2 12.4 H19", 1.5)], cutsBoth: true),
            small: EmblemDrawing([G.r(2, 2, 20, 20, 5.4)], [G.f("M2 12 H22 V16.6 A5.4 5.4 0 0 1 16.6 22 H7.4 A5.4 5.4 0 0 1 2 16.6 Z")],
                                 cut: [G.s("M4.6 12.4 H8 L10 7.4 L13.4 17.6 L15.6 12.4 H19.4", 2.2)], cutsBoth: true)),
        EmblemGlyph("drop", .health,
            detail: EmblemDrawing([G.f("M12 2.4 C12 2.4 19.2 10.2 19.2 14.6 A7.2 7.2 0 0 1 4.8 14.6 C4.8 10.2 12 2.4 12 2.4 Z")],
                                  [G.f("M12 2.4 C12 2.4 19.2 10.2 19.2 14.6 A7.2 7.2 0 0 1 12 21.8 Z")], cut: [G.s("M8.2 15 A3.8 3.8 0 0 0 10.6 18.2", 1.4)]),
            small: EmblemDrawing([G.f("M12 1.6 C12 1.6 20 10 20 14.8 A8 8 0 0 1 4 14.8 C4 10 12 1.6 12 1.6 Z")],
                                 [G.f("M12 1.6 C12 1.6 20 10 20 14.8 A8 8 0 0 1 12 22.8 Z")], cut: [G.s("M7.6 15.2 A4.4 4.4 0 0 0 10.4 19", 2)])),
    ]
}
