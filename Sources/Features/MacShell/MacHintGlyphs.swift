import SwiftUI
#if os(macOS)
import AppKit
#endif

/// The ink of each row-strip icon (SF Symbol at the strip's subhead size, centred in its frame), measured once by
/// rendering the symbol and scanning its alpha. A row cap floats above this ink and never covers it.
/// Served as `rows mac-main.hints.glyphs`: per row cap, the control's frame and its glyph's ink box, in window points.
@MainActor
enum MacHintGlyphs {
    private static var cache: [String: (top: CGFloat, bottom: CGFloat, width: CGFloat)] = [:]

    /// Ink rows of a symbol inside its image, in points from the image's top.
    static func ink(_ symbol: String) -> (top: CGFloat, bottom: CGFloat, width: CGFloat, height: CGFloat) {
        #if os(macOS)
        let cfg = NSImage.SymbolConfiguration(pointSize: WhooshTypography.subhead.size, weight: .regular)
        guard let i = NSImage(systemSymbolName: symbol, accessibilityDescription: nil)?.withSymbolConfiguration(cfg) else { return (0, 0, 0, 0) }
        if let c = cache[symbol] { return (c.top, c.bottom, c.width, i.size.height) }
        let scale = 4, w = Int(i.size.width) * scale, h = Int(i.size.height) * scale
        guard w > 0, h > 0, let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: w, pixelsHigh: h, bitsPerSample: 8,
                                                       samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
                                                       bytesPerRow: 0, bitsPerPixel: 0) else { return (0, 0, 0, 0) }
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
        i.draw(in: NSRect(x: 0, y: 0, width: w, height: h))
        NSGraphicsContext.restoreGraphicsState()
        var top = h, bottom = -1
        for y in 0..<h { for x in 0..<w where (rep.colorAt(x: x, y: y)?.alphaComponent ?? 0) > 0.1 { top = min(top, y); bottom = max(bottom, y); break } }
        let r = (CGFloat(top) / CGFloat(scale), CGFloat(bottom + 1) / CGFloat(scale), i.size.width)
        cache[symbol] = (r.0, r.1, r.2)
        return (r.0, r.1, r.2, i.size.height)
        #else
        return (0, 0, 0, 0)
        #endif
    }

    static func symbol(forAx ax: String) -> String? {
        guard let r = ax.range(of: ".act.") else { return nil }
        let id = String(ax[r.upperBound...])
        return RowStrip.items.first { $0.id == id }?.symbol
    }

    /// The glyph's ink box inside a control frame (the image is centred in the frame).
    static func box(_ symbol: String, in f: CGRect) -> CGRect {
        let k = ink(symbol)
        let imageTop = f.midY - k.height / 2
        return CGRect(x: f.midX - k.width / 2, y: imageTop + k.top, width: k.width, height: k.bottom - k.top)
    }

    /// The highest ink top across the strip's icons, from the frame's top: one line for every cap of a row.
    static func stripInkTop(frameHeight: CGFloat) -> CGFloat {
        RowStrip.items.map { s -> CGFloat in let k = ink(s.symbol); return (frameHeight - k.height) / 2 + k.top }.min() ?? 0
    }

    static func rows() -> [ControlRow] {
        FrameProbe.ids.compactMap { ax -> ControlRow? in
            guard ax.contains(".act."), let s = symbol(forAx: ax), let f = MacHintTable.liveFrame(ax) else { return nil }
            let g = box(s, in: f)
            return ["ax": ax, "symbol": s, "glyph": ["x": g.minX, "y": g.minY, "w": g.width, "h": g.height]]
        }
    }
}
