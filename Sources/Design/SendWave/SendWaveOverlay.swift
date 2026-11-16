#if os(macOS)
import AppKit
import SwiftUI

/// The glass over one Whoosh window while a wave crosses it: a borderless, click-through panel sized to the window's
/// frame and ordered just above it as its child, so it draws over the title band, the toolbar and the content alike
/// while the window's own edges never move. It shows the window's picture under the shader; where the halo does
/// nothing it is transparent and the live window shows through. Pooled per window, hidden between waves.
@MainActor
final class SendWaveOverlay {
    final class Panel: NSPanel {
        override var canBecomeKey: Bool { false }
        override var canBecomeMain: Bool { false }
    }

    @MainActor @Observable
    final class Model {
        var shot: CGImage?
        var start: Date?
        var size: CGSize = .zero
        var origin: CGPoint = .zero
        var hole: CGRect = .zero
        var reach: CGFloat = 1
        var dark = false
    }

    let panel: Panel
    let model = Model()
    private weak var target: NSWindow?
    /// The window it covers, by title (the recorder names its frames with it).
    var title: String { target?.title ?? "" }

    init() {
        panel = Panel(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: true)
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.ignoresMouseEvents = true
        panel.isReleasedWhenClosed = false
        panel.collectionBehavior = [.transient, .ignoresCycle, .fullScreenAuxiliary]
        // the hidden test rig: every window of the process is invisible, the glass included; the wave still runs
        // and the recorder renders it on its own
        if HiddenRig.isOn { panel.alphaValue = 0 }
        panel.contentView = NSHostingView(rootView: SendWaveOverlayView(model: model))
    }

    /// Places the glass over `window` with the wave's numbers in that window's own points; shows nothing yet.
    func prepare(over window: NSWindow, origin: CGPoint, hole: CGRect, reach: CGFloat) {
        target = window
        let f = window.frame
        model.size = f.size
        model.origin = SendWaveGeometry.local(origin, in: f)
        model.hole = SendWaveGeometry.local(hole, in: f)
        model.reach = reach
        model.dark = window.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
        panel.setFrame(f, display: false)
    }

    /// The window's picture landed: the glass appears on the wave's shared clock.
    func show(_ shot: CGImage, start: Date) {
        guard let target, target.isVisible else { return }
        model.shot = shot
        model.start = start
        // Attached as the window's child only: it is ordered exactly where its window is (in the hidden test rig,
        // off the owner's screen with it) and is never ordered front on its own, never key, never activates the app.
        if panel.parent !== target { panel.parent?.removeChildWindow(panel); target.addChildWindow(panel, ordered: .above) }
    }

    func hide() {
        model.shot = nil
        model.start = nil
        panel.parent?.removeChildWindow(panel)
        panel.orderOut(nil)
    }
}

/// The drawing on the glass: the picture under the one shader, redrawn every display frame for SENDWAVE_MS.
struct SendWaveOverlayView: View {
    let model: SendWaveOverlay.Model

    var body: some View {
        if let shot = model.shot, let start = model.start, model.size.width > 0 {
            TimelineView(.animation) { ctx in
                let t = min(max(ctx.date.timeIntervalSince(start) / SendWaveTuning.durationS, 0), 1)
                Image(decorative: shot, scale: CGFloat(shot.width) / model.size.width)
                    .resizable()
                    .frame(width: model.size.width, height: model.size.height)
                    .layerEffect(SendWaveShader.make(t: t, size: model.size, origin: model.origin, hole: model.hole,
                                                     dark: model.dark, reach: model.reach),
                                 maxSampleOffset: SendWaveShader.maxOffset)
            }
            .frame(width: model.size.width, height: model.size.height)
            .allowsHitTesting(false)
        }
    }
}
#endif
