import SwiftUI
import Observation

/// Round 2 of doors-nav (probes-r1 round 4, the shared title-bar probe): a window that draws its content under a transparent title bar
/// must start every control below the bar's REAL height (`frame.height - contentLayoutRect.height`: 28 or 52 pt by system, 0 in full
/// screen). No typed number for the band: an anchor measures where its pane really starts in the window and how far the pane's first
/// control sits below that, and the pane takes the missing points as extra top room. A pane that already starts clear of the band
/// gets 0 and nothing moves (the owner's design is untouched).
@MainActor @Observable
final class TitleBandClearance {
    static let shared = TitleBandClearance()
    private(set) var extras: [String: CGFloat] = [:]

    /// Extra top room, in points, the pane `key` needs to put its first control below the band.
    func extra(_ key: String) -> CGFloat { extras[key] ?? 0 }

    func set(_ key: String, _ points: CGFloat) {
        if abs(points - extra(key)) > 0.5 { extras[key] = points }
    }

    /// The missing points: the band's bottom minus where the pane's first control sits (pane top plus its offset in the pane).
    nonisolated static func missing(band: CGFloat, paneTop: CGFloat, firstControl: CGFloat) -> CGFloat {
        max(0, band - (paneTop + firstControl))
    }
}

#if os(macOS)
import AppKit

/// Sits at the top of a pane (zero drawing). `firstControl` is the distance from the pane's top to its first control's top.
struct TitleBandAnchor: NSViewRepresentable {
    let key: String
    let firstControl: CGFloat

    func makeNSView(context: Context) -> Anchor { Anchor(key: key, firstControl: firstControl) }
    func updateNSView(_ v: Anchor, context: Context) { v.firstControl = firstControl; v.measure() }

    final class Anchor: NSView {
        let key: String
        var firstControl: CGFloat
        init(key: String, firstControl: CGFloat) {
            self.key = key
            self.firstControl = firstControl
            super.init(frame: .zero)
        }
        @available(*, unavailable) required init?(coder: NSCoder) { nil }

        override func viewDidMoveToWindow() { super.viewDidMoveToWindow(); measure() }
        /// A resize, full screen and a new window size all lay the pane out again, and the band is read again with it.
        override func layout() { super.layout(); measure() }

        func measure() {
            guard let w = window, let content = w.contentView else { return }
            let band: CGFloat = w.styleMask.contains(.fullSizeContentView) ? max(0, w.frame.height - w.contentLayoutRect.height) : 0
            let r = convert(bounds, to: content)
            let top = content.isFlipped ? r.minY : content.bounds.height - r.maxY
            let need = TitleBandClearance.missing(band: band, paneTop: top, firstControl: firstControl)
            let key = self.key
            DispatchQueue.main.async { MainActor.assumeIsolated { TitleBandClearance.shared.set(key, need) } }
        }
    }
}
#endif

/// `rows mac-main.controls` (the shared title-bar case reads `<surface>.controls`): the controls the window draws inside its content, by
/// accessibility id. The toolbar and the sidebar toggle are not listed: they are placed ON the title bar's own line, above it, as frame-view
/// subviews (MacToolbarHost, SidebarToggleHost), by design and visible there; the traffic lights are the window's.
@MainActor
enum MacShellControls {
    static func rows(_ m: AppModel) -> [ControlRow] {
        var out: [ControlRow] = [
            ["ax": "mac-main.sidebar.header.mailboxes", "pane": "sidebar", "kind": "header"],
            ["ax": "mac-main.sidebar.mailboxes.pick", "pane": "sidebar", "kind": "button"],
            ["ax": "mac-main.list.command", "pane": "list", "kind": "field"],
        ]
        for i in m.tabIds.indices { out.append(["ax": "mac-main.splits.\(i)", "pane": "list", "kind": "tab"]) }
        return out
    }
}
