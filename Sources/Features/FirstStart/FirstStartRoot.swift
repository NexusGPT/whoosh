import SwiftUI

/// What the main window's overlay slot draws while the first-start flow is registered: the slot's other overlays (the palette over
/// the trainer's hint) stay, and the flow draws above them. With the flow idle it adds nothing and every click goes through.
struct FirstStartOverlayHost: View {
    var body: some View {
        ZStack {
            PaletteMacOverlay()
            FirstStartRoot()
        }
    }
}

/// The flow over the real three-pane window, shown empty behind a veil (1a). Layers, bottom to top: the veils (one per pane, so each
/// step can light the pane it teaches), the stage art of the lit pane (2b), the coach card or the sheet, the medallion (3a), the ring (step 10).
struct FirstStartRoot: View {
    @State private var model = FirstStartModel.shared
    @State private var app = AppModel.shared
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        GeometryReader { g in
            let geo = FirstStartGeometry(size: g.size)
            ZStack(alignment: .topLeading) {
                if model.visible {
                    FirstStartVeils(model: model, geo: geo)
                    FirstStartBackArt(model: model, geo: geo)
                    cardLayer(geo)
                    FirstStartFrontArt(model: model, geo: geo)
                    if model.skipped { FirstStartEmptyInbox(model: model, geo: geo) }
                    if model.ringShowing { FirstStartRing(model: model, size: g.size).id(model.ringId) }
                }
            }
            .frame(width: g.size.width, height: g.size.height, alignment: .topLeading)
            .onAppear { model.windowSize = g.size }
            .onChange(of: g.size) { _, now in model.windowSize = now }
        }
        .onAppear { model.evaluateLaunch() }
        .onChange(of: app.surface) { _, _ in if model.active { model.accountsMayHaveChanged() } }
        .accessibilityIdentifier(FirstStartFeature.X)
    }

    @ViewBuilder private func cardLayer(_ geo: FirstStartGeometry) -> some View {
        let still = reduceMotion || model.frozen
        ZStack(alignment: .topLeading) {
            if model.active && !model.skipped {
                if model.step <= 5 {
                    FirstStartCardShell(model: model, geo: geo).transition(.opacity)
                } else if model.step == FirstStartModel.addStep {
                    FirstStartSheet(model: model, geo: geo, step: 6).transition(FirstStartSheetMotion.transition(still))
                } else if model.step == FirstStartModel.aiStep {
                    FirstStartAICard(model: model, geo: geo).transition(.opacity)
                } else if model.step == FirstStartModel.repliesStep {
                    FirstStartRepliesCard(model: model, geo: geo).transition(.opacity)
                } else if model.step == FirstStartModel.importStep {
                    FirstStartSheet(model: model, geo: geo, step: FirstStartModel.importStep).transition(FirstStartSheetMotion.transition(still))
                }
            }
        }
        .animation(still ? nil : .easeOut(duration: FirstStartLook.popoverS), value: model.step)
    }
}

/// One veil per pane: a lit pane's veil fades out in MOTION_MAX_MS, so the pane a step teaches comes up at full contrast (2b `setVeils`).
struct FirstStartVeils: View {
    let model: FirstStartModel
    let geo: FirstStartGeometry
    @Environment(\.colorScheme) private var scheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var shown = false

    var body: some View {
        let lit = FirstStartGeometry.lit(step: model.step, lifted: model.lifted)
        let still = reduceMotion || model.frozen
        ZStack(alignment: .topLeading) {
            ForEach(FirstStartGeometry.paneNames, id: \.self) { name in
                let r = geo.pane(name)
                let on = !lit.contains(name) && shown
                Rectangle().fill(FirstStartLook.veil.color(scheme))
                    .frame(width: r.width, height: r.height)
                    .offset(x: r.minX, y: r.minY)
                    .opacity(on ? 1 : 0)
                    .contentShape(Rectangle())
                    .allowsHitTesting(!model.lifted)   // a lit pane's veil is clear but still takes the click: nothing behind the flow is touched
                    .onTapGesture {}
                    .animation(still ? nil : .timingCurve(0.2, 0.8, 0.2, 1, duration: FirstStartLook.veilS), value: on)
                    .whooshAx("\(FirstStartFeature.X).veil.\(name)")
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .onAppear { shown = true }   // 3a: the veil fades in over 200 ms when the flow opens
    }
}

enum FirstStartSheetMotion {
    /// the sheet fades in over POPOVER_MS and scales 0.96 to 1 from its top edge (the Add account sheet's own entrance)
    static func transition(_ still: Bool) -> AnyTransition {
        still ? .opacity : .opacity.combined(with: .scale(scale: CGFloat(WhooshConstants.POPOVER_SCALE_FROM), anchor: .top))
    }
}
