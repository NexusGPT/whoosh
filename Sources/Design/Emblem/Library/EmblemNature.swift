import CoreGraphics
// Nature family. sprout and cloud: board glyphs, verbatim.
enum EmblemNature {
    static let glyphs: [EmblemGlyph] = [
        EmblemGlyph("sprout", .nature,
            detail: EmblemDrawing([G.r(11, 10.5, 2, 9.5, 1), G.f("M12.3 11.2 C12.2 5.4 15.6 2.6 21 2.8 C21.3 8.4 18 11.6 12.3 11.2 Z"), G.r(5.5, 19.4, 13, 2.6, 1.3)],
                                  [G.f("M11.7 13.6 C5.2 14 3 9.8 3.4 5.6 C8.4 5.2 12 8 11.7 13.6 Z")], cut: [G.s("M13.8 9.6 L18.6 5", 1.3)]),
            small: EmblemDrawing([G.r(10.8, 10.5, 2.4, 10, 1.2), G.f("M12.4 11.6 C12.3 5.2 15.8 2.2 21.6 2.4 C21.9 8.6 18.3 12 12.4 11.6 Z"), G.r(5, 19.2, 14, 2.9, 1.45)],
                                 [G.f("M11.6 14.2 C4.6 14.6 2.4 10 2.8 5.4 C8.2 5 12 8 11.6 14.2 Z")], cut: [G.s("M14.2 9.6 L19 4.8", 1.9)])),
        EmblemGlyph("cloud", .nature,
            detail: EmblemDrawing([G.c(17.2, 8.6, 3.7)], [G.f("M7 19.2 H17.4 A4.4 4.4 0 0 0 18 10.45 A6 6 0 0 0 6.5 10.8 A4.2 4.2 0 0 0 7 19.2 Z")],
                                  cut: [G.s("M12 10.6 V16 M9.6 13.8 L12 16.2 L14.4 13.8", 1.7)], cutsBoth: true),
            small: EmblemDrawing([G.c(17.4, 8.4, 3.9)], [G.f("M6.8 19.8 H17.6 A4.6 4.6 0 0 0 18.2 10.6 A6.3 6.3 0 0 0 6.2 10.9 A4.45 4.45 0 0 0 6.8 19.8 Z")],
                                 cut: [G.s("M12 10.4 V16.2 M9.2 13.6 L12 16.4 L14.8 13.6", 2.3)], cutsBoth: true)),
        EmblemGlyph("sun", .nature,
            detail: EmblemDrawing([G.c(12, 12, 5)], stride(from: 0, to: 360, by: 45).map { G.r(11, 1.6, 2, 4, 1, rot: CGFloat($0)) }),
            small: EmblemDrawing([G.c(12, 12, 5.6)], stride(from: 0, to: 360, by: 45).map { G.r(10.8, 0.8, 2.4, 4.6, 1.2, rot: CGFloat($0)) })),
        EmblemGlyph("moon", .nature,
            detail: EmblemDrawing([G.c(11, 12.4, 8.6)], [G.f("M17.4 3.4 L18.4 5.8 L20.8 6.8 L18.4 7.8 L17.4 10.2 L16.4 7.8 L14 6.8 L16.4 5.8 Z")],
                                  cut: [G.c(15.6, 8.4, 6.4)]),
            small: EmblemDrawing([G.c(10.8, 12.6, 9.4)], [G.f("M17.6 2.6 L18.8 5.6 L21.8 6.8 L18.8 8 L17.6 11 L16.4 8 L13.4 6.8 L16.4 5.6 Z")],
                                 cut: [G.c(15.8, 8.2, 6.8)])),
        EmblemGlyph("leaf", .nature,
            detail: EmblemDrawing([G.f("M4 20 C3.6 10.6 8.6 3.6 20.4 3.6 C20.4 15.2 13.6 20.4 4 20 Z")], [G.f("M4 20 L20.4 3.6 C20.4 15.2 13.6 20.4 4 20 Z")],
                                  cut: [G.s("M5.4 18.6 L15.8 8.2", 1.3)], cutsBoth: true),
            small: EmblemDrawing([G.f("M3.4 20.6 C3 10.2 8.4 2.8 21 2.8 C21 15.4 13.8 21 3.4 20.6 Z")], [G.f("M3.4 20.6 L21 2.8 C21 15.4 13.8 21 3.4 20.6 Z")],
                                 cut: [G.s("M5 19 L16 8", 1.9)], cutsBoth: true)),
        EmblemGlyph("tree", .nature,
            detail: EmblemDrawing([G.c(12, 9.4, 7)], [G.r(10.8, 13.6, 2.4, 7.6, 1), G.r(6.4, 19.6, 11.2, 2.2, 1.1)]),
            small: EmblemDrawing([G.c(12, 9, 7.8)], [G.r(10.6, 13.6, 2.8, 8, 1.2), G.r(5.8, 19.6, 12.4, 2.6, 1.3)])),
    ]
}
