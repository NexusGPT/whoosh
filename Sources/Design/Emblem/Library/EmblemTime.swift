import CoreGraphics
// Time and safety family. stopwatch: board glyph, verbatim.
enum EmblemTime {
    static let glyphs: [EmblemGlyph] = [
        EmblemGlyph("stopwatch", .time,
            detail: EmblemDrawing([G.c(12, 13.6, 8), G.r(9.8, 1.6, 4.4, 2.6, 1.1), G.r(11, 3.4, 2, 3, 0.4)], [G.f("M12 13.6 V7.4 A6.2 6.2 0 0 1 17.6 16.3 Z")],
                                  cut: [G.s("M12 13.6 V8.6", 1.7), G.c(12, 13.6, 1.6)]),
            small: EmblemDrawing([G.c(12, 13.8, 8.4), G.r(9.4, 1.2, 5.2, 2.8, 1.2), G.r(10.8, 3.2, 2.4, 3, 0.4)], [G.f("M12 13.8 V7 A6.8 6.8 0 0 1 18.1 16.8 Z")],
                                 cut: [G.s("M12 13.8 V8.4", 2.3), G.c(12, 13.8, 2)])),
        EmblemGlyph("clock", .time,
            detail: EmblemDrawing([G.c(12, 12, 9.2)], [G.f("M12 12 V2.8 A9.2 9.2 0 0 1 21.2 12 Z")], cut: [G.s("M12 6.4 V12 L15.6 14.2", 1.7)], cutsBoth: true),
            small: EmblemDrawing([G.c(12, 12, 9.8)], [G.f("M12 12 V2.2 A9.8 9.8 0 0 1 21.8 12 Z")], cut: [G.s("M12 6 V12 L16 14.4", 2.3)], cutsBoth: true)),
        EmblemGlyph("hourglass", .time,
            detail: EmblemDrawing([G.f("M5 2.6 H19 V4.4 H17.8 C17.8 8.4 14.6 10.4 13.4 12 C14.6 13.6 17.8 15.6 17.8 19.6 H19 V21.4 H5 V19.6 H6.2 C6.2 15.6 9.4 13.6 10.6 12 C9.4 10.4 6.2 8.4 6.2 4.4 H5 Z")],
                                  [G.f("M8 19.6 C8.4 17 10.6 15.8 12 15.4 C13.4 15.8 15.6 17 16 19.6 Z"), G.f("M8.4 6.8 H15.6 C15 8.6 13.4 9.6 12 10.4 C10.6 9.6 9 8.6 8.4 6.8 Z")]),
            small: EmblemDrawing([G.f("M4.4 2 H19.6 V4.2 H18.4 C18.4 8.4 15 10.4 13.6 12 C15 13.6 18.4 15.6 18.4 19.8 H19.6 V22 H4.4 V19.8 H5.6 C5.6 15.6 9 13.6 10.4 12 C9 10.4 5.6 8.4 5.6 4.2 H4.4 Z")],
                                 [G.f("M7.6 19.8 C8 16.8 10.4 15.4 12 15 C13.6 15.4 16 16.8 16.4 19.8 Z")])),
        EmblemGlyph("lock", .time,
            detail: EmblemDrawing([G.r(4, 10, 16, 11.6, 2.6)], [G.s("M7.8 10 V7.4 A4.2 4.2 0 0 1 16.2 7.4 V10", 2.2)],
                                  cut: [G.c(12, 14.6, 1.6), G.r(11.3, 15, 1.4, 3.4, 0.6)]),
            small: EmblemDrawing([G.r(3.4, 9.8, 17.2, 12.4, 2.8)], [G.s("M7.4 9.8 V7 A4.6 4.6 0 0 1 16.6 7 V9.8", 2.8)],
                                 cut: [G.c(12, 14.8, 2.2), G.r(11, 15.4, 2, 3.6, 0.8)])),
        EmblemGlyph("shield", .time,
            detail: EmblemDrawing([G.f("M12 2.4 L20 5.4 V11.4 C20 16.2 16.6 19.8 12 21.6 C7.4 19.8 4 16.2 4 11.4 V5.4 Z")],
                                  [G.f("M12 2.4 L20 5.4 V11.4 C20 16.2 16.6 19.8 12 21.6 Z")], cut: [G.s("M8.4 12 L11 14.6 L15.8 9.6", 1.6)], cutsBoth: true),
            small: EmblemDrawing([G.f("M12 1.6 L20.8 4.8 V11.4 C20.8 16.6 17.2 20.6 12 22.4 C6.8 20.6 3.2 16.6 3.2 11.4 V4.8 Z")],
                                 [G.f("M12 1.6 L20.8 4.8 V11.4 C20.8 16.6 17.2 20.6 12 22.4 Z")], cut: [G.s("M8 12 L11 15 L16.2 9.6", 2.3)], cutsBoth: true)),
        EmblemGlyph("sparkle", .time,
            detail: EmblemDrawing([G.f("M10 3 C10.8 8 12 9.2 17 10 C12 10.8 10.8 12 10 17 C9.2 12 8 10.8 3 10 C8 9.2 9.2 8 10 3 Z")],
                                  [G.f("M17.6 13.4 C18 16 18.6 16.6 21.2 17 C18.6 17.4 18 18 17.6 20.6 C17.2 18 16.6 17.4 14 17 C16.6 16.6 17.2 16 17.6 13.4 Z")]),
            small: EmblemDrawing([G.f("M10 2 C10.9 7.6 12.4 9.1 18 10 C12.4 10.9 10.9 12.4 10 18 C9.1 12.4 7.6 10.9 2 10 C7.6 9.1 9.1 7.6 10 2 Z")],
                                 [G.f("M17.8 12.8 C18.3 15.8 19 16.5 22 17 C19 17.5 18.3 18.2 17.8 21.2 C17.3 18.2 16.6 17.5 13.6 17 C16.6 16.5 17.3 15.8 17.8 12.8 Z")])),
    ]
}
