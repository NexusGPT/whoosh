#if os(macOS)
import AppKit
import ImageIO
import SwiftUI
import UniformTypeIdentifiers

/// Evidence only, off unless WHOOSH_SENDWAVE_RECORD names a folder: while a wave runs, every second display frame
/// each window's slice is RENDERED here, never read from the screen: the window's picture with the same shader at
/// the same moment drawn over it (ImageRenderer), written off the main thread as `<run>-<window title>-<ms>.png`
/// (ms from the first glass shown; the title with anything but letters and digits dropped). It works with every window at alpha 0 (the hidden test rig).
@MainActor
final class SendWaveRecorder {
    private let folder = SendWaveCenter.env["WHOOSH_SENDWAVE_RECORD"].map { URL(fileURLWithPath: $0) }
    private var run = 0
    private var ticks = 0
    private var started = false

    func tick(_ glasses: [SendWaveOverlay]) {
        guard let folder, !glasses.isEmpty else { return }
        ticks += 1
        guard ticks % 2 == 1 else { return }
        if !started { started = true; run += 1; try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true) }
        for (i, g) in glasses.enumerated() {
            let m = g.model
            guard let shot = m.shot, let start = m.start, m.size.width > 0 else { continue }
            let elapsed = Date().timeIntervalSince(start)
            let t = min(max(elapsed / SendWaveTuning.durationS, 0), 1)
            let frame = ZStack {
                Image(decorative: shot, scale: CGFloat(shot.width) / m.size.width).resizable()
                Image(decorative: shot, scale: CGFloat(shot.width) / m.size.width).resizable()
                    .layerEffect(SendWaveShader.make(t: t, size: m.size, origin: m.origin, hole: m.hole, dark: m.dark, reach: m.reach),
                                 maxSampleOffset: SendWaveShader.maxOffset)
            }
            .frame(width: m.size.width, height: m.size.height)
            let renderer = ImageRenderer(content: frame)
            renderer.scale = CGFloat(shot.width) / m.size.width
            guard let img = renderer.cgImage else { continue }
            let title = String((g.title.isEmpty ? "w\(i)" : g.title).filter { $0.isLetter || $0.isNumber })
            let url = folder.appendingPathComponent(String(format: "%02d-%@-%04d.png", run, title, Int((elapsed * 1000).rounded())))
            let box = SendWaveImageBox(img)
            Task.detached(priority: .utility) {
                guard let dest = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil) else { return }
                CGImageDestinationAddImage(dest, box.image, nil)
                CGImageDestinationFinalize(dest)
            }
        }
    }

    func flush() { ticks = 0; started = false }
}

/// A finished frame handed to the writer thread (CGImage is immutable).
struct SendWaveImageBox: @unchecked Sendable { let image: CGImage; init(_ i: CGImage) { image = i } }
#endif
