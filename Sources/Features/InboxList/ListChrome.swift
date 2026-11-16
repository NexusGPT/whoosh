import SwiftUI
#if canImport(AppKit)
import AppKit
#endif

/// The list's floating glass: the selection bar (X, ⇧J) and the undo capsule. Both rise UNDO_RISE_PX with
/// opacity (transform + opacity only, docs/MOTION.md law 5); Reduced Motion turns the rise into a cross-fade.
struct ListChrome: View {
    let model: AppModel
    @Bindable var list: InboxListState
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        let _ = InteractionProbe.noteBody(.chromeBodies, inDispatch: .chromeBodiesInDispatch)   // perf-archive r6: a count for the measure
        VStack(spacing: WhooshSpacing.s8) {
            if !list.selection.isEmpty { SelectionBar(model: model, count: list.selection.count).transition(rise) }
            // one pill (components r4): with a suggestion on offer the action words lead it ("Moved to junk · Block sender? B");
            // without one, the undo capsule alone. Z undoes either way.
            if list.selection.isEmpty, SuggestionActs.current(model) != nil { SuggestionFoot(lead: list.undoWords, undoable: list.undoAvailable).transition(rise) }
            else if let words = list.undoWords, list.selection.isEmpty { undoCapsule(words).transition(rise) }
        }
        .padding(.bottom, WhooshSpacing.s8)          // the foot strip below is its own view (ListFootStrip)
        .animation(WhooshMotion.animation(WhooshMotion.newRow, reduceMotion: reduceMotion), value: list.selection.isEmpty)
        .animation(WhooshMotion.animation(WhooshMotion.newRow, reduceMotion: reduceMotion), value: list.undoSaid)
    }

    private var rise: AnyTransition {
        reduceMotion ? .opacity : .opacity.combined(with: .offset(y: WhooshMotion.undoRise))
    }

    private func undoCapsule(_ words: String) -> some View {
        HStack(spacing: WhooshSpacing.s10) {
            Text(words).font(WhooshTypography.subhead.font(emphasized: false)).foregroundStyle(WhooshPalette.label).lineLimit(1)
            if list.undoAvailable {
                Button(Words.text("Undo")) { _ = model.press("Z") }.buttonStyle(.plain)
                    .font(WhooshTypography.subhead.font(emphasized: true)).foregroundStyle(WhooshPalette.accent)
                KeyCap("Z")
            }
        }
        .padding(.horizontal, WhooshSpacing.s16).frame(height: WhooshMetrics.segmentHeight)
        .background(Capsule().fill(WhooshPalette.background))
        .whooshGlass(in: Capsule())
        .overlay { if list.undoAvailable { UndoTimerLine(list: list, reduceMotion: reduceMotion).id(list.undoGen) } }
        .accessibilityIdentifier("mac-main.list.undo")
    }
}

/// The time left on the undo capsule: its outline, a 1.5 pt accent line that empties over the capsule's life and ends as the
/// capsule leaves. One linear `trim` animation started when the capsule appears, for the time the same stamp says is left
/// (`InboxListState.undoTimer`, the clock whose expiry removes the capsule); nothing writes per frame and nothing sits on the
/// action's path. Keyed on the capsule's generation, so a second action restarts it. Reduced Motion: the line stays full and
/// leaves with the capsule's fade.
private struct UndoTimerLine: View {
    let list: InboxListState
    let reduceMotion: Bool
    @State private var left: Double = 1

    var body: some View {
        Capsule().inset(by: 0.75).trim(from: 0, to: left)
            .stroke(WhooshPalette.accent, style: StrokeStyle(lineWidth: 1.5, lineCap: .round))
            .allowsHitTesting(false).accessibilityHidden(true)
            .onAppear {
                guard !reduceMotion else { return }
                let frac = list.undoTimer ?? 1
                left = frac
                withAnimation(.linear(duration: frac * Double(InboxListState.undoChipMs) / 1000)) { left = 0 }
            }
    }
}

/// The action bar for a selection: one glass bar naming the count, then the batch actions with their keys (the same
/// five as the row strip, read from the live preset). One command carries every selected id, so one Z undoes the batch
/// where the action keeps an undo entry. Its cross (and Esc) clears the selection.
struct SelectionBar: View {
    let model: AppModel
    let count: Int

    /// The full bar (glyph and key cap per action) when the pane has room; else the glyphs alone, the keys in their tooltips.
    var body: some View {
        ViewThatFits(in: .horizontal) {
            bar(keys: true)
            bar(keys: false)
        }
        .padding(.horizontal, WhooshSpacing.s8)
        .whooshAx("mac-main.list.selectionbar")
    }

