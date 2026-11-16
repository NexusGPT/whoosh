import SwiftUI
#if os(macOS)
import AppKit
#endif

/// `shot`: the PNG of a window, and how its pixels map to `measure` frames.
@MainActor
extension WindowBridge {
    /// What a shot wrote and how its pixels map to `measure` frames (content-view points, origin top-left):
    /// pixel = ((x + contentX) * scale, (y + contentY) * scale). A hidden rig and a live capture draw the whole
    /// window (title band included), a render of the content view does not: the mapping says which, so no check
    /// needs to know which path drew the shot.
    public struct ShotResult {
        public let source: String; public let contentX: CGFloat; public let contentY: CGFloat; public let scale: CGFloat
        /// The window the picture is of (identifier and title), so a caller can see it is the one it asked for (probes-r1, round 6).
        public var window = "", title = ""
    }

    #if os(macOS)
    /// The content view's top-left inside the window's frame (points, origin top-left): the title band's height
    /// for a titled window, zero for a full-size-content one.
    static func contentOrigin(_ content: NSView, inWindowFrameOf w: NSWindow) -> CGPoint {
        let r = content.convert(content.bounds, to: nil)   // window coordinates: origin bottom-left of the frame
        return CGPoint(x: r.minX, y: w.frame.height - r.maxY)
    }
    #endif

    /// Captures the front window (or, with `slot`, that surface's window) as the window server draws it
    /// (ShotLive, WKWebView included); falls back to rendering the view tree when the capture fails.
    #if os(macOS)
    @discardableResult
    public static func shot(to path: String, slot: SurfaceSlot? = nil, window target: NSWindow? = nil) throws -> ShotResult {
        let win: NSWindow? = if let target { target } else if let slot { window(for: slot) } else { NSApp.windows.first(where: \.isVisible) }
        guard let w = win, let content = w.contentView else { throw ControlFailure("no visible window to shoot\(slot.map { " for \($0.id)" } ?? "")") }
        func named(_ r: ShotResult) -> ShotResult { var r = r; r.window = WindowBridge.id(of: w); r.title = w.title; return r }
        // Hidden rig: the window server draws the window at alpha 0, so its live image is empty. The window's frame view
        // (title band, traffic lights and content: the live capture's geometry) is rendered from the view tree instead, with
        // each visible web view's own WebKit picture drawn over it; `source` says how many were drawn and any that were not.
        let view = HiddenRig.isOn ? (content.superview ?? content) : content
        view.layoutSubtreeIfNeeded()
        view.displayIfNeeded()
        var liveFailure = ""
        if !ShotLive.disabled && !HiddenRig.isOn {
            var why = ""
            if let png = ShotLive.png(of: w, why: &why) {
                try png.write(to: URL(fileURLWithPath: path))
                let px = CGFloat(NSBitmapImageRep(data: png)?.pixelsWide ?? 0)
                let o = contentOrigin(content, inWindowFrameOf: w)
                return named(ShotResult(source: "live", contentX: o.x, contentY: o.y, scale: px / max(w.frame.width, 1)))
            }
            liveFailure = why
        }
        // Hidden rig: cacheDisplay leaves a WKWebView's page out (blank 5 s after a load), so WebKit is asked for each visible web
        // view's own picture (WindowShotWeb.swift), waited for with the run loop turning, and drawn over the tree's picture below.
        var webTargets: [ShotWeb.Target] = []
        var webImages: [NSImage?] = []
        var webNote = ""
        if HiddenRig.isOn && !ShotWeb.disabled {
            ShotWeb.mountCoverReaders(in: view)
            webTargets = ShotWeb.targets(in: view)
            let settled = ShotWeb.settle(in: view, targets: webTargets)
            webTargets = ShotWeb.targets(in: view)   // a page that reloaded while it was waited for may have moved
            (webImages, webNote) = ShotWeb.pictures(of: webTargets, scale: w.backingScaleFactor)
            webNote = (webNote.isEmpty ? settled : settled + "; " + webNote) + "; " + MessageBodyPolicy.rulesNote
            if ShotWeb.coverNote.hasPrefix("cover frames NOT found") {   // never paste the mail over a card whose place is unknown
                webImages = webImages.map { _ in nil }
                webNote += "; web: not ready (the first-start card's frame was not found, so the page is not pasted under it)"
            }
        }
        guard let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { throw ControlFailure("the window could not be rendered") }
        view.cacheDisplay(in: view.bounds, to: rep)
        // dark-mode page came out as light text on nothing: flatten it onto the window background colour.
        guard let out = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: rep.pixelsWide, pixelsHigh: rep.pixelsHigh,
                                         bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                                         colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0),
              let ctx = NSGraphicsContext(bitmapImageRep: out) else { throw ControlFailure("the window could not be rendered") }
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = ctx
        ctx.cgContext.scaleBy(x: CGFloat(rep.pixelsWide) / view.bounds.width, y: CGFloat(rep.pixelsHigh) / view.bounds.height)
        w.effectiveAppearance.performAsCurrentDrawingAppearance {
            NSColor.windowBackgroundColor.setFill()
            NSRect(origin: .zero, size: view.bounds.size).fill()
        }
        rep.draw(in: NSRect(origin: .zero, size: view.bounds.size), from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: nil)
        ShotWeb.draw(webImages, of: webTargets, in: view, covers: HiddenRig.isOn ? ShotWeb.covers(in: view, content: content) : [],
                     veils: HiddenRig.isOn ? ShotWeb.veils(in: view, content: content, dark: w.effectiveAppearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua) : [])   // the web views' own pictures, at their places (hidden rig only: empty otherwise)
        NSGraphicsContext.restoreGraphicsState()
        guard let png = out.representation(using: .png, properties: [:]) else { throw ControlFailure("the PNG could not be encoded") }
        try png.write(to: URL(fileURLWithPath: path))
        let o = view === content ? CGPoint.zero : contentOrigin(content, inWindowFrameOf: w)
        let scale = CGFloat(rep.pixelsWide) / max(view.bounds.width, 1)
        let coverCount = HiddenRig.isOn ? ShotWeb.covers(in: view, content: content).count : 0
        let src = HiddenRig.isOn ? "render (hidden rig\(webNote.isEmpty ? "" : "; " + webNote)\(coverCount > 0 ? "; web pictures clipped around \(coverCount) cover(s)" : "")\(ShotWeb.coverNote.isEmpty ? "" : "; " + ShotWeb.coverNote))" : liveFailure.isEmpty ? "render" : "render (live capture failed: \(liveFailure))"
        // Mutant (checks/rig-hidden-check.py shot-frames-match-pixels): the mapping claims a title band the shot does not have.
        let wrongY: CGFloat = ProcessInfo.processInfo.environment["WHOOSH_SHOT_MAPPING_MUTANT"] == "title-band" ? 28 : 0
        return named(ShotResult(source: src, contentX: o.x, contentY: o.y + wrongY, scale: scale))
    }
    #else
    @discardableResult
    public static func shot(to path: String, slot: SurfaceSlot? = nil) throws -> ShotResult {
        throw ControlFailure("shot is served on the Mac only at this commit")
    }
    #endif
}
