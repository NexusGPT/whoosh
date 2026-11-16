import SwiftUI
#if os(macOS)
import AppKit
#endif
import Observation

/// Frames by accessibility id for the socket's `measure <ax-id>`. SwiftUI builds its accessibility
/// tree only for an assistive client, so the shell cannot read frames from it in-process (seen on the
/// rig: 7 elements walked, none of the view ids). A view that wants to be measured uses `.whooshAx(id)`,
/// which sets the accessibility id and, ONLY WHILE A SOCKET CLIENT IS MEASURING, reports the view's frame here,
/// in window points, origin top-left.
///
/// Cost when nobody measures: none beyond the accessibility id (and, since the `labels` verb, one read of `gate.labels` where
/// `whooshAx` is called; no view, reader, copy or preference is added while it is false). Round 2 (list lane's arrow measurements): the old
/// probe put a GeometryReader and a global-frame onChange behind every marked view, so each focus move re-measured
/// every row. Now the readers exist only after `request(_:)` switched the probe on, and a 3 s idle timer switches it off.
@MainActor
public enum FrameProbe {
    @MainActor @Observable
    final class Switch {
        var on = false
        /// The `labels` verb's own switch: the hidden text copies mount only while it AND `on` are true. `measure` never sets it.
        var labels = false
        /// Counts up when an id lost its reader while the probe is on: every reader still mounted reports again (see `clear`).
        var again = 0
    }
    static let gate = Switch()
    /// How long the readers stay mounted after the last frame request, in seconds. Pending a docs/CONSTANTS.md row.
    static let lingerS = Double(WhooshConstants.PROBE_LINGER_S)
    /// How long `request` lets the run loop turn so the just-mounted readers publish, in seconds.
    static let settleS = WhooshConstants.PROBE_SETTLE_S

    private static var frames: [String: CGRect] = [:]
    private static var idleTimer: Timer?

    static func set(_ id: String, _ r: CGRect?) { frames[id] = r }

    /// Which reader holds each id. A row whose place in its list changes reports under a new id, and the row that held that id leaves a moment
    /// later: its goodbye must not take the id away from the row that took it over. Seen 13 Nov 2026 (main136): after one rule row was removed
    /// the next row's Remove control had no frame, so the check's click fell on nothing. `clear` forgets an id only for the reader that holds it.
    private static var owners: [String: UUID] = [:]
    static func set(_ id: String, _ r: CGRect, owner: UUID) { frames[id] = r; owners[id] = owner }
    static func clear(_ id: String, owner: UUID) {
        guard owners[id] == owner else { return }
        frames[id] = nil; radii[id] = nil; owners[id] = nil
        askAgain()
    }

    /// Two views can stand under ONE id (a second main window draws the same sidebar rows): the last one to report holds the id, and when it leaves the id
    /// would stay empty although the first view is still on screen (par-macshell, main136 to main139: after a second window was opened and closed, the first
    /// window's sidebar rows had no frame). So a cleared id asks every mounted reader to report again, once per run-loop turn however many ids were cleared.
    private static var againQueued = false
    private static func askAgain() {
        guard gate.on, !againQueued else { return }
        againQueued = true
        DispatchQueue.main.async { againQueued = false; if gate.on { gate.again &+= 1 } }
    }

    /// The corner radius a marked view says it draws (`whooshAx(_:radius:)`), served on `measure` beside the frame (component-grammar seam, C1).
    /// SwiftUI exposes no drawn radius to a reader, so the view hands over the constant it draws with; kept only while measuring, like the frames.
    private static var radii: [String: CGFloat] = [:]
    static func setRadius(_ id: String, _ r: CGFloat?) { radii[id] = r }
    static func radius(_ id: String) -> CGFloat? { radii[id] }

    /// True while the readers are mounted (a test hook).
    public static var isMeasuring: Bool { gate.on }

    /// The frame of `id`. Switches the probe on if it is off and lets the main run loop turn once so the readers publish.
    public static func request(_ id: String) -> CGRect? {
        armIdle()
        if !gate.on {
            gate.on = true
            settle()
        }
        return frames[id]
    }

    /// A long wait inside a verb (the shot's settle can run 16 s) keeps the readers mounted: the linger timer starts again, so the frames the verb reads after its
    /// wait are still there (main112: the 3 s timer unmounted every reader during the wait, and the shot answered `null` for the attachment header's ids).
    static func keepAlive() { if gate.on { armIdle() } }

