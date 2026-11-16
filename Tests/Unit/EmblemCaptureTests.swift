import CoreGraphics
import Foundation
import ImageIO
import SwiftUI
import Testing
import UniformTypeIdentifiers
@testable import WhooshUI

// Renders the emblem library and the list-row marks for checks/emblem-check.py. Runs only when WHOOSH_EMBLEM_CAPTURE
// names the output folder (the check passes _captures/emblem); otherwise it returns at once.
@Suite struct EmblemCaptureTests {
    static let sizes: [CGFloat] = [56, 20, 16]
    static let columns = 12

    @MainActor @Test func renderLibraryAndRows() throws {
        guard let out = ProcessInfo.processInfo.environment["WHOOSH_EMBLEM_CAPTURE"] else { return }
        let dir = URL(fileURLWithPath: out)
        try FileManager.default.createDirectory(at: dir.appendingPathComponent("rows"), withIntermediateDirectories: true)
        let accounts = try EmblemProbe.accounts()
        let colours = accounts.compactMap { $0["colour"] as? String }
        var cells: [[String: Any]] = [], rows: [[String: Any]] = []
        for dark in [false, true] {
            let scheme = dark ? "dark" : "light"
            for d in Self.sizes {
                let pitch = d * 2
                let sheet = ZStack(alignment: .topLeading) {
                    Rectangle().fill(WhooshPalette.background.color(dark ? .dark : .light))
                    ForEach(Array(EmblemLibrary.all.enumerated()), id: \.offset) { i, g in
                        EmblemMark(glyph: g, colourHex: colours[i % colours.count], size: .points(d))
                            .offset(x: CGFloat(i % Self.columns) * pitch + d / 2, y: CGFloat(i / Self.columns) * pitch + d / 2)
                    }
                }
                .frame(width: CGFloat(Self.columns) * pitch, height: CGFloat((EmblemLibrary.all.count + Self.columns - 1) / Self.columns) * pitch,
                       alignment: .topLeading)
                .environment(\.colorScheme, dark ? .dark : .light)
                try write(sheet, dir.appendingPathComponent("library-\(scheme)-\(Int(d)).png"))
                for (i, g) in EmblemLibrary.all.enumerated() {
                    let r = EmblemRecipe(colourHex: colours[i % colours.count], diameter: d, dark: dark)
                    cells.append(["glyph": g.id, "family": g.family.rawValue, "scheme": scheme, "size": Double(d),
                                  "x": Double(CGFloat(i % Self.columns) * pitch + d / 2), "y": Double(CGFloat(i / Self.columns) * pitch + d / 2),
                                  "clipFree": EmblemProbe.clipFree(g.drawing(r.tier), diameter: d, box: r.glyphBox)].merging(Self.json(r)) { a, _ in a })
                }
            }
            let assigned = try Self.assignments()
            for a in accounts {
                let id = a["id"] as? String ?? "", hex = a["colour"] as? String ?? ""
                let glyph = EmblemLibrary.resolve(assigned[id])
                let mark = EmblemMark(glyph: glyph, colourHex: hex, size: .row).environment(\.colorScheme, dark ? .dark : .light)
                try write(mark, dir.appendingPathComponent("rows/\(id)-\(scheme).png"))
                let list = EmblemRecipe(colourHex: hex, diameter: 16, dark: dark, listRow: true)
                let raw = EmblemRecipe(colourHex: hex, diameter: 16, dark: dark)
                rows.append(["account": id, "name": a["name"] ?? "", "glyph": glyph.id, "scheme": scheme, "list": Self.json(list),
                             "raw": Self.json(raw), "rawSaturation": raw.colour.saturation, "listSaturation": list.colour.saturation])
            }
        }
        let doc: [String: Any] = ["scale": 2, "columns": Self.columns, "cells": cells, "rows": rows,
                                  "EMBLEM_LIST_SATURATION": WhooshConstants.EMBLEM_LIST_SATURATION,
                                  "ICON_LIBRARY_MIN": WhooshConstants.ICON_LIBRARY_MIN, "glyphCount": EmblemLibrary.all.count]
        try JSONSerialization.data(withJSONObject: doc, options: [.prettyPrinted, .sortedKeys]).write(to: dir.appendingPathComponent("render.json"))
    }

    static func assignments() throws -> [String: String] {
        let data = try Data(contentsOf: EmblemProbe.root.appendingPathComponent("fixtures/emblem/accounts.json"))
        return (try JSONSerialization.jsonObject(with: data) as? [String: Any])?["accounts"] as? [String: String] ?? [:]
    }

    static func json(_ r: EmblemRecipe) -> [String: Any] {
        ["tier": r.tier.rawValue, "colour": r.colour.hex, "ink": r.ink.hex, "secondTone": r.secondTone.hex, "darkInk": r.darkInk,
         "baseTop": r.baseTop.hex, "baseBottom": r.baseBottom.hex, "specular": r.specular as Any, "innerShadow": r.innerShadow as Any,
         "rimLight": r.rimLight.map { Double($0) } as Any, "glyphShadow": r.glyphShadow, "glyphBox": Double(r.glyphBox)]
    }

    @MainActor private func write<V: View>(_ view: V, _ url: URL) throws {
        let renderer = ImageRenderer(content: view)
        renderer.scale = 2
        let image = try #require(renderer.cgImage, "nothing rendered for \(url.lastPathComponent)")
        let dest = try #require(CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil))
        CGImageDestinationAddImage(dest, image, nil)
        #expect(CGImageDestinationFinalize(dest))
    }
}
