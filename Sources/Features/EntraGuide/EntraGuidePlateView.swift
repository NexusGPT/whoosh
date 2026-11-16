import SwiftUI

/// The plate: a camera over the screenshot with the ring on the control. Only transform and opacity move: the camera pans and zooms in
/// 260 ms on the same picture; a new picture fades in at its destination (MOTION rule 11); a short ring move slides.
struct EntraGuidePlateView: View {
    let model: EntraGuideModel
    @Environment(\.accessibilityReduceMotion) private var reduce
    @State private var cams: [String: EntraGuideCamera] = [:]
    @State private var active = ""
    @State private var recent: [String] = []
    @State private var movesCamera = false
    @State private var ring = CGRect.zero
    @State private var ringSlides = false
    @State private var ringToken = 0

    var body: some View {
        GeometryReader { g in
            ZStack(alignment: .topLeading) {
                Rectangle().fill(WhooshPalette.guidePaper)
                ForEach(recent, id: \.self) { id in
                    if let p = model.data.plate(id) {
                        EntraGuideLayer(plate: p, cam: cams[id], size: g.size, active: active == id, movesCamera: movesCamera,
                                        covers: active == id ? (model.step.cover ?? []) : [], reduce: reduce)
                    }
                }
                EntraGuideRingView(box: ring).id(ringToken)
                    .animation(ringSlides ? EntraGuideMetrics.motion(EntraGuideMetrics.ease(EntraGuideMetrics.slideS), reduce) : nil, value: ring)
                EntraGuideCaptionView(text: model.step.cap, ring: ring, plate: g.size).id("cap\(ringToken)-\(model.index)")
                if model.data.plate(active)?.isDrawn == true { drawnFlag }
            }
            .clipped()
            .onAppear { place(g.size, animate: false) }
            .onChange(of: model.index) { _, _ in place(g.size, animate: true) }
            .onChange(of: g.size) { _, s in place(s, animate: false) }
        }
        .accessibilityElement(children: .contain).accessibilityIdentifier("\(EntraGuideWords.surface).plate")
    }

    private var drawnFlag: some View {
        Text(EntraGuideWords.drawn).font(.system(size: 11, weight: .semibold)).foregroundStyle(WhooshPalette.warning)
            .padding(.horizontal, 9).padding(.vertical, 5).background(WhooshPalette.fill, in: Capsule())
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topTrailing).padding(8).allowsHitTesting(false)
            .accessibilityIdentifier("\(EntraGuideWords.surface).drawn-flag")
    }

    /// Aims the camera and the ring at the current step's target. Same picture and a short ring move: both slide; otherwise the ring is new.
    private func place(_ size: CGSize, animate: Bool) {
        let step = model.step
        guard let p = model.data.plate(step.plate), size.width > 0 else { return }
        let cam = EntraGuideCamera.fit(w: p.w, h: p.h, focus: step.focus, in: size)
        let box = cam.box(ring: step.ring, w: p.w, h: p.h, pad: Double(EntraGuideMetrics.ringPad))
        let same = animate && active == p.id
        let near = same && abs(Double(ring.minY - box.minY)) < EntraGuideMetrics.slideBelow
        movesCamera = same
        cams[p.id] = cam
        if !recent.contains(p.id) { recent = Array((recent + [p.id]).suffix(2)) }
        active = p.id
        ringSlides = near
        if !near { ringToken += 1 }
        ring = box
        if !animate { ringSlides = false }
        EntraGuideImages.shared.warm(model.upcomingFiles)
    }
}

/// One picture on the camera. Its base frame is the plate's width; the camera scales it from the top left and moves it.
struct EntraGuideLayer: View {
    let plate: EntraGuidePlate
    let cam: EntraGuideCamera?
    let size: CGSize
    let active: Bool
    let movesCamera: Bool
    let covers: [[Double]]
    let reduce: Bool

    var body: some View {
        let base = size.width, height = size.width * plate.h / plate.w
        let rel = (cam?.k ?? 1) * plate.w / max(base, 1)
        content(base, height)
            .frame(width: base, height: height, alignment: .topLeading)
            .overlay(alignment: .topLeading) {
                ForEach(Array(covers.enumerated()), id: \.offset) { _, c in
                    if c.count == 4 {
                        Rectangle().fill(Color.white).frame(width: (c[2] - c[0]) * base, height: (c[3] - c[1]) * height)
                            .offset(x: c[0] * base, y: c[1] * height).transition(.opacity)
                    }
                }
            }
            .scaleEffect(rel, anchor: .topLeading)
            .offset(x: cam?.tx ?? 0, y: cam?.ty ?? 0)
            .animation(movesCamera ? EntraGuideMetrics.motion(EntraGuideMetrics.ease(EntraGuideMetrics.cameraS), reduce) : nil, value: cam)
            .opacity(active ? 1 : 0)
            .animation(EntraGuideMetrics.motion(.easeInOut(duration: EntraGuideMetrics.layerFadeS), reduce), value: active)
            .animation(.easeOut(duration: EntraGuideMetrics.fadeS), value: covers.count)
            .zIndex(active ? 2 : 1)
            .allowsHitTesting(false)
    }

    @ViewBuilder private func content(_ w: CGFloat, _ h: CGFloat) -> some View {
        if let file = plate.file, let img = EntraGuideImages.shared.image(file) {
            Image(decorative: img, scale: 1).resizable().interpolation(.high).frame(width: w, height: h)
        } else {
            Color.clear.frame(width: w, height: h)
        }
    }
}

extension EntraGuideModel {
    /// The picture files of the next two steps, decoded in the background before they are needed.
    var upcomingFiles: [String] {
        (index...min(index + 2, max(total - 1, 0))).compactMap { data.steps.indices.contains($0) ? data.plate(data.steps[$0].plate)?.file : nil }
    }
}
