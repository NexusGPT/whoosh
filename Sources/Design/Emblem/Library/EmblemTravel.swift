import CoreGraphics
// Travel family. airplane and sailboat: board glyphs, verbatim (the airplane is drawn at 40 degrees).
enum EmblemTravel {
    static let glyphs: [EmblemGlyph] = [
        EmblemGlyph("airplane", .travel,
            detail: EmblemDrawing([G.f("M13.5 9.6 L21.4 14 V16 L13.5 13.6 V18.4 L15.6 19.9 V21.6 L12 20.6 L8.4 21.6 V19.9 L10.5 18.4 V13.6 L2.6 16 V14 L10.5 9.6 V4.2 A1.5 1.5 0 0 1 13.5 4.2 Z", rot: 40)],
                                  [G.f("M13.5 9.6 L21.4 14 V16 L13.5 13.6 Z", rot: 40)]),
            small: EmblemDrawing([G.f("M13.8 9.4 L22.4 14 V16.4 L13.8 13.8 V18.2 L16 19.8 V22 L12 20.9 L8 22 V19.8 L10.2 18.2 V13.8 L1.6 16.4 V14 L10.2 9.4 V4 A1.8 1.8 0 0 1 13.8 4 Z", rot: 40)],
                                 [G.f("M13.8 9.4 L22.4 14 V16.4 L13.8 13.8 Z", rot: 40)])),
        EmblemGlyph("sailboat", .travel,
            detail: EmblemDrawing([G.f("M11 2.6 V15.6 H3.8 Z"), G.f("M2.6 17.4 H21.4 L18.6 21.2 H5.4 Z")], [G.f("M12.8 6 V15.6 H19.4 Z")]),
            small: EmblemDrawing([G.f("M10.8 2 V15.4 H3 Z"), G.f("M1.8 17.4 H22.2 L19.2 21.8 H4.8 Z")], [G.f("M12.9 5.4 V15.4 H20.2 Z")])),
        EmblemGlyph("suitcase", .travel,
            detail: EmblemDrawing([G.r(3, 7.4, 18, 13.6, 2.6), G.r(8.6, 3.4, 6.8, 5.6, 2)], [G.r(7, 7.4, 2.2, 13.6, 0.4), G.r(14.8, 7.4, 2.2, 13.6, 0.4)],
                                  cut: [G.r(10.4, 5.2, 3.2, 2.4, 0.7)]),
            small: EmblemDrawing([G.r(2.4, 7, 19.2, 14.6, 2.8), G.r(8, 2.8, 8, 6, 2.2)], [G.r(6.6, 7, 2.8, 14.6, 0.4), G.r(14.6, 7, 2.8, 14.6, 0.4)],
                                 cut: [G.r(10, 4.8, 4, 2.6, 0.7)])),
        EmblemGlyph("globe", .travel,
            detail: EmblemDrawing([G.c(12, 12, 9.2)], [G.f("M12 2.8 C8.6 6 8.6 18 12 21.2 C15.4 18 15.4 6 12 2.8 Z")],
                                  cut: [G.s("M3.4 12 H20.6", 1.3), G.s("M5 7.6 H19 M5 16.4 H19", 1.1)], cutsBoth: true),
            small: EmblemDrawing([G.c(12, 12, 9.8)], [G.f("M12 2.2 C8.2 5.8 8.2 18.2 12 21.8 C15.8 18.2 15.8 5.8 12 2.2 Z")],
                                 cut: [G.s("M2.6 12 H21.4", 2)], cutsBoth: true)),
        EmblemGlyph("compass", .travel,
            detail: EmblemDrawing([G.c(12, 12, 9.2)], [G.f("M15.8 8.2 L13.4 13.4 L8.2 15.8 L10.6 10.6 Z")], cut: [G.ring(12, 12, 7.6, 1.2)]),
            small: EmblemDrawing([G.c(12, 12, 9.8)], [G.f("M16.6 7.4 L13.8 13.8 L7.4 16.6 L10.2 10.2 Z")], cut: [G.ring(12, 12, 8, 1.6)])),
        EmblemGlyph("ticket", .travel,
            detail: EmblemDrawing([G.f("M3 6 H21 V9.6 A2.4 2.4 0 0 0 21 14.4 V18 H3 V14.4 A2.4 2.4 0 0 0 3 9.6 Z")],
                                  [G.f("M15.4 6 H21 V9.6 A2.4 2.4 0 0 0 21 14.4 V18 H15.4 Z")], cut: [G.s("M15.4 7.6 V16.4", 1.1)], cutsBoth: true),
            small: EmblemDrawing([G.f("M2.2 5.4 H21.8 V9.4 A2.6 2.6 0 0 0 21.8 14.6 V18.6 H2.2 V14.6 A2.6 2.6 0 0 0 2.2 9.4 Z")],
                                 [G.f("M15.6 5.4 H21.8 V9.4 A2.6 2.6 0 0 0 21.8 14.6 V18.6 H15.6 Z")], cut: [G.s("M15.6 7 V17", 1.8)], cutsBoth: true)),
    ]
}
