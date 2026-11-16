import CoreGraphics

/// One part of a pictogram on the 24-unit grid: a filled outline, or a stroked line (round caps and joins).
public struct EmblemShape: Equatable, Sendable {
    public let d: String
    /// Stroke width in grid units; nil fills the outline.
    public let stroke: CGFloat?
    /// Dash length in grid units (the lifebuoy's bands); the phase is half a dash, as on the board.
    public let dash: CGFloat?
    /// Rotation in degrees about the grid centre (12, 12).
    public let rotation: CGFloat

    public init(d: String, stroke: CGFloat? = nil, dash: CGFloat? = nil, rotation: CGFloat = 0) {
        self.d = d; self.stroke = stroke; self.dash = dash; self.rotation = rotation
    }

    /// The outline in grid units, rotated.
    public var path: CGPath {
        let raw = EmblemPath.make(d)
        guard rotation != 0 else { return raw }
        var t = CGAffineTransform(translationX: 12, y: 12).rotated(by: rotation * .pi / 180).translatedBy(x: -12, y: -12)
        return raw.copy(using: &t) ?? raw
    }

    /// The area this part paints, strokes widened to their real width.
    public var area: CGPath {
        guard let stroke else { return path }
        return path.copy(strokingWithWidth: stroke, lineCap: .round, lineJoin: .round, miterLimit: 4)
    }
}

/// One drawing of a pictogram: the ink tone, the second tone drawn over it, and the cut-out.
/// `cutsBoth` applies the cut-out to the second tone as well (the board's masked second groups: mountain, cloud).
public struct EmblemDrawing: Equatable, Sendable {
    public let primary: [EmblemShape]
    public let secondary: [EmblemShape]
    public let cut: [EmblemShape]
    public let cutsBoth: Bool

    public init(_ primary: [EmblemShape], _ secondary: [EmblemShape] = [], cut: [EmblemShape] = [], cutsBoth: Bool = false) {
        self.primary = primary; self.secondary = secondary; self.cut = cut; self.cutsBoth = cutsBoth
    }
}

/// A library glyph: the detailed drawing (40 pt and up) and its own simplified drawing (39 pt and down).
public struct EmblemGlyph: Identifiable, Equatable, Sendable {
    public let id: String
    public let family: EmblemFamily
    public let detail: EmblemDrawing
    public let small: EmblemDrawing

    public init(_ id: String, _ family: EmblemFamily, detail: EmblemDrawing, small: EmblemDrawing) {
        self.id = id; self.family = family; self.detail = detail; self.small = small
    }

    public var name: String { id.replacingOccurrences(of: "-", with: " ") }
    public func drawing(_ tier: EmblemTier) -> EmblemDrawing { tier == .detailed ? detail : small }
}

/// Shape builders in the board's own vocabulary (rect with rx, circle, ellipse, stroked line).
enum G {
    static func f(_ d: String, rot: CGFloat = 0) -> EmblemShape { EmblemShape(d: d, rotation: rot) }
    static func s(_ d: String, _ w: CGFloat, rot: CGFloat = 0) -> EmblemShape { EmblemShape(d: d, stroke: w, rotation: rot) }
    static func r(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat, _ h: CGFloat, _ rx: CGFloat, rot: CGFloat = 0) -> EmblemShape {
        f(rectD(x, y, w, h, rx), rot: rot)
    }
    static func c(_ cx: CGFloat, _ cy: CGFloat, _ r: CGFloat) -> EmblemShape { f(ellipseD(cx, cy, r, r, 0)) }
    static func ring(_ cx: CGFloat, _ cy: CGFloat, _ r: CGFloat, _ w: CGFloat, dash: CGFloat? = nil) -> EmblemShape {
        EmblemShape(d: ellipseD(cx, cy, r, r, 0), stroke: w, dash: dash)
    }
    static func ellipse(_ cx: CGFloat, _ cy: CGFloat, _ rx: CGFloat, _ ry: CGFloat, _ deg: CGFloat, _ w: CGFloat) -> EmblemShape {
        EmblemShape(d: ellipseD(cx, cy, rx, ry, deg), stroke: w)
    }
    static func rectD(_ x: CGFloat, _ y: CGFloat, _ w: CGFloat, _ h: CGFloat, _ r0: CGFloat) -> String {
        let r = min(r0, w / 2, h / 2)
        return "M\(x + r) \(y) H\(x + w - r) A\(r) \(r) 0 0 1 \(x + w) \(y + r) V\(y + h - r) A\(r) \(r) 0 0 1 \(x + w - r) \(y + h) "
            + "H\(x + r) A\(r) \(r) 0 0 1 \(x) \(y + h - r) V\(y + r) A\(r) \(r) 0 0 1 \(x + r) \(y) Z"
    }
    static func ellipseD(_ cx: CGFloat, _ cy: CGFloat, _ rx: CGFloat, _ ry: CGFloat, _ deg: CGFloat) -> String {
        let a = deg * .pi / 180, dx = rx * cos(a), dy = rx * sin(a)
        return "M\(cx - dx) \(cy - dy) A\(rx) \(ry) \(deg) 1 0 \(cx + dx) \(cy + dy) A\(rx) \(ry) \(deg) 1 0 \(cx - dx) \(cy - dy) Z"
    }
}
