#if os(macOS)
import AppKit
import SwiftUI

/// The three panes as Apple Mail builds them: an NSSplitViewController with a sidebar item, a content-list
/// item and the reader (the pattern started from: NSSplitViewController + NSSplitViewItem(sidebarWith…) /
/// (contentListWith…), the AppKit three-pane that NavigationSplitView wraps). Both side panes drag and
/// collapse; the reader is the content pane and never collapses.
struct MacSplitView: NSViewControllerRepresentable {
    let sidebar: AnyView
    let list: AnyView
    let reader: AnyView
    let state: MacShellState
    var sizesWindow = true

    func makeNSViewController(context: Context) -> MacSplitController {
        MacSplitController(sidebar: sidebar, list: list, reader: reader, state: state, sizesWindow: sizesWindow)
    }

    func updateNSViewController(_ c: MacSplitController, context: Context) {
        c.hosts[0].rootView = sidebar
        c.hosts[1].rootView = list
        c.hosts[2].rootView = reader
        c.apply(reduceMotion: context.environment.accessibilityReduceMotion)
    }
}

final class MacSplitController: NSSplitViewController {
    typealias G = MacShellGeometry
    let hosts: [NSHostingController<AnyView>]
    let state: MacShellState
    private let sizesWindow: Bool
    private var placed = false
    var compact: Bool?
    private var resizeToken: NSObjectProtocol?

    init(sidebar: AnyView, list: AnyView, reader: AnyView, state: MacShellState, sizesWindow: Bool) {
        LaunchClock.once("split-init")   // perf-archive r3: launch phases around the three hosting controllers
        hosts = [sidebar, list, reader].map { NSHostingController(rootView: AnyView($0.focusEffectDisabled())) }   // no system focus ring on a pane's own views either (see `focusRingType` below)
        LaunchClock.once("split-hosts-made")
        self.state = state
        self.sizesWindow = sizesWindow
        super.init(nibName: nil, bundle: nil)
        for h in hosts { h.sizingOptions = [] }
        // A pane's host view takes the keyboard when the shell moves the pane focus (`focus`). AppKit then draws its focus ring around the WHOLE
        // pane once the window is in keyboard-focus mode (after a picker gave the keys back): a blue rectangle around the list column (lead,
        // 13 Nov, real-key audit on main118, pictures after ⇧⌘F and ⌘J). The focused ROW is the focus mark; the pane draws no ring.
        for h in hosts { h.view.focusRingType = .none }
        for (h, p) in zip(hosts, MacShellState.Pane.allCases) { h.view.setAccessibilityIdentifier("mac-main.\(p.rawValue)") }   // uistate.first_responder reads these ids
        splitView.isVertical = true
        splitView.dividerStyle = .thin
        let s = NSSplitViewItem(viewController: hosts[0])   // flush full-height sidebar, as the board draws it (the sidebarWith item is macOS 26's inset floating panel: 212 wide at 220, seen on shell-r7-16)
        s.minimumThickness = G.sidebarMinW
        s.maximumThickness = G.sidebarMaxW
        s.canCollapse = true
        s.holdingPriority = NSLayoutConstraint.Priority(rawValue: 260)
        let l = NSSplitViewItem(viewController: hosts[1])
        l.minimumThickness = G.listMinW
        l.maximumThickness = G.listMaxW
        l.canCollapse = true
        l.holdingPriority = NSLayoutConstraint.Priority(rawValue: 255)
        let r = NSSplitViewItem(viewController: hosts[2])
        r.minimumThickness = G.readerMinW
        r.canCollapse = false
        r.holdingPriority = NSLayoutConstraint.Priority(rawValue: 250)
        [s, l, r].forEach(addSplitViewItem)
    }

    @available(*, unavailable) required init?(coder: NSCoder) { nil }

    /// The window's chrome and size, set once and BEFORE the first display when the window is already there (launch-r2: launch-r1 measured
    /// a second layout pass of the whole shell, 45 ms, 17 of them in `setFrame(display: true)` after the first display at the default
    /// size). `display: false` and an unchanged size skipped: the first layout runs at the final size. Same values, same order as before.
    private var framed = false
    private func frameWindow(_ w: NSWindow) {
        guard !framed else { return }
        framed = true
        w.titlebarAppearsTransparent = true
        w.titleVisibility = .hidden
        keepTitleHidden(w)
        w.titlebarSeparatorStyle = .none   // the v2 drawing has no hairline under the traffic lights
        w.styleMask.insert(.fullSizeContentView)
        w.minSize = NSSize(width: G.windowMinW, height: G.windowMinH)   // the frame, as winframes reports it
        if sizesWindow {
            let size = NSSize(width: G.windowW, height: G.windowH)
            if w.frame.size != size { w.setFrame(NSRect(origin: w.frame.origin, size: size), display: false) }
            w.center()
        }
    }

