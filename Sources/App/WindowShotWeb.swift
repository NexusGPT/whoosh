#if os(macOS)
import AppKit
import WebKit

/// The hidden rig's picture of the web views in a window. A hidden test copy's window is drawn at alpha 0 and the system
/// reports it occluded: `cacheDisplay` of the view tree then leaves a WKWebView's page out (it came out blank 5 s after a
/// load, with the text still in the page). WebKit's own `takeSnapshot` renders the page itself, so `WindowBridge.shot`
/// asks it for each visible web view and draws the answer over the tree's picture. Runs only when a hidden picture is
/// asked (nothing at rest). `WHOOSH_SHOT_WEB_MUTANT=off` restores the old path (checks/rig-hidden-check.py hidden-web-text).
@MainActor
enum ShotWeb {
    /// SHOT_WEB_SNAPSHOT_TIMEOUT_S: the most one web view is waited for, in seconds (not yet in docs/CONSTANTS.md: reported to the lead for the ledger).
    private static let snapshotTimeoutS: Double = 3

    static var disabled: Bool { ProcessInfo.processInfo.environment["WHOOSH_SHOT_WEB_MUTANT"] == "off" }

    /// The longest the verb waits for the app's animations to end and for the reader's page to be loaded and painted (a bound, in seconds).
    private static let settleBaseS: Double = 3
    /// The most the load scales the bound by. A check's socket must wait longer than the verb's longest answer: journeys/lib/shotwait.py reads settleBaseS, this factor,
    /// motionS and snapshotTimeoutS from THIS file (one place), so a change here moves every check's wait.
    private static let settleMaxLoadFactor: Double = 6
    /// The bound follows the machine: the base times the one-minute load per core (at least 1, at most 6), so a loaded test machine (main111: load 30 to 116 on 12 cores, the
    /// page not ready after 3 s) is waited for longer. A page that is still not ready then says `web: not ready`; a blank body is never passed.
    private static var settleTimeoutS: Double {
        var load = [Double](repeating: 0, count: 1)
        let n = getloadavg(&load, 1)
        let perCore = n > 0 ? load[0] / Double(max(ProcessInfo.processInfo.activeProcessorCount, 1)) : 1
        return settleBaseS * min(max(perCore, 1), settleMaxLoadFactor)
    }
    /// The motion cap (docs/CONSTANTS.md MOTION_MAX_MS, 200 ms) plus a frame: every response animation of the window has ended after this.
    private static let motionS: Double = 0.25

    /// True when some layer under `v` still runs a Core Animation animation.
    private static func animating(_ v: NSView) -> Bool {
        func walk(_ l: CALayer) -> Bool { (l.animationKeys() ?? []).isEmpty ? (l.sublayers ?? []).contains(where: walk) : true }
        return v.layer.map(walk) ?? false
    }

