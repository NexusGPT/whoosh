import CoreGraphics
// Money family. coin and diamond: board glyphs, verbatim.
enum EmblemMoney {
    static let glyphs: [EmblemGlyph] = [
        EmblemGlyph("coin", .money,
            detail: EmblemDrawing([G.c(12, 12, 9.2)], [G.ring(12, 12, 7.6, 1.1)],
                                  cut: [G.s("M15.4 8.2 A4.8 4.8 0 1 0 15.4 15.8", 2), G.s("M7.2 10.9 H12.6 M7.2 13.1 H12.6", 1.25)]),
            small: EmblemDrawing([G.c(12, 12, 9.6)], cut: [G.s("M15.8 8 A5.1 5.1 0 1 0 15.8 16", 2.6), G.s("M6.8 12 H12.4", 1.8)])),
        EmblemGlyph("diamond", .money,
            detail: EmblemDrawing([G.f("M6.6 3.8 H17.4 L22 9.6 L12 21.2 L2 9.6 Z")], [G.f("M6.6 3.8 H17.4 L22 9.6 H2 Z")],
                                  cut: [G.s("M8.6 9.6 L12 4.4 L15.4 9.6", 1), G.s("M2 9.8 H22", 1.2)]),
            small: EmblemDrawing([G.f("M6.2 3.2 H17.8 L22.8 9.6 L12 22 L1.2 9.6 Z")], [G.f("M6.2 3.2 H17.8 L22.8 9.6 H1.2 Z")],
                                 cut: [G.s("M1.2 9.9 H22.8", 1.8)])),
        EmblemGlyph("bank", .money,
            detail: EmblemDrawing([G.f("M12 2.4 L21.8 8 H2.2 Z"), G.r(4.4, 9.6, 2.6, 8.2, 0.6), G.r(8.6, 9.6, 2.6, 8.2, 0.6),
                                   G.r(12.8, 9.6, 2.6, 8.2, 0.6), G.r(17, 9.6, 2.6, 8.2, 0.6)],
                                  [G.r(2.2, 18.6, 19.6, 3, 1)], cut: [G.c(12, 5.8, 1.1)]),
            small: EmblemDrawing([G.f("M12 1.8 L22.6 8.2 H1.4 Z"), G.r(4.2, 9.8, 3.2, 8, 0.6), G.r(10.4, 9.8, 3.2, 8, 0.6), G.r(16.6, 9.8, 3.2, 8, 0.6)],
                                 [G.r(1.6, 18.6, 20.8, 3.4, 1.2)])),
        EmblemGlyph("wallet", .money,
            detail: EmblemDrawing([G.r(4.6, 3.4, 13, 4.2, 1.2), G.r(2.6, 6, 18.8, 14.4, 2.6)], [G.r(13.4, 10.2, 8, 6, 2)],
                                  cut: [G.c(16.4, 13.2, 1.3)], cutsBoth: true),
            small: EmblemDrawing([G.r(2, 5.6, 20, 15.4, 2.8)], [G.r(13.2, 9.8, 8.8, 6.8, 2.2)], cut: [G.c(16.6, 13.2, 1.8)], cutsBoth: true)),
        EmblemGlyph("card", .money,
            detail: EmblemDrawing([G.r(2.2, 5, 19.6, 14, 2.6)], [G.f("M2.2 8.4 H21.8 V11 H2.2 Z")], cut: [G.s("M5.6 15.4 H10", 1.4)]),
            small: EmblemDrawing([G.r(1.6, 4.4, 20.8, 15.2, 2.8)], [G.f("M1.6 8.2 H22.4 V11.4 H1.6 Z")], cut: [G.s("M5.4 15.6 H9.8", 2.2)])),
        EmblemGlyph("price-tag", .money,
            detail: EmblemDrawing([G.f("M3 4.6 A1.6 1.6 0 0 1 4.6 3 H11.6 L21.2 12.6 A1.6 1.6 0 0 1 21.2 14.8 L14.8 21.2 A1.6 1.6 0 0 1 12.6 21.2 L3 11.6 Z")],
                                  [G.f("M9.6 13.4 L13.4 9.6 L16 12.2 L12.2 16 Z")], cut: [G.c(7.6, 7.6, 1.8)]),
            small: EmblemDrawing([G.f("M2.4 4.2 A1.8 1.8 0 0 1 4.2 2.4 H11.8 L21.8 12.4 A1.8 1.8 0 0 1 21.8 14.8 L14.8 21.8 A1.8 1.8 0 0 1 12.4 21.8 L2.4 11.8 Z")],
                                 [G.f("M9.2 13.6 L13.6 9.2 L16.6 12.2 L12.2 16.6 Z")], cut: [G.c(7.4, 7.4, 2.3)])),
    ]
}
