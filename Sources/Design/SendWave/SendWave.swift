import SwiftUI
#if os(macOS)
import AppKit
#endif

public extension View {
    /// The send shockwave for this pane's sends (docs/PRD.md "## Send shockwave"). `sent` is the pane's live send
    /// (an outbox id, nil when nothing is sending): the moment it becomes a new id the send has ALREADY happened on
    /// the key frame, and on the next display frame the wave starts from the send button marked `.whooshAx(origin)`.
    /// It spreads across the whole app (owner, 2026-11-08: "like the whole app"): every open Whoosh window, edge to
    /// edge with its title band, draws the part of the one wave that crosses it, while the windows themselves stay
    /// still. It never takes a click or a key, reduced motion turns it off, and any key or click in the app, or the
    /// send leaving its state (undo), ends it at once.
    func sendWave(pane: String, origin: String, on sent: String?) -> some View {
        modifier(SendWaveModifier(pane: pane, origin: origin, sent: sent))
    }
}

struct SendWaveModifier: ViewModifier {
    let pane: String
    let origin: String
    let sent: String?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var frame: CGRect = .zero
    @State private var window = SendWaveWindowBox()

    func body(content: Content) -> some View {
        content
            .background {
                GeometryReader { g in
                    Color.clear
                        .onAppear { frame = g.frame(in: .global) }
                        .onChange(of: g.frame(in: .global)) { _, r in frame = r }
                }
                #if os(macOS)
                SendWaveWindowReader { window.value = $0 }.frame(width: 0, height: 0)
                #endif
            }
            .onChange(of: sent) { old, new in
                #if os(macOS)
                guard let new, new != old else {
                    if new == nil, old != nil { SendWaveStage.shared.interrupt("the send left its state", pane: pane) }
                    return
                }
                SendWaveStage.shared.fire(pane: pane, button: origin, paneFrame: frame, window: window.value, reduceMotion: reduceMotion)
                #endif
            }
            #if os(macOS)
            .onDisappear { SendWaveStage.shared.interrupt("the pane closed", pane: pane) }
            .task { await SendWaveShader.warm() }
            #endif
    }
}

/// The pane's window, kept outside SwiftUI's diffing (a reference the reader fills).
final class SendWaveWindowBox {
    #if os(macOS)
    weak var value: NSWindow?
    #endif
}
