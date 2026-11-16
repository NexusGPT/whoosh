import SwiftUI

/// The sidebar toggle: the button Apple Mail draws beside the traffic lights, in both states (shown: over the sidebar's top;
/// hidden: over the list's top, in the same spot, so it never moves). `sidebar.left` glyph, tooltip with the key.
struct SidebarToggleButton: View {
    @State private var shell = MacShellState.shared
    static let size = CGSize(width: WhooshMetrics.actionIcon + WhooshSpacing.s12, height: WhooshMetrics.actionIcon + WhooshSpacing.s8)

    var body: some View {
        let _ = shell.sidebarCollapsed
        Button { SidebarVisibility.toggle() } label: {
            Image(systemName: "sidebar.left").font(.system(size: WhooshMetrics.actionIcon - 1, weight: .regular))
                .foregroundStyle(WhooshPalette.sidebarSecondaryLabel)
                .frame(width: Self.size.width, height: Self.size.height)
                .contentShape(Rectangle())
        }
        .buttonStyle(SidebarPressStyle())
        .help(SidebarVisibility.tooltip)
        .accessibilityLabel(Words.text(shell.sidebarCollapsed ? "Show sidebar" : "Hide sidebar"))
        .whooshAx("mac-main.sidebar.toggle")   // measurable at request time (FrameProbe anchor), not only when it was placed
        .ignoresSafeArea()
    }
}

#if os(macOS)
import AppKit

/// Puts the toggle into the window's frame view (the layer above the split), just right of the zoom button. The split item that
/// collapses never holds it, so it stays put while the sidebar slides. One per window; found again after a rebuild.
@MainActor
final class SidebarToggleHost {
    static let shared = SidebarToggleHost()
    static let axId = "mac-main.sidebar.toggle"
    private var host: NSHostingView<SidebarToggleButton>?
    /// The lights' line: half the titlebar's height below the window top (the traffic lights are centred in the bar). The zoom button's
    /// own frame is NOT used for y: on macOS 26 it reports a position about 16 pt below where the lights are drawn, which put the
    /// toggle a row under them (owner 2026-11-08 12:37, confirmed on a real screen capture).
    private(set) var lightsLine: CGFloat = 16
    /// Distance from the lights' line to the centre of what the toggle DRAWS, in window points (0 = on the line). The drawing is
    /// scanned from the hosting view's rendered pixels, so a content offset inside the frame (a safe-area inset) shows too.
    var centreOffset: CGFloat? {
        guard let h = host, let rep = h.bitmapImageRepForCachingDisplay(in: h.bounds) else { return nil }
        h.cacheDisplay(in: h.bounds, to: rep)
        var first = -1, last = -1
        for y in 0..<rep.pixelsHigh {
            for x in 0..<rep.pixelsWide where (rep.colorAt(x: x, y: y)?.alphaComponent ?? 0) > 0.05 {
                if first < 0 { first = y }
                last = y
                break
            }
        }
        guard first >= 0 else { return nil }
        let scale = CGFloat(rep.pixelsHigh) / h.bounds.height
        let drawnFromHostTop = CGFloat(first + last + 1) / 2 / scale
        let hostTop = (h.superview?.isFlipped ?? false) ? h.frame.minY : (h.superview?.bounds.height ?? 0) - h.frame.maxY
        return hostTop + drawnFromHostTop - lightsLine
    }

    func install(in window: NSWindow) {
        guard let frameView = window.contentView?.superview, let zoom = window.standardWindowButton(.zoomButton),
              let bar = zoom.superview else { return }
        if let h = host, h.superview === frameView { place(h, zoom: zoom, bar: bar, frameView: frameView); return }
        host?.removeFromSuperview()
        let h = NSHostingView(rootView: SidebarToggleButton())
        h.setFrameSize(SidebarToggleButton.size)
        h.safeAreaRegions = []   // a hosting view under the titlebar draws its content below the bar by the bar's height otherwise
        h.sizingOptions = []
        h.autoresizingMask = [.minYMargin, .maxXMargin]   // pinned to the top-left through every resize
        frameView.addSubview(h)
        host = h
        place(h, zoom: zoom, bar: bar, frameView: frameView)
    }

    private func place(_ h: NSHostingView<SidebarToggleButton>, zoom: NSButton, bar: NSView, frameView: NSView) {
        guard let window = frameView.window else { return }
        let z = bar.convert(zoom.frame, to: frameView)
        let size = SidebarToggleButton.size
        let barHeight = window.frame.height - window.contentLayoutRect.height
        lightsLine = (barHeight > 0 ? barHeight : WhooshMetrics.sidebarTop - WhooshSpacing.s20) / 2
        // frameView may be flipped (origin top-left) or not: place by distance from the window top either way.
        let top = lightsLine - size.height / 2
        let y = frameView.isFlipped ? top : frameView.bounds.height - top - size.height
        h.frame = CGRect(x: z.maxX + WhooshSpacing.s12, y: y, width: size.width, height: size.height)
        // FrameProbe rects are window points with the origin top-left.
        FrameProbe.set(Self.axId, CGRect(x: h.frame.minX, y: top, width: size.width, height: size.height))
    }
}

/// Sits in the sidebar's background; when it reaches a window it installs the toggle there.
struct SidebarWindowAnchor: NSViewRepresentable {
    final class Anchor: NSView {
        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            guard let w = window else { return }
            DispatchQueue.main.async { MainActor.assumeIsolated { SidebarToggleHost.shared.install(in: w) } }
        }
    }
    func makeNSView(context: Context) -> Anchor { Anchor() }
    func updateNSView(_ v: Anchor, context: Context) {
        if let w = v.window { DispatchQueue.main.async { MainActor.assumeIsolated { SidebarToggleHost.shared.install(in: w) } } }
    }
}
#endif
