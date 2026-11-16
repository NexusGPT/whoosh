import CoreGraphics
import Foundation
import SwiftUI
import Testing
@testable import WhooshUI

// Written from the emblem board (docs/design/boards/badges/emblem), docs/DESIGN.md "## Account badge: emblem" and
// docs/CONSTANTS.md. MUTANTS: each test names, in its trailing comment, the mutation that must turn it red.

enum EmblemProbe {
    static let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    static func accounts() throws -> [[String: Any]] {
        let data = try Data(contentsOf: root.appendingPathComponent("docs/design/fixtures/mac-main-sample.json"))
        let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        return try #require(json?["accounts"] as? [[String: Any]])
    }
    /// True when every painted point of the drawing lands inside the disc, less the 0.5 pt hairline.
    static func clipFree(_ drawing: EmblemDrawing, diameter d: CGFloat, box: CGFloat) -> Bool {
        let shapes = (drawing.primary + drawing.secondary).map(\.area)
        let k = box / 24, o = (d - box) / 2, limit = d / 2 - EmblemConstruction.hairline
        var y: CGFloat = -2
        while y <= 26 {
            var x: CGFloat = -2
            while x <= 26 {
                let px = o + x * k - d / 2, py = o + y * k + d * EmblemConstruction.glyphDrop - d / 2
                if (px * px + py * py).squareRoot() > limit, shapes.contains(where: { $0.contains(CGPoint(x: x, y: y)) }) { return false }
                x += 0.2
            }
            y += 0.2
        }
        return true
    }
}

@Suite struct EmblemTests {
    @Test func theLibraryHoldsAtLeastTheMinimumInNamedFamilies() {
        #expect(EmblemLibrary.all.count >= WhooshConstants.ICON_LIBRARY_MIN)          // mutant: drop a family file
        #expect(Set(EmblemLibrary.all.map(\.id)).count == EmblemLibrary.all.count)      // mutant: a duplicated id
        for f in EmblemFamily.allCases { #expect(EmblemLibrary.glyphs(in: f).count >= 4, "\(f) is thin") }
        for g in EmblemLibrary.all { #expect(g.family == EmblemLibrary.glyphs(in: g.family).first { $0.id == g.id }?.family) }
    }

    @Test func everyGlyphHasTwoSeparateDrawingsThatParse() {
        for g in EmblemLibrary.all {
            #expect(g.detail != g.small, "\(g.id): the 16 pt drawing is the 56 pt one")   // mutant: small = detail
            for s in g.detail.primary + g.detail.secondary + g.small.primary + g.small.secondary {
                #expect(!s.area.boundingBoxOfPath.isEmpty, "\(g.id): a part does not parse")
            }
            #expect(!g.detail.primary.isEmpty && !g.small.primary.isEmpty)
            // The simplified drawing never thins a cut-out: every stroked cut is at least as wide as the detailed one.
            let dw = g.detail.cut.compactMap(\.stroke).min() ?? 0, sw = g.small.cut.compactMap(\.stroke).min() ?? .infinity
            #expect(g.small.cut.isEmpty || sw >= dw, "\(g.id): a 16 pt cut-out is thinner")
        }
    }

    @Test func noGlyphIsClippedAtAnyTier() {
        for g in EmblemLibrary.all {
            for d in [CGFloat(56), 20, 16] {
                let r = EmblemRecipe(colourHex: "#2F6FED", diameter: d, dark: false)
                #expect(EmblemProbe.clipFree(g.drawing(r.tier), diameter: d, box: r.glyphBox), "\(g.id) clips at \(d) pt")
            }
        }
    }

    @Test func theTinyTierDropsSpecularInnerShadowAndGlyphShadow() {
        for d in [CGFloat(14), 16, 18] {
            let r = EmblemRecipe(colourHex: "#2F6FED", diameter: d, dark: false)
            #expect(r.tier == .tiny && r.specular == nil && r.innerShadow == nil && !r.glyphShadow && r.rimLight == nil)  // mutant: specular at 16
        }
        let big = EmblemRecipe(colourHex: "#2F6FED", diameter: 56, dark: true)
        #expect(big.specular != nil && big.innerShadow != nil && big.glyphShadow && big.rimLight != nil)
        #expect(EmblemRecipe(colourHex: "#2F6FED", diameter: 20, dark: false).tier == .simplified)
    }

    @Test func theListRowMarkUsesTheListSaturation() {
        let raw = EmblemRGB(hex: "#2F6FED")!
        let list = EmblemRecipe(colourHex: "#2F6FED", diameter: 16, dark: false, listRow: true)
        #expect(abs(list.colour.saturation - raw.saturation * WhooshConstants.EMBLEM_LIST_SATURATION) < 0.005)  // mutant: full colour on rows
        // swift-tests lane: the quiet saturation also applies to the sidebar and compact sizes since commit 32250e21 ("the sidebar lane measured the marks as loud";
        // Sources/Design/Emblem/EmblemMark.swift:19-22, docs/lanes/emblem.md:99). Settings and reply keep the full colour.
        #expect(EmblemSize.row.isListRow && EmblemSize.sidebar.isListRow && EmblemSize.compact.isListRow)
        #expect(!EmblemSize.settings.isListRow && !EmblemSize.reply.isListRow)
    }

    @Test func fiveWarmFixtureColoursTakeTheDarkInk() throws {
        let dark = try EmblemProbe.accounts().filter { EmblemRecipe.inkIsDark(EmblemRGB(hex: $0["colour"] as? String ?? "")!) }
        #expect(Set(dark.compactMap { $0["name"] as? String }) == ["Nexus billing", "Family", "LHG", "Provanta RFP", "Photo"])  // mutant: 4.5:1 rule
        for a in try EmblemProbe.accounts() {
            let r = EmblemRecipe(colourHex: a["colour"] as? String ?? "", diameter: 56, dark: false)
            for s in [0.2, 0.8] { #expect(r.ink.contrast(r.base(at: s)) >= 3, "\(a["name"] ?? "") ink under 3:1") }
        }
    }

    @Test func oldRingIconIdsResolveToEmblems() {
        for id in AccountMarkIcon.libraryIds.keys { #expect(EmblemLibrary.resolve(id).id != "" ) }
        #expect(EmblemLibrary.resolve("plane").id == "airplane")
        #expect(EmblemLibrary.resolve(nil).id == EmblemLibrary.fallback.id)
    }

    @MainActor @Test func theHiddenSizeDrawsNothing() {
        let hidden = ImageRenderer(content: EmblemMark(glyph: EmblemLibrary.all[0], colourHex: "#2F6FED", size: .hidden))
        let shown = ImageRenderer(content: EmblemMark(glyph: EmblemLibrary.all[0], colourHex: "#2F6FED", size: .row))
        #expect((hidden.cgImage?.width ?? 0) == 0)                                     // mutant: hidden draws the row mark
        #expect((shown.cgImage?.width ?? 0) > 0)
    }
}