    private static func armIdle() {
        idleTimer?.invalidate()
        idleTimer = Timer.scheduledTimer(withTimeInterval: lingerS, repeats: false) { _ in
            MainActor.assumeIsolated { gate.on = false; frames.removeAll(); radii.removeAll(); owners.removeAll(); idleTimer = nil }
        }
    }

    private static func settle() {
        let deadline = Date().addingTimeInterval(settleS)
        while Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
    }

    // MARK: labels (the socket's `labels` verb): what a leaf text needs against the box it got

    private static var labelsTimer: Timer?

    /// The `labels` verb's switch: turns `on` and `labels` on, arms both linger timers (`lingerS`), settles if either was off.
    /// The labels timer is armed with the frame timer and never later, so it fires first; a `measure` re-arms only the frame timer.
    static func switchLabelsOn() {
        labelsTimer?.invalidate()
        labelsTimer = Timer.scheduledTimer(withTimeInterval: lingerS, repeats: false) { _ in
            MainActor.assumeIsolated {
                gate.labels = false; texts.removeAll(); needs.removeAll(); claimed.removeAll(); capped = false; labelsTimer = nil
            }
        }
        armIdle()
        let turned = !gate.on || !gate.labels
        gate.on = true
        gate.labels = true
        if turned { settle() }
    }

    /// The two sizes a leaf text's hidden copies took: `ideal` with the width left free (one line, never wrapped), `atWidth`
    /// at the box's width with the height left free (wrapped as the box wraps it).
    struct Need { var ideal: CGSize?; var atWidth: CGSize? }
    private static var texts: [String: String] = [:]
    private static var needs: [String: Need] = [:]
    static func setText(_ id: String, _ t: String?) { texts[id] = t; if t == nil { needs[id] = nil } }
    static func setNeed(_ id: String, ideal: Bool, _ s: CGSize) {
        var n = needs[id] ?? Need()
        if ideal { n.ideal = s } else { n.atWidth = s }
        needs[id] = n
    }

    /// At most this many leaf texts carry copies at once (two text layouts each), so one `labels` request adds at most
    /// 2 x 200 text layouts to the window it reads. A cap, not a design number: past it a view answers `text: null` and the
    /// reply says `capped: true`.
    static let labelCopyCap = 200
    private static var claimed: Set<String> = []
    private(set) static var capped = false

    /// A copy at the box's width taller than the one-line copy by more than this wraps (points; frames snap to the pixel
    /// grid, half a point at 2x, so a smaller difference is rounding, not a second line).
    static let wrapTolPt: CGFloat = 0.5
    /// Mirror walk caps inside a `Text` (ceilings, not design numbers).
    private static let mirrorNodeCap = 120
    private static let mirrorDepthCap = 24

    /// The modifiers a leaf text may carry between it and `.whooshAx`, by type name as `String(describing:)` prints it. Each one
    /// only sets layout, look or accessibility metadata: none holds a view, a closure, an action or a task, so a hidden copy of
    /// the text cannot fetch, mark, focus or change anything. CLOSED apart from `leafModifierFamilies`: a type not listed makes the view "not a leaf" (no copy,
    /// `text: null`). Source of each name: the SwiftUICore/SwiftUI .swiftinterface of the macOS 26.5 SDK (the modifier the
    /// public call returns, shown in parentheses).
    static let leafModifiers: Set<String> = [
        "_PaddingLayout",                                    // padding()
        "_FrameLayout",                                      // frame(width:height:alignment:)
        "_FlexFrameLayout",                                  // frame(minWidth:...maxHeight:alignment:)
        "_FixedSizeLayout",                                  // fixedSize(), fixedSize(horizontal:vertical:)
        "_OpacityEffect",                                    // opacity(_:)
        "_TraitWritingModifier<LayoutPriorityTraitKey>",     // layoutPriority(_:)
        "AccessibilityAttachmentModifier",                   // accessibilityIdentifier, accessibilityLabel, accessibilityHidden (the copy is hidden from accessibility)
        "_EnvironmentKeyWritingModifier<Optional<Font>>",    // font(_:) on a View
        "_EnvironmentKeyWritingModifier<Optional<Color>>",   // foregroundColor(_:) on a View
        "_EnvironmentKeyWritingModifier<Optional<Int>>",     // lineLimit(_:)
        "_EnvironmentKeyWritingModifier<TextAlignment>",     // multilineTextAlignment(_:)
        "_EnvironmentKeyWritingModifier<TruncationMode>",    // truncationMode(_:)
        "_EnvironmentKeyWritingModifier<CGFloat>",           // minimumScaleFactor(_:) (a number; no action can ride on it)
        "_ForegroundStyleModifier<Color>",                   // foregroundStyle(_:) with a Color
        "_ForegroundStyleModifier<HierarchicalShapeStyle>",  // foregroundStyle(.secondary) and the like
        "_ForegroundStyleModifier<ThemeColor>",              // foregroundStyle(WhooshPalette.x): the app's own colour (Sources/Design/ThemeColor.swift, a ShapeStyle)
        "_TraitWritingModifier<TransitionTraitKey>",         // transition(_:): a trait the parent reads when the view comes and goes
    ]