    /// The main window never draws its title text: it would sit on the sidebar button (owner's picture, 16 Nov, the installed main195). `frameWindow`
    /// hides it once, and on the owner's copy something turned it on again after that (not seen on the hidden copy). So the window is watched: each time
    /// the title text is turned on, it is hidden again, in the same pass (no frame shows it).
    private var titleWatch: NSKeyValueObservation?
    private func keepTitleHidden(_ w: NSWindow) {
        guard titleWatch == nil else { return }
        titleWatch = w.observe(\.titleVisibility) { [weak self] _, _ in
            MainActor.assumeIsolated { self?.hideTitleText() }
        }
    }
    private func hideTitleText() {
        if let w = view.window, w.titleVisibility != .hidden { w.titleVisibility = .hidden }
    }

    override func viewWillAppear() {
        super.viewWillAppear()
        LaunchClock.mark("split-will-appear")
        guard !placed, let w = view.window else { return }   // no window yet: viewDidAppear below does it, as before
        frameWindow(w)
        // perf-archive round 2: the dividers go in BEFORE the first layout when the split view already spans its final width
        // (launch-r1 phase 5: placing them in viewDidAppear laid the three panes out a second time, about 45 ms). The same widths
        // as before: `placeDividers` takes the same numbers from the same window width. A split view not yet sized (width 0 or
        // narrower than the two side panes) keeps the old place, viewDidAppear, so nothing is placed against a wrong width.
        placeIfSized(w, "dividers-in-will-appear")
    }

    /// AppKit's hook before this view lays out its subviews (the frame the parent gave it is set by then): the usual first moment
    /// the split view has its final width. Placing here, the panes' first layout is at their placed widths. After placement: one Bool.
    override func viewWillLayout() {
        super.viewWillLayout()
        if framed { hideTitleText() }   // a second guard beside the watch: one comparison per layout
        guard !placed, framed, let w = view.window else { return }   // the window is sized already (viewWillAppear); never resized inside a layout
        placeIfSized(w, "dividers-in-will-layout")
    }

    /// Places only when the split view spans the window's content (its final width) and has room for both side panes; else nothing.
    private func placeIfSized(_ w: NSWindow, _ phase: String) {
        let sb = splitView.bounds.width
        guard !placed, sb >= w.contentLayoutRect.width - 1, sb > G.sidebarW + G.listW else { return }
        placeOnce(w)
        LaunchClock.mark(phase)
    }

    override func viewDidAppear() {
        super.viewDidAppear()
        guard let w = view.window else { return }
        if !placed { frameWindow(w); placeOnce(w); LaunchClock.mark("dividers-in-did-appear") }
        guard !installed else { return }
        installed = true
        MacToolbarHost.shared.install(in: w)
        // A window that crosses the compact breakpoint (a resize, the socket's `resize`) takes the other column widths.
        resizeToken = NotificationCenter.default.addObserver(forName: NSWindow.didResizeNotification, object: w, queue: .main) { [weak self] _ in
            MainActor.assumeIsolated { self?.windowResized() }
        }
        record()
        LaunchClock.mark("split-did-appear-end")
    }

    private var installed = false   // the toolbar and the resize observer, once, in viewDidAppear as before

    /// The first placement: the dividers at the window's widths, then the shell state's collapse, then the list beside the sidebar.
    /// Same calls, same order as the old viewDidAppear block; runs once.
    private func placeOnce(_ w: NSWindow) {
        guard !placed else { return }
        placeDividers(compact: w.frame.width < G.compactBelowW)
        // placing the dividers opens every pane: the shell state (already collapsed or not) wins again
        splitViewItems[0].isCollapsed = state.sidebarCollapsed
        splitViewItems[1].isCollapsed = state.listCollapsed
        placeListForSidebar(animated: false)   // launched hidden (remembered): the list starts wide, no slide
        placed = true
    }

    private func windowResized() {
        guard placed, let w = view.window else { return }
        let c = w.frame.width < G.compactBelowW
        MacToolbarHost.shared.install(in: w)
        guard c != compact else { return }
        placeDividers(compact: c)
        splitViewItems[0].isCollapsed = state.sidebarCollapsed
        splitViewItems[1].isCollapsed = state.listCollapsed
        placeListForSidebar(animated: false)
        record()
    }

