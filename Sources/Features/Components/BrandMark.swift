import SwiftUI

/// Which glass a BrandMark draws, readable by the socket: the system's Liquid Glass or the macOS 15 recipe.
public enum BrandMaterial: String, Sendable {
    case glass26 = "glass-26", glass15 = "glass-15"
    @MainActor public static var current: BrandMaterial {
        if #available(macOS 26.0, iOS 26.0, *), !WhooshRigStandIn.isOn { return .glass26 }
        return .glass15
    }
}

/// What is on screen now, for `rows whoosh.brand-mark` (ComponentsFeature). Each mark registers on appear, leaves on disappear.
/// `ax` is the id the mark carries as a measurable view (`.whooshAx`), so the socket's `measure <ax>` reads its RENDERED frame;
/// `size` is only what the call site asked for. `material` is what the glass modifier reported when it drew: a mark whose glass
/// was removed never reports and reads "flat".
@MainActor
enum BrandMarkRegistry {
    static let flat = "flat"
    private static var live: [UUID: (ax: String, surface: String, size: CGFloat)] = [:]
    private static var drawn: [UUID: String] = [:]
    static func axId(_ id: UUID, surface: String) -> String {
        "whoosh.brand-mark.\(surface.isEmpty ? "none" : surface).\(id.uuidString.prefix(8).lowercased())"
    }
    static func add(_ id: UUID, surface: String, size: CGFloat) { live[id] = (axId(id, surface: surface), surface, size) }
    static func setMaterial(_ id: UUID, _ material: String) { drawn[id] = material }
    static func remove(_ id: UUID) { live[id] = nil; drawn[id] = nil }
    static func rows() -> [ControlRow] {
        live.sorted { ($0.value.surface, $0.value.size, $0.value.ax) < ($1.value.surface, $1.value.size, $1.value.ax) }.map { id, v in
            ["ax": v.ax, "id": "whoosh.brand-mark", "surface": v.surface, "size": Double(v.size), "material": drawn[id] ?? flat]
        }
    }
}

/// The Whoosh envelope as thick frosted glass: ONE view for every place the mark is drawn (add-account head, welcome card,
/// sidebar empty state). `size` is the envelope's height in points; the width follows the SVG's own proportions.
///
/// macOS 26 / iOS 26: the system's `glassEffect(.regular)` in the envelope shape, through WhooshGlass.
/// macOS 15 / iOS 18, from the owner's references (docs/design/brand/references/liquid-glass/README.md):
///   1. body: the shape over .ultraThinMaterial plus the glass tint (ref-1, thick frosted body);
///   2. refraction (ref-3): a faint, blurred, offset copy of soft caustic colour clipped inside the shape. SwiftUI cannot sample
///      the pixels behind a view, so the caustic is the accent family, not the real backdrop;
///   3. inner shadow (ref-4): a blurred dark stroke masked to the inside, offset down;
///   4. top specular band (ref-2): a white-to-clear gradient over the upper half, clipped to the shape;
///   5. rim (ref-3): a 1 pt white-to-clear stroke, brightest at the top;
///   6. the quiet shadow under it (ref-1).
/// Motion on it stays transform and opacity (`.transition(.opacity)` or a scale at the call site).
public struct BrandMark: View {
    let size: CGFloat
    let surface: String
    @Environment(\.colorScheme) private var scheme
    @State private var token = UUID()
    @State private var drawn = BrandMarkRegistry.flat

    public init(size: CGFloat = 56, surface: String = "") { self.size = size; self.surface = surface }

    private var shape: EnvelopeShape { EnvelopeShape() }

    public var body: some View {
        let h = size, w = size * EnvelopeShape.aspect
        let dark = scheme == .dark
        let t = token
        Color.clear
            .frame(width: w, height: h)
            // the seam: system glass on 26, tint + material + rim + shadow on 15; it reports the material it drew
            .whooshGlass(in: shape, report: { m in BrandMarkRegistry.setMaterial(t, m); drawn = m })
            .overlay { if BrandMaterial.current == .glass15 { depth(w: w, h: h, dark: dark) } }
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("Whoosh")
            .accessibilityValue(drawn)
            .whooshAx(BrandMarkRegistry.axId(token, surface: surface))   // sets the accessibility id and lets `measure` read the drawn frame
            .onAppear { BrandMarkRegistry.add(token, surface: surface, size: size) }
            // the surface and size can change after the first frame: the add-account head decides its surface from `model.style`, which the
            // inline card's own onAppear sets AFTER this child's (SwiftUI appears children first), so Welcome's mark registered as
            // "add-account" at 36 pt and the welcome check read no mark (main50 to main52). The row follows the value.
            .onChange(of: surface) { _, now in BrandMarkRegistry.add(token, surface: now, size: size) }
            .onChange(of: size) { _, now in BrandMarkRegistry.add(token, surface: surface, size: now) }
            .onDisappear { BrandMarkRegistry.remove(token) }
    }

    /// The macOS 15 layers 2 to 5, all clipped to the envelope.
    private func depth(w: CGFloat, h: CGFloat, dark: Bool) -> some View {
        ZStack {
            // 2. refraction: soft caustic colour, offset and blurred, inside the glass
            ZStack {
                Circle().fill(WhooshPalette.accent.color(scheme).opacity(dark ? 0.30 : 0.22)).frame(width: h * 0.8, height: h * 0.8).offset(x: -w * 0.18, y: h * 0.12)
                Circle().fill(WhooshPalette.warning.color(scheme).opacity(dark ? 0.16 : 0.12)).frame(width: h * 0.6, height: h * 0.6).offset(x: w * 0.22, y: -h * 0.10)
            }
            .blur(radius: h * 0.16)
            // 3. inner shadow: a blurred dark rim, masked to the inside, pushed down
            shape.stroke(Color.black.opacity(dark ? 0.55 : 0.22), lineWidth: max(2, h * 0.08)).blur(radius: max(1.5, h * 0.05)).offset(y: h * 0.03)
            // 4. specular band over the upper half
            LinearGradient(stops: [.init(color: .white.opacity(dark ? 0.22 : 0.55), location: 0), .init(color: .white.opacity(0), location: 0.5)],
                           startPoint: .top, endPoint: .bottom)
        }
        .frame(width: w, height: h)
        .clipShape(shape)
        .overlay {
            // 5. rim: 1 pt, white at the top fading to clear at the bottom
            shape.strokeBorder(LinearGradient(colors: [.white.opacity(dark ? 0.55 : 0.95), .white.opacity(0)], startPoint: .top, endPoint: .bottom), lineWidth: 1)
        }
        .allowsHitTesting(false)
    }
}