    /// The families of environment-only modifiers a leaf text may carry. `.whooshText` (Sources/Design/WhooshTypography.swift) is
    /// `font`, `tracking`, `monospacedDigit`, `lineSpacing` and `padding` on a View; the SDK interface gives the bodies of `font`
    /// (an `_EnvironmentKeyWritingModifier`) and `lineSpacing`, `padding` (`_PaddingLayout`, listed above) but declares `tracking` and
    /// `monospacedDigit` only as `some View`, so their modifier names cannot be listed one by one. Both families below hold only a
    /// key path and a value or a transform of an environment value: no view, no action, no task. Prefix-matched on the printed type name.
    static let leafModifierFamilies: [String] = ["_EnvironmentKeyWritingModifier<", "_EnvironmentKeyTransformModifier<"]

    /// True when a modifier of this printed type name may sit between a leaf text and `.whooshAx` (the one place that decides it).
    static func isLeafModifier(_ typeName: String) -> Bool {
        leafModifiers.contains(typeName) || leafModifierFamilies.contains { typeName.hasPrefix($0) }
    }

    /// The text of a LEAF text, read with Mirror only while `gate.labels` is on, and a claim on one of `labelCopyCap` copies.
    /// A leaf is a `Text`, or a `Text` inside `ModifiedContent` layers whose modifier type is in `leafModifiers`. Its text is
    /// every verbatim string or localization key inside the `Text`, joined by a space. Nil (no copy) for anything else.
    static func leafText(_ id: String, _ value: Any) -> String? {
        var v = value
        var depth = 0
        while !(v is Text) {
            depth += 1
            guard depth < mirrorDepthCap, String(describing: type(of: v)).hasPrefix("ModifiedContent<") else { return nil }
            var content: Any?
            var modifier: Any?
            for c in Mirror(reflecting: v).children {
                if c.label == "content" { content = c.value } else if c.label == "modifier" { modifier = c.value }
            }
            guard let c = content, let m = modifier, isLeafModifier(String(describing: type(of: m))) else { return nil }
            v = c
        }
        var found: [String] = []
        var budget = mirrorNodeCap
        func strings(_ x: Any, _ label: String?, _ d: Int) {
            guard budget > 0, d < mirrorDepthCap else { return }
            budget -= 1
            if let s = x as? String { if label == "verbatim" || label == "key" { found.append(s) }; return }
            for c in Mirror(reflecting: x).children { strings(c.value, c.label, d + 1) }
        }
        strings(v, nil, 0)
        guard budget > 0, !found.isEmpty else { return nil }
        if !claimed.contains(id) {
            guard claimed.count < labelCopyCap else { capped = true; return nil }
            claimed.insert(id)
        }
        return found.joined(separator: " ")
    }

    public static func frame(_ id: String) -> CGRect? { frames[id] }

    #if os(macOS)
    /// The AppKit view each marked view carries while measuring: its frame is read from it at request time and
    /// converted to its window's CONTENT view (points, origin top-left). SwiftUI's `.global` is the root of the
    /// hosting view that draws it, and the Mac shell hosts each pane in its own NSHostingController, so a `.global`
    /// frame in the reader pane started at the pane, not at the window (quickreply r4's offset).
    private static var anchors: [String: WeakView] = [:]
    final class WeakView { weak var view: NSView?; init(_ v: NSView) { view = v } }
    static func anchor(_ id: String, _ v: NSView?) { anchors[id] = v.map(WeakView.init) }
    /// Forgets `id` only while `v` is the view under it (or none is left): a view that leaves never takes away an id its neighbour took over (see `clear`).
    static func forgetAnchor(_ id: String, _ v: NSView) {
        if let held = anchors[id]?.view, held !== v { return }
        anchors[id] = nil
        askAgain()
    }

