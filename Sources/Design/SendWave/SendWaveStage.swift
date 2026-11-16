#if os(macOS)
import AppKit
import QuartzCore

/// The one wave across the whole app. A send fires it; on the next display frame it takes its origin on screen
/// (the send button's round end), asks every open Whoosh window for its picture (off the main thread), and shows
/// each window's slice on a glass over it, all on one clock from the first picture that lands. The send itself is
/// already committed: nothing here runs on the key's own turn, and a picture that is slow only starts the wave late.
@MainActor
final class SendWaveStage {
    static let shared = SendWaveStage()

    private var pane = ""
    private var id = 0
    private var meter: SendWaveMeter?
    private var monitor: Any?
    private var start: Date?
    private var pool: [ObjectIdentifier: SendWaveOverlay] = [:]
    private var showing: [SendWaveOverlay] = []
    private let recorder = SendWaveRecorder()

    func fire(pane: String, button: String, paneFrame: CGRect, window: NSWindow?, reduceMotion: Bool) {
        let triggered = CACurrentMediaTime()
        if meter != nil { finish("a new send") }
        guard let id = SendWaveCenter.shared.start(pane: pane, origin: .zero, reduceMotion: reduceMotion) else { return }
        self.pane = pane; self.id = id
        guard let w = window else { SendWaveCenter.shared.end(pane, id: id, interrupted: "no window"); return }
        var began = false
        let m = SendWaveMeter(pane: pane, id: id, triggered: triggered, onTick: { [weak self] in
            guard let self else { return }
            if !began { began = true; self.begin(from: w, button: button, paneFrame: paneFrame); return }
            self.recorder.tick(self.showing)
            // a picture that never lands never holds anything: the run gives up after its own length
            if self.start == nil, CACurrentMediaTime() - triggered > SendWaveTuning.durationS { self.finish("no picture") }
        }, onEnd: { [weak self] in self?.finish(nil) })
        meter = m
        m.start(in: w)
        monitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .leftMouseDown, .rightMouseDown]) { [weak self] e in
            MainActor.assumeIsolated { self?.finish("input") }
            return e
        }
    }

    func interrupt(_ why: String, pane: String) { if self.pane == pane, meter != nil { finish(why) } }

    /// The first display frame after the send: origin on screen, every window's picture asked for.
    private func begin(from source: NSWindow, button: String, paneFrame: CGRect) {
        let (origin, hole) = SendWaveGeometry.origin(button: button, pane: paneFrame, in: source)
        let targets = Self.windows()
        let reach = SendWaveGeometry.reach(from: origin, over: targets.map(\.frame))
        SendWaveCenter.shared.update(pane, id: id) { r in
            r.origin = origin; r.hole = hole; r.windows = targets.count
        }
        let (pane, id) = (self.pane, self.id)
        for t in targets {
            let o = pool[ObjectIdentifier(t)] ?? SendWaveOverlay()
            pool[ObjectIdentifier(t)] = o
            o.prepare(over: t, origin: origin, hole: t === source ? hole : .zero, reach: reach)
            let req = SendWaveSnapshot.request(t)
            Task.detached(priority: .userInitiated) { [weak self] in
                let shot = SendWaveSnapshot.capture(req)
                await self?.landed(shot, on: o, window: t, pane: pane, id: id)
            }
        }
    }

    private func landed(_ shot: SendWaveSnapshot.Shot?, on o: SendWaveOverlay, window: NSWindow, pane: String, id: Int) {
        guard pane == self.pane, id == self.id, SendWaveCenter.shared.isRunning(pane, id: id),
              let s = shot ?? SendWaveSnapshot.drawn(window) else { return }
        let first = start == nil
        if first {
            start = Date()
            meter?.begin(at: CACurrentMediaTime())
            SendWaveCenter.shared.update(pane, id: id) { r in r.snapshotMs = s.ms; r.snapshotSource = s.source; r.started = CACurrentMediaTime() }
        }
        o.show(s.image, start: start ?? Date())
        showing.append(o)
    }

    private func finish(_ reason: String?) {
        let wasRunning = meter != nil
        SendWaveCenter.shared.end(pane, id: id, interrupted: reason)
        meter?.stop(); meter = nil
        if wasRunning, let r = SendWaveCenter.shared.runs[pane], r.id == id { SendWavePerfLog.write(r) }
        if let m = monitor { NSEvent.removeMonitor(m) }; monitor = nil
        showing.forEach { $0.hide() }; showing = []
        start = nil
        recorder.flush()
    }

    /// Every Whoosh window a person can see now (the main window, a pop-out composer, Settings, the notch panel),
    /// never the glass itself nor the menu bar item. In the hidden test rig (WHOOSH_RIG_HIDDEN=1) no window is on
    /// screen, so visibility there means ordered in.
    private static func windows() -> [NSWindow] {
        let hidden = HiddenRig.isOn   // every window ordered in at alpha 0 (Sources/App/HiddenRig.swift)
        return NSApp.windows.filter { w in
            !(w is SendWaveOverlay.Panel) && !String(describing: type(of: w)).contains("StatusBar") && w.isVisible
                && w.frame.width > 1 && w.frame.height > 1
                && (hidden || (w.alphaValue > 0 && w.occlusionState.contains(.visible)))
        }
    }
}
#endif
