import SwiftUI
import QuartzCore
#if os(macOS)
import AppKit
#endif

/// The landing light (owner 00:35: "Lifted until it lands but with amazing animation that combines the aurora effect
/// to landing"; board thread-cards-r2/cards-s3, notes "The landing animation"). When a sent card lands, a smaller
/// Aurora ring expands from its countdown ring across the card only, over LAND_AURORA_MS, LAND_BAND of the card's
/// diagonal, fading over its last 40 %: the send wave's own shader (lens, channel split, rings, edge highlights) on
/// the card's own pixels, so it is clipped to the card's corners by the card's alpha. The state has ALREADY changed
/// when it starts ("Envoyé" on the landing frame): it never holds a key, never takes a click, and reduced motion
/// draws nothing (the caller cross-fades). The glass clearing belongs to the caller (quickreply's ThreadLift): it
/// reads `progress` on the same clock.
public struct AuroraLanding: Equatable, Sendable {
    /// The landing's identity (the outbox entry that landed): a new id plays a new landing.
    public let id: String
    /// The landing frame's moment.
    public let start: Date
    /// The countdown ring's centre, in the card's own points (origin top-left).
    public let origin: CGPoint

    public init(id: String, start: Date = Date(), origin: CGPoint) {
        self.id = id; self.start = start; self.origin = origin
    }

    /// The light's run, 0 at the landing frame to 1 at LAND_AURORA_MS; 1 at once under reduced motion. ThreadLift
    /// clears the glass behind the front by this number, so the two can never drift apart.
    public static func progress(since start: Date?, at now: Date = Date(), reduceMotion: Bool = false) -> Double {
        guard let start, !reduceMotion, !SendWaveCenter.reduceMotionOverride else { return 1 }
        return min(max(now.timeIntervalSince(start) / SendWaveTuning.landingDurationS, 0), 1)
    }

    /// The light's length in seconds (LAND_AURORA_MS).
    public static var duration: Double { SendWaveTuning.landingDurationS }
}

public extension View {
    /// Plays `landing` over this view (a sent card) when its id is new; nil draws nothing.
    func auroraLanding(_ landing: AuroraLanding?) -> some View { modifier(AuroraLandingModifier(landing: landing)) }
}

struct AuroraLandingModifier: ViewModifier {
    let landing: AuroraLanding?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @Environment(\.colorScheme) private var scheme
    @State private var run = AuroraLandingRun()

    func body(content: Content) -> some View {
        // One structure in every state (no branch), so the card keeps its identity when the light starts and ends.
        let on = landing != nil && run.active == landing?.id && !reduceMotion
        let l = landing
        let dark = scheme == .dark
        TimelineView(.animation(paused: !on)) { ctx in
            content.visualEffect { effect, geo in
                effect.layerEffect(
                    SendWaveShader.make(t: AuroraLanding.progress(since: l?.start, at: ctx.date), size: geo.size, origin: l?.origin ?? .zero,
                                        dark: dark, band: SendWaveTuning.landingBand, fadeFrom: SendWaveTuning.landingFadeFrom),
                    maxSampleOffset: SendWaveShader.maxOffset, isEnabled: on)
            }
        }
        #if os(macOS)
        .background { SendWaveWindowReader { run.window = $0 }.frame(width: 0, height: 0) }
        #endif
        .onChange(of: landing?.id) { _, id in
            guard let l = landing, id != nil else { run.stop(); return }
            run.play(l, reduceMotion: reduceMotion)
        }
        .onAppear { if let l = landing, AuroraLanding.progress(since: l.start) < 1 { run.play(l, reduceMotion: reduceMotion) } }
        .onDisappear { run.stop() }
        .task { await SendWaveShader.warm() }
    }
}

/// One card's landing: which landing is drawn, its frame meter, and its perf line (kind "landing").
@MainActor @Observable
final class AuroraLandingRun {
    private(set) var active: String?
    #if os(macOS)
    @ObservationIgnored weak var window: NSWindow?
    @ObservationIgnored private var meter: SendWaveMeter?
    #endif
    @ObservationIgnored private var pane = ""
    @ObservationIgnored private var runId = 0

    func play(_ l: AuroraLanding, reduceMotion: Bool) {
        stop()
        pane = "landing:" + l.id
        guard let id = SendWaveCenter.shared.start(pane: pane, origin: l.origin, reduceMotion: reduceMotion, kind: "landing") else { return }
        runId = id
        active = l.id
        #if os(macOS)
        guard let w = window else { return }   // no window yet: it draws, unmeasured and unlogged
        let m = SendWaveMeter(pane: pane, id: id, triggered: CACurrentMediaTime(), duration: SendWaveTuning.landingDurationS,
                              onTick: {}, onEnd: { [weak self] in self?.finish(nil) })
        meter = m
        m.start(in: w)
        m.begin(at: CACurrentMediaTime())
        #endif
    }

    func stop() { if active != nil { finish("the landing was replaced") } }

    private func finish(_ reason: String?) {
        #if os(macOS)
        let wasMeasured = meter != nil
        #else
        let wasMeasured = false   // no display-link meter off the Mac
        #endif
        SendWaveCenter.shared.end(pane, id: runId, interrupted: reason)
        #if os(macOS)
        meter?.stop(); meter = nil
        #endif
        active = nil
        if wasMeasured, let r = SendWaveCenter.shared.runs[pane], r.id == runId { SendWavePerfLog.write(r) }
    }
}
