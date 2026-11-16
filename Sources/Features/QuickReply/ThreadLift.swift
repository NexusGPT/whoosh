import SwiftUI

/// The one material of the thread's foot (board cards-s3, owner pick 00:35 "Lifted until it lands"): what is still
/// yours (a reply in its undo window, a draft, a failed send) floats as lifted glass; once a reply has gone it LANDS
/// and lies flat like every message: no fill, the hairline edge only (round 18; the `mine` tint before). The landing is opacity and transform
/// only, never a layout change, and it gates no key: the state flips on the input frame, the material catches up.
/// The glass clears on the SAME clock as the Aurora landing light (`AuroraLanding.progress`, LAND_AURORA_MS), so the
/// frost dissolves in the light's wake and the two can never drift apart; the hairline fades in and the card settles
/// LAND_SETTLE_PT into the page on that clock too. Reduced motion: no light, no settle, one LAND_STATE_MS cross-fade.
struct ThreadLift: ViewModifier {
    /// true while the card is still yours to change; false once it is on the record.
    let lifted: Bool
    /// The landing in flight (its start is the landing frame); nil while lifted, or for a card that was always flat.
    let landing: AuroraLanding?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var fade: Double
    /// true only while the landing light runs: the clock ticks then, and stops the frame the light ends (no idle ticking).
    @State private var lit = false
    /// The landing is over by the wall clock: the end state holds whether or not a display tick ever drew the light
    /// (a hidden rig window may not tick).
    @State private var done = false
    private typealias L = WhooshGlass.Lift
    private typealias C = WhooshConstants

    init(lifted: Bool, landing: AuroraLanding?) {
        self.lifted = lifted; self.landing = landing
        _fade = State(initialValue: lifted ? 1 : 0)
    }

    private var lightRuns: Bool { !lifted && !reduceMotion && landing != nil && AuroraLanding.progress(since: landing?.start) < 1 }

    func body(content: Content) -> some View {
        TimelineView(.animation(paused: !lit)) { ctx in
            let p = reduceMotion || done ? 1 : AuroraLanding.progress(since: landing?.start, at: ctx.date)
            let t = p * Double(C.LAND_AURORA_MS)   // ms since the landing frame
            let glass = lifted ? 1 : (landing == nil || reduceMotion ? fade : 1 - Self.easeIn(min(t / Double(C.LAND_GLASS_MS), 1)))
            let hairline = lifted ? 0 : (landing == nil || reduceMotion ? 1 - fade : Self.window(t, from: C.LAND_HAIRLINE_AT_MS, for: C.LAND_HAIRLINE_MS))
            let settle = lifted || landing == nil || reduceMotion ? 0 : CGFloat(C.LAND_SETTLE_PT) * Self.window(t, from: C.LAND_SETTLE_AT_MS, for: C.LAND_SETTLE_MS)
            content
                .padding(.horizontal, WhooshSpacing.s16).padding(.vertical, WhooshSpacing.s14)
                .background {
                    let shape = RoundedRectangle(cornerRadius: L.cardRadius, style: .continuous)
                    // Flat (hid-compose round 18, board cards-s3 "what has gone lies flat like every message"; main111 sent-1280-light drew
                    // it in the blue `mine` tint): no fill, only the hairline edge the thread's messages have. Lifted: the frost, faded out on landing.
                    shape.strokeBorder(WhooshPalette.separator, lineWidth: WhooshMetrics.separator).opacity(hairline)
                    shape.fill(.clear).whooshLift(in: shape).opacity(glass)
                }
                .offset(y: settle)
                .auroraLanding(landing)
                .onChange(of: p >= 1) { _, done in if done { lit = false } }
        }
        .onAppear { lit = lightRuns }
        .onChange(of: landing?.id) { _, _ in lit = lightRuns; done = false }
        .task(id: landing?.id) {
            guard landing != nil else { return }
            try? await Task.sleep(for: .milliseconds(C.LAND_AURORA_MS + C.LAND_STATE_MS))
            done = true; lit = false
        }
        .onChange(of: lifted) { _, now in
            // A send undone or a draft reopened: glass at once (yours again on this frame). Landed with no light
            // (reduced motion, or a card that landed off screen): one cross-fade.
            if now { fade = 1; done = false } else { withAnimation(.easeOut(duration: Double(C.LAND_STATE_MS) / 1000)) { fade = 0 } }
        }
    }

    private static func easeIn(_ x: Double) -> Double { x * x }
    private static func easeOut(_ x: Double) -> Double { 1 - (1 - x) * (1 - x) }
    /// 0 before `from` ms, 1 after `from + for` ms, eased between.
    private static func window(_ t: Double, from: Int, for ms: Int) -> Double {
        easeOut(min(max((t - Double(from)) / Double(ms), 0), 1))
    }
}

extension View {
    /// Lifted glass while still yours; lands flat as a `mine` block once it is on the record, lit by `landing`.
    func threadLift(_ lifted: Bool, landing: AuroraLanding? = nil) -> some View { modifier(ThreadLift(lifted: lifted, landing: landing)) }
}