    /// `id`'s frame in its window's content view, from its anchor; nil when it has no anchor in a window.
    static func contentFrame(_ id: String) -> (CGRect, NSWindow)? {
        guard let v = anchors[id]?.view, let w = v.window, let content = w.contentView else { return nil }
        // A scroll moves a view through its clip view, and SwiftUI settles the hosted views' frames on its next layout: lay the window out now (only when a check asks), so the
        // frame served is the place as drawn after the scroll, not the one before it (hid-start: y 552 served, 473 drawn after a 79 pt scroll).
        content.layoutSubtreeIfNeeded()
        let r = v.convert(v.bounds, to: content)
        let y = content.isFlipped ? r.minY : content.bounds.height - r.maxY
        return (CGRect(x: r.minX, y: y, width: r.width, height: r.height), w)
    }
    #endif

    #if os(macOS)
    /// One row per marked view whose anchor is in `target` and on its content view: `ax`, `text` (or null), `need_w`, `need_h`,
    /// `box_w`, `box_h` in points. One-line text needs its ideal width and its one-line height; text that wraps at the box's
    /// width needs that width and its wrapped height. A view with no copy answers `text: null` and its box as its need.
    /// `judged` counts the rows with text, `marked` all rows.
    static func labels(in target: NSWindow) -> [String: Any] {
        var rows: [ControlRow] = []
        var judged = 0
        for id in anchors.keys.sorted() {
            guard let (box, w) = contentFrame(id), w === target, let content = w.contentView else { continue }
            guard box.width > 0, box.height > 0, box.intersects(content.bounds) else { continue }
            var row: ControlRow = ["ax": id, "box_w": box.width, "box_h": box.height]
            if let t = texts[id], let a = needs[id]?.ideal, let b = needs[id]?.atWidth {
                let wraps = b.height > a.height + wrapTolPt
                row["text"] = t
                row["need_w"] = wraps ? max(box.width, b.width) : a.width
                row["need_h"] = b.height
                judged += 1
            } else {
                row["text"] = NSNull()
                row["need_w"] = box.width
                row["need_h"] = box.height
            }
            rows.append(row)
        }
        return ["labels": rows, "judged": judged, "marked": rows.count, "capped": capped]
    }
    #endif

    public static var ids: [String] { frames.keys.sorted() }
}

struct WhooshAxModifier: ViewModifier {
    let id: String
    /// A leaf text's string; set by `whooshAx` only while `gate.labels` is on (nil, never looked for, otherwise).
    var text: String? = nil
    /// The corner radius the view draws, when it says so (see `FrameProbe.setRadius`).
    var radius: CGFloat? = nil

    func body(content: Content) -> some View {
        let measuring = FrameProbe.gate.on
        content
            .accessibilityIdentifier(id)
            .background {
                if measuring {
                    #if os(macOS)
                    FrameAnchor(id: id, again: FrameProbe.gate.again)
                    #endif
                    ProbeFrameReader(id: id, radius: radius)
                    if let text, FrameProbe.gate.labels {
                        LabelCopies(id: id, text: text, content: content)
                    }
                }
            }
    }
}

/// Reports the frame of the marked view it sits behind, under the view's CURRENT id (mounted only while measuring). The id of a list row holds its
/// index, so it changes when a row above it is removed: the reader then lets the old id go and reports under the new one (`FrameProbe.clear`).
private struct ProbeFrameReader: View {
    let id: String
    let radius: CGFloat?
    @State private var owner = UUID()  // local-ui: which reader holds the id, for the probe only