    private(set) lazy var slide = MacSlideTween(
        sidebar: .init(item: splitViewItems[0], view: hosts[0].view, rest: (G.sidebarMinW, G.sidebarMaxW)),
        list: .init(item: splitViewItems[1], view: hosts[1].view, rest: (G.listMinW, G.listMaxW))) { [weak self] in
        self?.settleDividers()   // pins lifted: put both dividers exactly on target so the holding priorities cannot drift them
    }
    var slidingToCollapsed: Bool?

    /// Sidebar and list at the wide or the compact widths of the v2 drawing (docs/CONSTANTS.md).
    private func placeDividers(compact c: Bool) {
        compact = c
        let sb = c ? G.sidebarCompactW : G.sidebarW, ls = c ? G.listCompactW : G.listW
        splitView.setPosition(sb, ofDividerAt: 0)
        splitView.setPosition(sb + splitView.dividerThickness + ls, ofDividerAt: 1)
    }

    /// The shell state is the truth for collapse; a drag that collapses a pane writes back to it.
    func apply(reduceMotion: Bool) {
        let want = [state.sidebarCollapsed, state.listCollapsed]
        if slide.running, placed { slideRetargetIfNeeded(); focus(state.focusedPane); return }
        for (i, collapsed) in want.enumerated() where splitViewItems[i].isCollapsed != collapsed {
            if i == 0, placed, !reduceMotion, !HiddenRig.isOn, view.window != nil { slideSidebar(collapsed: collapsed); continue }   // a hidden (alpha 0) window gets no animation frames: it jumps to the end state
            // Reduced Motion or no window: the pane jumps to its end state; nothing moves.
            guard !reduceMotion, !HiddenRig.isOn, view.window != nil else {
                splitViewItems[i].isCollapsed = collapsed
                if i == 0, placed { placeListForSidebar(animated: false) }   // Reduced Motion: the list jumps with the sidebar
                continue
            }
            // docs/MOTION.md rules 1, 2, 4: the state already changed (MacShellState, same frame); the pane
            // settles over TAB_MORPH_MS on a no-overshoot spring-like curve, no completion handler, and a
            // second toggle mid-flight re-targets the same animator instead of queueing.
            NSAnimationContext.runAnimationGroup { ctx in
                ctx.duration = WhooshMotion.tabMorph
                ctx.timingFunction = CAMediaTimingFunction(controlPoints: 0.2, 0.9, 0.3, 1)
                ctx.allowsImplicitAnimation = true
                splitViewItems[i].animator().isCollapsed = collapsed
            }
        }
        focus(state.focusedPane)
    }

    private func focus(_ pane: MacShellState.Pane) {
        guard let w = view.window, let i = MacShellState.Pane.allCases.firstIndex(of: pane) else { return }
        let target = hosts[i].view
        if let r = w.firstResponder as? NSView, r.isDescendant(of: target) { return }
        w.makeFirstResponder(target)
    }

    override func splitViewDidResizeSubviews(_ notification: Notification) {
        super.splitViewDidResizeSubviews(notification)
        guard placed else { return }   // a resize before placement is layout, not the owner's drag
        if slide.running { record(); return }   // during the slide the tween owns the items; the widths on screen are still reported
        let collapsed = [splitViewItems[0].isCollapsed, splitViewItems[1].isCollapsed]
        if state.sidebarCollapsed != collapsed[0] { state.sidebarCollapsed = collapsed[0] }
        if state.listCollapsed != collapsed[1] { state.listCollapsed = collapsed[1] }
        record()
    }

    private func record() {
        var widths: [MacShellState.Pane: CGFloat] = [:]
        for (i, p) in MacShellState.Pane.allCases.enumerated() {
            widths[p] = splitViewItems[i].isCollapsed ? 0 : hosts[i].view.frame.width
        }
        state.record(widths)
    }
}
#endif

#if os(macOS)
/// The sidebar's desktop blur (WhooshSidebarMaterial: vibrant material, not glass): AppKit's own
/// behind-window sidebar material, the token's tint drawn over it by the shell.
struct MacSidebarVibrancy: NSViewRepresentable {
    func makeNSView(context: Context) -> NSVisualEffectView {
        let v = NSVisualEffectView()
        v.material = .sidebar
        v.blendingMode = .behindWindow
        v.state = .followsWindowActiveState
        return v
    }
    func updateNSView(_ v: NSVisualEffectView, context: Context) {}
}
#endif
