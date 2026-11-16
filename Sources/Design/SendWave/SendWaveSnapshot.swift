import SwiftUI
import QuartzCore
#if os(macOS)
import AppKit

/// Layer (1), the base: a picture of the pane as the window server last composited it (glass, vibrancy and the
/// text view included; SwiftUI's own renderers cannot draw an AppKit text view). Taken after the send's commit,
/// OFF the main thread: the main actor only places the request and receives the finished picture.
enum SendWaveSnapshot {
    struct Shot: @unchecked Sendable { let image: CGImage; let source: String; let ms: Double }
    struct Request: Sendable { let screenRect: CGRect; let windowNumber: UInt32; let nominal: Bool }

    /// CGWindowListCreateImage is marked unavailable in the SDK but answers for a window of this process without
    /// the Screen Recording permission (Sources/App/ShotLive.swift does the same lookup).
    private typealias ListImage = @convention(c) (CGRect, UInt32, UInt32, UInt32) -> Unmanaged<CGImage>?
    private static let listImage: ListImage? = {
        guard let h = dlopen(nil, RTLD_NOW), let sym = dlsym(h, "CGWindowListCreateImage") else { return nil }
        return unsafeBitCast(sym, to: ListImage.self)
    }()

    /// The whole window, edge to edge (title band and toolbar included, its shadow left out).
    @MainActor static func request(_ window: NSWindow) -> Request {
        let f = window.frame
        let primaryTop = NSScreen.screens.first?.frame.maxY ?? f.maxY
        return Request(screenRect: CGRect(x: f.minX, y: primaryTop - f.maxY, width: f.width, height: f.height),
                       windowNumber: UInt32(window.windowNumber), nominal: SendWaveTuning.snapshotScale < 2)
    }

    /// Any thread. The window server's image is copied once into plain memory so no later frame pays for it.
    static func capture(_ r: Request) -> Shot? {
        let t0 = CACurrentMediaTime()
        // kCGWindowListOptionIncludingWindow = 8; kCGWindowImageBoundsIgnoreFraming = 1, kCGWindowImageNominalResolution = 16
        guard let f = listImage, let img = f(r.screenRect, 8, r.windowNumber, r.nominal ? 17 : 1)?.takeRetainedValue(),
              img.width > 0, let owned = copy(img), !blank(owned) else { return nil }
        return Shot(image: owned, source: "window", ms: (CACurrentMediaTime() - t0) * 1000)
    }

    /// A window at alpha 0 (the hidden test rig) gives the window server nothing to show: the window's own drawing
    /// of its whole frame view (title band included) stands in, on the main thread.
    @MainActor static func drawn(_ window: NSWindow) -> Shot? {
        let t0 = CACurrentMediaTime()
        guard let view = window.contentView?.superview ?? window.contentView,
              let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { return nil }
        view.cacheDisplay(in: view.bounds, to: rep)
        guard let img = rep.cgImage else { return nil }
        return Shot(image: img, source: "drawn", ms: (CACurrentMediaTime() - t0) * 1000)
    }

    /// True when the middle of the picture is fully transparent (nothing was composited).
    private static func blank(_ img: CGImage) -> Bool {
        var px = [UInt8](repeating: 0, count: 4)
        guard let ctx = CGContext(data: &px, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
                                  space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        else { return false }
        ctx.draw(img, in: CGRect(x: -CGFloat(img.width / 2), y: -CGFloat(img.height / 2), width: CGFloat(img.width), height: CGFloat(img.height)))
        return px[3] == 0
    }

    private static func copy(_ img: CGImage) -> CGImage? {
        guard let space = img.colorSpace ?? CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: img.width, height: img.height, bitsPerComponent: 8, bytesPerRow: 0, space: space,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)
        else { return nil }
        ctx.draw(img, in: CGRect(x: 0, y: 0, width: img.width, height: img.height))
        return ctx.makeImage()
    }
}

/// The window a view sits in, for the snapshot and the frame meter.
struct SendWaveWindowReader: NSViewRepresentable {
    let found: @MainActor (NSWindow?) -> Void
    func makeNSView(context: Context) -> NSView { Probe(found) }
    func updateNSView(_ v: NSView, context: Context) {}
    private final class Probe: NSView {
        let found: @MainActor (NSWindow?) -> Void
        init(_ f: @escaping @MainActor (NSWindow?) -> Void) { found = f; super.init(frame: .zero) }
        required init?(coder: NSCoder) { nil }
        override func viewDidMoveToWindow() { super.viewDidMoveToWindow(); found(window) }
    }
}
#endif