    var body: some View {
        GeometryReader { g in
            Color.clear
                .onAppear { FrameProbe.set(id, g.frame(in: .global), owner: owner); FrameProbe.setRadius(id, radius) }
                .onChange(of: g.frame(in: .global)) { _, r in FrameProbe.set(id, r, owner: owner) }
                .onChange(of: id) { old, new in
                    FrameProbe.clear(old, owner: owner)
                    FrameProbe.set(new, g.frame(in: .global), owner: owner); FrameProbe.setRadius(new, radius)
                }
                .onChange(of: FrameProbe.gate.again) { _, _ in FrameProbe.set(id, g.frame(in: .global), owner: owner); FrameProbe.setRadius(id, radius) }
                .onDisappear { FrameProbe.clear(id, owner: owner) }
        }
    }
}

/// Two hidden copies of a leaf text (`FrameProbe.leafModifiers`), mounted only by the `labels` switch: one with its width free,
/// one pinned to the box's width with its height free. Each lies over a box-sized clear view, so they never resize the readers beside them.
struct LabelCopies<C: View>: View {
    let id: String
    let text: String
    let content: C

    var body: some View {
        // Each copy in its own overlay, the second pinned to the box's width. In one stack the one-line copy made the stack as wide as
        // itself, and the stack then offered that width to the second copy, which never wrapped: a label drawn whole on two lines read as
        // one line wider than its box and was called cut (main191, Settings > Replies > Sources at 700x480: "Clients / Lorvan Hotels" in a
        // box of 83x30 read 135.5x15; the same figures in a stand-alone layout of the tile, 15 Nov 2026).
        Color.clear
            .overlay(alignment: .topLeading) {
                content.fixedSize(horizontal: true, vertical: false).background { NeedReader(id: id, ideal: true) }
                    .hidden().accessibilityHidden(true).allowsHitTesting(false)
            }
            .overlay(alignment: .topLeading) {
                GeometryReader { g in
                    content.frame(width: g.size.width, alignment: .leading).fixedSize(horizontal: false, vertical: true)
                        .background { NeedReader(id: id, ideal: false) }
                }
                .hidden().accessibilityHidden(true).allowsHitTesting(false)
            }
            .onAppear { FrameProbe.setText(id, text) }
            .onChange(of: text) { _, t in FrameProbe.setText(id, t) }
            .onDisappear { FrameProbe.setText(id, nil) }
    }
}

/// Reports the size of the hidden copy it sits behind (`ideal`: the one-line copy; else the copy at the box's width).
private struct NeedReader: View {
    let id: String
    let ideal: Bool
    var body: some View {
        GeometryReader { g in
            Color.clear
                .onAppear { FrameProbe.setNeed(id, ideal: ideal, g.size) }
                .onChange(of: g.size) { _, s in FrameProbe.setNeed(id, ideal: ideal, s) }
        }
    }
}

#if os(macOS)
/// A zero-cost NSView laid out exactly where the marked view is; FrameProbe converts its bounds on request.
struct FrameAnchor: NSViewRepresentable {
    let id: String
    /// `FrameProbe.gate.again` as the marked view last read it: a new value makes SwiftUI call `updateNSView`, which registers the anchor again.
    var again = 0
    func makeNSView(context: Context) -> NSView {
        let v = AnchorView(); v.id = id; return v
    }
    func updateNSView(_ v: NSView, context: Context) {
        if let a = v as? AnchorView, a.id != id { FrameProbe.forgetAnchor(a.id, a); a.id = id }
        FrameProbe.anchor(id, v)
    }
    static func dismantleNSView(_ v: NSView, coordinator: ()) { if let a = v as? AnchorView { FrameProbe.forgetAnchor(a.id, a) } }

    final class AnchorView: NSView {
        var id = ""
        override func hitTest(_ point: NSPoint) -> NSView? { nil }   // never takes a click
        override func viewDidMoveToWindow() { super.viewDidMoveToWindow(); if window == nil { FrameProbe.forgetAnchor(id, self) } else { FrameProbe.anchor(id, self) } }
    }
}
#endif

public extension View {
    /// `.accessibilityIdentifier(id)` plus a frame the socket's `measure` can read (readers mounted only while measuring), and
    /// for a leaf text a need the socket's `labels` can read (copies mounted only under the `labels` switch). With that switch
    /// off this reads one Bool and passes `text: nil`; the Mirror walk runs only under it.
    @MainActor
    func whooshAx(_ id: String, radius: CGFloat? = nil) -> some View {
        modifier(WhooshAxModifier(id: id, text: FrameProbe.gate.labels ? FrameProbe.leafText(id, self) : nil, radius: radius))
    }
}
