import Foundation
#if os(macOS)
import AppKit
#endif

/// The recorder for the one defect a still frame cannot show: the notch changing size, or flipping between two
/// states, while nobody touches it. Adapted from the owner's ghostty fork
/// (macos/Sources/Features/OneTerminal/Notch/NotchTwitch.swift, by the owner, MIT).
///
/// One line every time a watched value CHANGES, with the milliseconds since the recording was armed. A twitch
/// then has a shape a reader can name: one key alternating between two values while the pointer holds still.
/// Changes only, so a stable key costs one lookup. Nothing is written until a drive arms it
/// (`mac-notch.stage.twitch.on`), so the owner's own copy carries no cost.
@MainActor
enum NotchTwitch {
    /// Under the rig's own folder, so one run's recording is never read as another's.
    static var path: String {
        (ProcessInfo.processInfo.environment[ControlServer.socketEnv] ?? NSTemporaryDirectory()) + "/notch-twitch.log"
    }
    private(set) static var armed = false
    private static var last: [String: String] = [:]
    private static var handle: FileHandle?
    private static var origin: UInt64 = 0
    private static var sampler: Timer?

    /// Start a recording: the file is truncated and every remembered value forgotten, so each key's first line
    /// states where it started. The panel's laid-out tree is sampled faster than any beat that could move it.
    static func arm() {
        stop()
        last = [:]
        origin = DispatchTime.now().uptimeNanoseconds
        FileManager.default.createFile(atPath: path, contents: nil)
        handle = FileHandle(forWritingAtPath: path)
        armed = true
        write("0 armed")
        let t = Timer(timeInterval: 1.0 / Double(WhooshConstants.NOTCH_TWITCH_SAMPLE_HZ), repeats: true) { _ in
            MainActor.assumeIsolated { NotchTwitch.sample() }
        }
        RunLoop.main.add(t, forMode: .common)
        sampler = t
    }

    static func stop() {
        sampler?.invalidate()
        sampler = nil
        armed = false
        try? handle?.close()
        handle = nil
    }

    /// Only a change reaches the file: A, B, A, B are four lines, never buried under thousands of unchanged ones.
    static func note(_ key: String, _ value: String) {
        guard armed, last[key] != value else { return }
        last[key] = value
        write("\(ms()) \(key) \(value)")
    }

    /// A rectangle rounded to whole points: sub-point noise is not a twitch.
    static func box(_ r: CGRect) -> String {
        String(format: "%.0f,%.0f,%.0fx%.0f", r.origin.x, r.origin.y, r.size.width, r.size.height)
    }

    /// The whole recording, so a drive reads what the app wrote and not a second copy of it.
    static func read() -> String { (try? String(contentsOfFile: path, encoding: .utf8)) ?? "" }

    private static func sample() {
        let m = NotchModel.shared
        note("stage", "\(m.stage)")
        note("shape", String(format: "%.0fx%.0f", m.shapeSize.width, m.shapeSize.height))
        #if os(macOS)
        guard let w = NotchPanel.shared.window else { return }
        note("window", box(w.frame) + " vis=\(w.isVisible ? 1 : 0) key=\(w.isKeyWindow ? 1 : 0)")
        if let cv = w.contentView { note("tree", layout(cv, depth: 0)) }
        #endif
    }

    #if os(macOS)
    /// Every view in the panel, class then frame, in tree order: one string, so "did the layout change" is one
    /// comparison. Deep enough to reach the rows: a walk that stops above them reports stillness while they move.
    private static func layout(_ v: NSView, depth: Int) -> String {
        let f = v.frame
        var part = String(format: "%@@%.0f,%.0f,%.0fx%.0f", String(describing: type(of: v)), f.origin.x, f.origin.y, f.size.width, f.size.height)
        guard depth < WhooshConstants.NOTCH_TWITCH_TREE_DEPTH else { return part }
        for c in v.subviews { part += "|" + layout(c, depth: depth + 1) }
        return part
    }
    #endif

    private static func ms() -> String { String((DispatchTime.now().uptimeNanoseconds &- origin) / 1_000_000) }

    private static func write(_ line: String) {
        guard let handle, let d = (line + "\n").data(using: .utf8) else { return }
        try? handle.write(contentsOf: d)
    }
}