    /// Before a hidden picture: (1) the window's running animations end (the run loop turns until no layer animates, then one motion cap, so a row fading in or a theme
    /// restyle is over: main108 pictured "Outside apps" mid-fade); (2) every visible web view has finished loading, and the reader's body has painted its text
    /// (`MessageBodyModel.painted`, asked fresh), which also means a restyle after `theme` has reloaded the page in the new scheme. Returns the words for `source`:
    /// what was waited for and how long, or `web: not ready` when the bound passed first, so the check marks the tile NOT JUDGED and never passes a blank body.
    static func settle(in root: NSView, targets: [Target]) -> String {
        // The first-start flow's loops run all the time and the loop-seam checks compare two pictures one loop apart: a wait of varying length would shift the second picture's
        // phase (main111: loop-seam-step2 and step3 red). While the flow is up and not frozen the picture is taken at once.
        let flow = FirstStartModel.shared
        let flowOn = flow.active
        let flowFrozen = flow.frozen
        if flowOn && !flowFrozen { return "not settled: the first-start loops run" }
        let t0 = Date()
        RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02))
        while animating(root) && Date().timeIntervalSince(t0) < settleTimeoutS { FrameProbe.keepAlive(); RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(motionS))
        let motionMs = Int(Date().timeIntervalSince(t0) * 1000)
        guard !targets.isEmpty else { return "settled \(motionMs) ms" }
        let t1 = Date()
        func ready() -> Bool {
            if targets.contains(where: { $0.web.isLoading }) { return false }
            if let body = MessageBodyProbe.shared.current { body.refresh(); return body.painted }   // the reader's body is the web view of the main window
            return true
        }
        while !ready() && Date().timeIntervalSince(t1) < settleTimeoutS { FrameProbe.keepAlive(); RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        FrameProbe.keepAlive()   // the picture and the frame read that follow get a full linger
        let webMs = Int(Date().timeIntervalSince(t1) * 1000)
        return ready() ? "settled \(motionMs) ms, web ready after \(webMs) ms" : "settled \(motionMs) ms, web: not ready after \(webMs) ms"
    }

    /// One web view to draw: its place in the frame view (points, the frame view's own axes) and the view to ask.
    struct Target {
        let web: WKWebView
        /// The part of the web view that shows: its visible rect (already cut by every clipping ancestor, a scroll view
        /// included), in the web view's own coordinates.
        let visible: CGRect
        /// The same part in the frame view's coordinates, cut to the frame view's bounds.
        let placed: CGRect
    }

    /// What one ask answered, as plain values (the completion handler runs on the main queue; nothing but values leave it).
    private final class Answer: @unchecked Sendable {
        var done = false
        var tiff: Data?
        var why = ""
    }

    /// Every web view under `root` that shows something: not hidden (nor under a hidden view), in a window, with a visible
    /// rect that has an area inside `root`. Document order, so "web view 2 of 2" means the same view on every shot.
    static func targets(in root: NSView) -> [Target] {
        var found: [Target] = []
        func walk(_ v: NSView) {
            if let web = v as? WKWebView {
                let vis = web.visibleRect
                if !web.isHiddenOrHasHiddenAncestor, web.window != nil, vis.width > 0.5, vis.height > 0.5 {
                    let placed = web.convert(vis, to: root).intersection(root.bounds)
                    // the part asked of WebKit is the part placed (a web view reaching past the frame view is cut at both)
                    let asked = web.convert(placed, from: root).intersection(vis)
                    if !placed.isNull, !asked.isNull, asked.width > 0.5, asked.height > 0.5 {
                        found.append(Target(web: web, visible: asked, placed: web.convert(asked, to: root)))
                    }
                }
                return   // a web view's own subviews are WebKit's
            }
            for s in v.subviews { walk(s) }
        }
        walk(root)
        return found
    }

    /// Asks WebKit for a picture of each target at once and turns the main run loop until every one answered or
    /// `snapshotTimeoutS` passed since the asks (so one web view waits at most that long). Returns, per target, its picture
    /// or nil, and the words for `source`: "N web view(s) drawn" and/or "web view i of n not drawn: <why>".
    static func pictures(of targets: [Target], scale: CGFloat) -> (images: [NSImage?], note: String) {
        guard !targets.isEmpty else { return ([], "no web view") }
        var answers: [Answer] = []
        for t in targets {
            let a = Answer()
            answers.append(a)
            let cfg = WKSnapshotConfiguration()
            cfg.rect = t.visible
            cfg.snapshotWidth = NSNumber(value: Double(t.visible.width * scale))   // pixels, not points: the bitmap is drawn at the backing scale
            t.web.takeSnapshot(with: cfg) { image, error in
                a.tiff = image?.tiffRepresentation
                a.why = error.map { $0.localizedDescription } ?? (image == nil ? "no image" : "")
                a.done = true
            }
        }
        let deadline = Date().addingTimeInterval(snapshotTimeoutS)
        while answers.contains(where: { !$0.done }) && Date() < deadline {
            RunLoop.main.run(until: Date().addingTimeInterval(0.02))
        }
        var images: [NSImage?] = []
        var failed: [String] = []
        for (i, a) in answers.enumerated() {
            if a.done, let tiff = a.tiff, let img = NSImage(data: tiff) { images.append(img); continue }
            images.append(nil)
            failed.append("web view \(i + 1) of \(answers.count) not drawn: \(a.done ? (a.why.isEmpty ? "no image" : a.why) : "timeout")")
        }
        let drawn = images.compactMap { $0 }.count
        var parts: [String] = []
        if drawn > 0 { parts.append("\(drawn) web view\(drawn == 1 ? "" : "s") drawn") }
        parts.append(contentsOf: failed)
        return (images, parts.joined(separator: "; "))
    }

    /// Draws each picture at its place. The current graphics context is the bitmap's: unflipped, in the frame view's points,
    /// origin bottom-left, already scaled to pixels. `root` may be flipped or not: its y is turned to the context's.
    /// The places (in `root`'s coordinates) of the SwiftUI layers drawn ABOVE the window's content, which the picture of a web view must not paint over: the web
    /// picture is pasted last, so without this it lands on top of the first-start card and the palette (main107: the mail printed over the AI step's card).
    /// The foot strip is opaque and lies over the end of the reader, whose scrolled page passes under it (MacShellRoot: the reader keeps the strip's room as a bottom
    /// inset): main200 at 960x632 pasted the mail's last lines over the strip's keys.
    static let coverIds = ["mac-first-start.card", "mac-palette", "mac-main.foot.strip"]
    /// The frame readers of every layer that may cover a web page, mounted BEFORE the picture is rendered. They are mounted only while the probe is on, and a reader that
    /// was asked for after the render has not laid out when the page is pasted: main111 at 960x600 pasted the mail at full strength over the card and past the veil (no
    /// frame found for either), while at 1280 the same call found them by luck of timing. The first call turns the probe on and lets the run loop turn.
    /// Main114 still pasted the page over the card at 960x600: a hidden window is occluded, SwiftUI does not run its update pass for it until the window is laid out and displayed,
    /// so the readers the probe switch asked for were still not mounted after one short turn of the run loop. So the view is laid out and displayed in a loop until the frames
    /// that matter exist (the card and the veils while the first-start flow is up), bounded by `coverWaitS`; `ShotWeb.coverNote` says what was found, in the shot's `source`.
    private static let coverWaitS: Double = 3
    static var coverNote = ""
    static func mountCoverReaders(in view: NSView) {
        let ids = coverIds + FirstStartGeometry.paneNames.map({ "mac-first-start.veil.\($0)" })
        for id in ids { _ = FrameProbe.request(id) }
        let flow = FirstStartModel.shared
        let flowOn = flow.active
        let skipped = flow.skipped
        let needed = flowOn && !skipped ? ["mac-first-start.card"] : []
        let t0 = Date()
        func found() -> Bool { needed.allSatisfy { FrameProbe.contentFrame($0) != nil } }
        while !found() && Date().timeIntervalSince(t0) < coverWaitS {
            RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05))
            view.layoutSubtreeIfNeeded()
            view.displayIfNeeded()
        }
        let ms = Int(Date().timeIntervalSince(t0) * 1000)
        coverNote = needed.isEmpty ? "" : (found() ? "cover frames found after \(ms) ms" : "cover frames NOT found after \(ms) ms")
    }
    static func covers(in root: NSView, content: NSView) -> [NSRect] {
        coverIds.compactMap { id in
            _ = FrameProbe.request(id)
            guard let (f, _) = FrameProbe.contentFrame(id) else { return nil }
            let y = content.isFlipped ? f.minY : content.bounds.height - f.maxY
            return content.convert(NSRect(x: f.minX, y: y, width: f.width, height: f.height), to: root)
        }
    }

    /// The translucent layers above the web page that the page must show through, not be cut by: the first-start veils of the unlit panes (their own colour and alpha), as
    /// (place in `root`, colour). Drawn over the pasted page so the picture equals what the owner sees (main108: the body at full strength under the 0.82 veil).
    static func veils(in root: NSView, content: NSView, dark: Bool) -> [(NSRect, NSColor)] {
        let flow = FirstStartModel.shared
        let on = flow.active
        let skipped = flow.skipped
        guard on, !skipped else { return [] }
        let lit = FirstStartGeometry.lit(step: flow.step, lifted: flow.lifted)
        let v = FirstStartLook.veil.value(dark ? .dark : .light)
        let colour = NSColor(srgbRed: v.red, green: v.green, blue: v.blue, alpha: v.opacity)
        return FirstStartGeometry.paneNames.filter { !lit.contains($0) }.compactMap { name in
            let id = "mac-first-start.veil.\(name)"
            _ = FrameProbe.request(id)
            guard let (f, _) = FrameProbe.contentFrame(id) else { return nil }
            let y = content.isFlipped ? f.minY : content.bounds.height - f.maxY
            return (content.convert(NSRect(x: f.minX, y: y, width: f.width, height: f.height), to: root), colour)
        }
    }

    static func draw(_ images: [NSImage?], of targets: [Target], in root: NSView, covers: [NSRect] = [], veils: [(NSRect, NSColor)] = []) {
        for (img, t) in zip(images, targets) {
            guard let img else { continue }
            let p = t.placed
            let y = root.isFlipped ? root.bounds.maxY - p.maxY : p.minY - root.bounds.minY
            let dest = NSRect(x: p.minX - root.bounds.minX, y: y, width: p.width, height: p.height)
            NSGraphicsContext.saveGraphicsState()
            let clip = NSBezierPath(rect: dest)
            clip.windingRule = .evenOdd
            for c in covers {
                let cy = root.isFlipped ? root.bounds.maxY - c.maxY : c.minY - root.bounds.minY
                let hole = NSRect(x: c.minX - root.bounds.minX, y: cy, width: c.width, height: c.height).intersection(dest)
                if !hole.isNull { clip.appendRect(hole) }
            }
            clip.addClip()
            img.draw(in: dest, from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: nil)
            for (r, colour) in veils {
                let vy = root.isFlipped ? root.bounds.maxY - r.maxY : r.minY - root.bounds.minY
                let place = NSRect(x: r.minX - root.bounds.minX, y: vy, width: r.width, height: r.height).intersection(dest)
                if !place.isNull { colour.setFill(); place.fill(using: .sourceOver) }
            }
            NSGraphicsContext.restoreGraphicsState()
        }
    }
}
#endif
