import CoreGraphics
// Home family. house: board glyph, verbatim.
enum EmblemHome {
    static let glyphs: [EmblemGlyph] = [
        EmblemGlyph("house", .home,
            detail: EmblemDrawing([G.f("M12 3 L21.6 11.4 H19.2 V19.6 A1.9 1.9 0 0 1 17.3 21.5 H6.7 A1.9 1.9 0 0 1 4.8 19.6 V11.4 H2.4 Z")],
                                  [G.f("M12 3 L21.6 11.4 H2.4 Z")], cut: [G.r(10, 14.6, 4, 8, 1.3)]),
            small: EmblemDrawing([G.f("M12 2.6 L22 11.4 H19.6 V19.8 A1.9 1.9 0 0 1 17.7 21.7 H6.3 A1.9 1.9 0 0 1 4.4 19.8 V11.4 H2 Z")],
                                 [G.f("M12 2.6 L22 11.4 H2 Z")], cut: [G.r(9.6, 14.2, 4.8, 8.5, 1.4)])),
        EmblemGlyph("key", .home,
            detail: EmblemDrawing([G.c(7.5, 12, 5), G.r(11, 10.6, 11, 2.8, 1.2)], [G.r(17, 12.6, 2.4, 4, 0.8), G.r(13.4, 12.6, 2.2, 3, 0.8)],
                                  cut: [G.c(7.5, 12, 2)]),
            small: EmblemDrawing([G.c(7.4, 12, 5.6), G.r(11, 10.4, 11.6, 3.2, 1.4)], [G.r(17.4, 12.8, 3, 4.4, 1)], cut: [G.c(7.4, 12, 2.4)])),
        EmblemGlyph("lamp", .home,
            detail: EmblemDrawing([G.f("M7.4 3 H16.6 L20 12 H4 Z"), G.r(11, 12, 2, 6, 0.6)], [G.r(6.5, 18.4, 11, 3.2, 1.6)],
                                  cut: [G.s("M9.6 5.6 L8.6 9.2", 1.2)]),
            small: EmblemDrawing([G.f("M7 2.4 H17 L20.8 12.4 H3.2 Z"), G.r(10.7, 12, 2.6, 6.4, 0.8)], [G.r(5.6, 18.4, 12.8, 3.6, 1.8)],
                                 cut: [G.s("M9.4 5.4 L8.2 9.6", 1.8)])),
        EmblemGlyph("door", .home,
            detail: EmblemDrawing([G.r(5, 2.6, 14, 19, 2.2)], [G.r(8, 5.4, 8, 5.6, 1), G.r(8, 13.4, 8, 5.6, 1)], cut: [G.c(15.2, 12.4, 1.4)]),
            small: EmblemDrawing([G.r(4.4, 2, 15.2, 20.2, 2.4)], [G.r(7.6, 5, 8, 6.4, 1.2)], cut: [G.c(15.4, 12.4, 1.9)])),
        EmblemGlyph("bed", .home,
            detail: EmblemDrawing([G.r(2.6, 5, 3.4, 15, 1.4), G.r(2.6, 13.4, 18.8, 4.6, 1.4), G.r(19, 16, 2.4, 4, 0.8)],
                                  [G.r(7, 9.6, 5, 3.2, 1.4), G.r(12.6, 9, 8.8, 4.8, 1.6)]),
            small: EmblemDrawing([G.r(2, 4.4, 4, 16, 1.6), G.r(2, 13.2, 20, 5.4, 1.6), G.r(18.8, 16, 3, 4.6, 1)], [G.r(7.4, 8.6, 14, 5, 1.8)])),
        EmblemGlyph("umbrella", .home,
            detail: EmblemDrawing([G.f("M2.4 12.4 A9.6 9.6 0 0 1 21.6 12.4 Z"), G.s("M12 12 V18.6 A2 2 0 0 1 8 18.6", 2)],
                                  [G.f("M8.2 12.4 C8.6 7 10.2 3.4 12 2.8 C13.8 3.4 15.4 7 15.8 12.4 Z")]),
            small: EmblemDrawing([G.f("M1.8 12.6 A10.2 10.2 0 0 1 22.2 12.6 Z"), G.s("M12 12 V18.4 A2.3 2.3 0 0 1 7.4 18.4", 2.6)],
                                 [G.f("M8 12.6 C8.4 6.8 10.1 2.8 12 2.4 C13.9 2.8 15.6 6.8 16 12.6 Z")])),
    ]
}