    private func bar(keys: Bool) -> some View {
        HStack(spacing: WhooshSpacing.s10) {
            Text(Words.text("%lld selected", count)).font(WhooshTypography.subhead.font(emphasized: true)).foregroundStyle(WhooshPalette.label)
                .contentTransition(.numericText()).monospacedDigit().lineLimit(1).fixedSize()
            ForEach(RowStrip.items) { a in
                Button { _ = TrainerFeature.mouseUsed(a.id, model) } label: {
                    HStack(spacing: WhooshSpacing.s2) {
                        Image(systemName: a.symbol).font(WhooshTypography.subhead.font(emphasized: false))
                        if keys { KeyCap(LiveKeys.chord(a.id, a.key)).fixedSize() }
                    }
                }
                .buttonStyle(.plain).foregroundStyle(WhooshPalette.label).help(RowStrip.help(a))
                .accessibilityLabel(RowStrip.help(a))
                .whooshAx("mac-main.list.selectionbar.\(a.id)")
            }
            Button { _ = model.press("Escape") } label: {
                HStack(spacing: WhooshSpacing.s2) {
                    Image(systemName: "xmark")
                    if keys { KeyCap("Esc").fixedSize() }
                }
            }
            .buttonStyle(.plain).foregroundStyle(WhooshPalette.secondaryLabel).help(Words.text("Clear selection · Esc"))
            .accessibilityLabel(Words.text("Clear selection · Esc"))
            .whooshAx("mac-main.list.selectionbar.clear")
        }
        .padding(.horizontal, WhooshSpacing.s14).frame(height: WhooshMetrics.glassPillHeight)
        .whooshGlass(in: Capsule())
    }
}

/// How a row enters and leaves (owner 2026-11-11: "on superhman they go on the left and dissolve gently", "but fast"). The
/// leaving line slides LEFT and dissolves (transform and opacity) while its height closes after a short delay, so the rows
/// below rise behind it: four numbers in WhooshMotion (`rowExit*`). A new row drops in over ROW_INSERT_MS (unchanged). The
/// model already dropped the row on the key frame (MOTION law 1); the transaction that carries the drop is animated by
/// `RowCollapse.run`. Reduced Motion: a cross-fade, the list reflows at once, no slide.
enum RowMotion {
    static func transition(reduceMotion: Bool, height: CGFloat) -> AnyTransition {
        if reduceMotion { return .opacity.animation(WhooshMotion.archiveCollapse) }
        return .asymmetric(
            insertion: .opacity.combined(with: .offset(y: -WhooshMotion.undoRise)).animation(WhooshMotion.newRow),
            removal: .modifier(active: RowExitEffect(elapsedMs: Double(WhooshMotion.rowExitTotalMs), full: height),
                               identity: RowExitEffect(elapsedMs: 0, full: height)).animation(WhooshMotion.rowExit))
    }
}

/// A cubic-bezier easing (the CSS form of the drawing), solved for x by a few Newton steps then bisection. Pure.
nonisolated struct RowExitCurve {
    let x1, y1, x2, y2: Double
    func at(_ t: Double) -> Double {
        if t <= 0 { return 0 }
        if t >= 1 { return 1 }
        func bez(_ u: Double, _ a: Double, _ b: Double) -> Double { 3 * a * u * (1 - u) * (1 - u) + 3 * b * u * u * (1 - u) + u * u * u }
        func dbez(_ u: Double, _ a: Double, _ b: Double) -> Double { 3 * a * (1 - u) * (1 - u) + 6 * (b - a) * u * (1 - u) + 3 * (1 - b) * u * u }
        var u = t
        for _ in 0..<6 {
            let e = bez(u, x1, x2) - t, d = dbez(u, x1, x2)
            if abs(e) < 1e-5 { return bez(u, y1, y2) }
            if abs(d) < 1e-6 { break }
            u -= e / d
        }
        var lo = 0.0, hi = 1.0
        u = t
        for _ in 0..<24 {
            let e = bez(u, x1, x2) - t
            if abs(e) < 1e-5 { break }
            if e > 0 { hi = u } else { lo = u }
            u = (lo + hi) / 2
        }
        return bez(u, y1, y2)
    }
    static let slide = RowExitCurve(x1: 0.2, y1: 0.6, x2: 0.3, y2: 1)   // the drawing: cubic-bezier(.2,.6,.3,1)
    static let close = RowExitCurve(x1: 0.3, y1: 0.7, x2: 0.2, y2: 1)   // the drawing: cubic-bezier(.3,.7,.2,1)
}

/// The leaving row, animatable on one clock (`elapsedMs`, 0 to the total). The WHOLE card (background, highlight, mark, text)
/// slides left at its full height and dissolves (1 to 0.35 at 45 % of the slide, then to 0), drawn above the rows below,
/// while the row's own height closes from the delay on, so the gap shows for a moment and the rows below rise. Only
/// transform, opacity and the one collapsing row's height move (docs/MOTION.md rule 5). The card is not clipped by its row;
/// the list's scroll view clips at the pane's edge. `values` is pure so a test reads it.
/// `nonisolated`: Animatable's `animatableData` is read off the main actor by the animation system; the modifier holds
/// plain numbers and builds a view, so nothing here needs the main actor (main41 build: 'conformance crosses into
/// main actor-isolated code').
nonisolated struct RowExitEffect: ViewModifier, Animatable {
    var elapsedMs: Double
    let full: CGFloat
    var animatableData: Double {
        get { elapsedMs }
        set { elapsedMs = newValue }
    }
    struct Values: Equatable {
        /// how far along the slide, 0 to 1 (the drawing's keyframes: 0.55 at 45 % of the eased time, 1 at the end)
        var travel: Double
        var offsetX: CGFloat
        var opacity: Double
        var height: CGFloat
    }
    /// The four numbers are parameters so the defaults are not baked into the maths. `slidePt` 0 or less means the row's width
    /// (`rowWidth`). Reduced Motion: no slide.
    static func values(elapsedMs t: Double, full: CGFloat, slidePt: CGFloat = WhooshMotion.rowExitSlidePt,
                       slideMs: Int = WhooshMotion.rowExitSlideMs, delayMs: Int = WhooshMotion.rowExitCloseDelayMs,
                       closeMs: Int = WhooshMotion.rowExitCloseMs, rowWidth: CGFloat = 0, reduceMotion: Bool = false) -> Values {
        let s = RowExitCurve.slide.at(slideMs > 0 ? t / Double(slideMs) : 1)
        let c = RowExitCurve.close.at(closeMs > 0 ? (t - Double(delayMs)) / Double(closeMs) : 1)
        let knee = 0.45
        let travel = s <= knee ? 0.55 * (s / knee) : 0.55 + 0.45 * (s - knee) / (1 - knee)
        let opacity = s <= knee ? 1 - 0.65 * (s / knee) : 0.35 * (1 - (s - knee) / (1 - knee))
        let distance = slidePt > 0 ? slidePt : rowWidth
        return Values(travel: travel, offsetX: reduceMotion ? 0 : -distance * CGFloat(travel), opacity: opacity, height: full * CGFloat(1 - c))
    }
    func body(content: Content) -> some View {
        let v = Self.values(elapsedMs: elapsedMs, full: full)
        guard elapsedMs > 0 else { return AnyView(content) }
        let travel = v.travel, pt = WhooshMotion.rowExitSlidePt
        return AnyView(content
            .visualEffect { c, geo in c.offset(x: -(pt > 0 ? pt : geo.size.width) * CGFloat(travel)) }
            .opacity(v.opacity)
            .frame(height: full, alignment: .top)
            .frame(height: max(0, v.height), alignment: .top)
            .zIndex(1))
    }
}

/// Runs a command that takes rows off the list inside the exit animation, so the list's reflow is interpolated.
/// The command itself runs NOW, on the key's frame; the animation never gates it. Every removal glides, also the second of
/// a held key (as fast as one every 30 ms): the exits overlap, each its own animation on its own row, nothing queues and
/// nothing is skipped (owner 2026-11-11). Only Reduced Motion drops the row with no animation.
@MainActor
enum RowCollapse {
    private static var last: Date?
    /// probe: the last removal glided (true) or jumped (false)
    static private(set) var lastGlided = false
    /// probe: milliseconds between this removal and the one before (nil for the first); a measure only, it decides nothing
    static private(set) var lastGapMs: Int?

    static func run(_ body: () -> Bool) -> Bool {
        let now = Date()
        lastGapMs = last.map { Int(now.timeIntervalSince($0) * 1000) }
        last = now
        #if canImport(AppKit)
        let reduce = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        #else
        let reduce = UIAccessibility.isReduceMotionEnabled
        #endif
        lastGlided = !reduce
        guard lastGlided else { return body() }
        var ok = false
        withAnimation(WhooshMotion.archiveCollapse) { ok = body() }
        return ok
    }
}
