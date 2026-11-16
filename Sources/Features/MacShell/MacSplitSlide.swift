#if os(macOS)
import AppKit

/// The sidebar's slide and the list widths that go with it (docs/CONSTANTS.md LIST_W_PX, LIST_W_WIDE; the motion is MacSlideTween).
extension MacSplitController {
    /// The list column's width for the sidebar's state: LIST_W_WIDE while the sidebar is hidden, else the wide or compact
    /// width of the v2 drawing. The divider after the list sits at (the sidebar's width + its divider) + the list's width.
    func listTarget() -> CGFloat {
        state.sidebarCollapsed && G.mutant != "keep-list-width" ? ((compact ?? false) ? G.listWideCompactW : G.listWideW) : ((compact ?? false) ? G.listCompactW : G.listW)
    }

    /// Puts the list at its width for the current sidebar state, no motion (placement, a breakpoint cross, Reduced Motion).
    func placeListForSidebar(animated: Bool) {
        slide.cancel()
        if G.mutant == "reader-cap" { splitViewItems[2].maximumThickness = 600 }   // mutant: a capped reader leaves a blank band
        guard !state.listCollapsed else { return }
        let left = state.sidebarCollapsed ? 0 : ((compact ?? false) ? G.sidebarCompactW : G.sidebarW) + splitView.dividerThickness
        splitView.setPosition(left + listTarget(), ofDividerAt: 1)
    }

    /// A toggle while a slide runs: if the state now points the other way, re-target from the widths on screen.
    func slideRetargetIfNeeded() {
        guard slidingToCollapsed != state.sidebarCollapsed else { return }
        slidingToCollapsed = state.sidebarCollapsed
        slideSidebar(collapsed: state.sidebarCollapsed)
    }

    func settleDividers() {
        guard !state.listCollapsed else { return }
        let sb = state.sidebarCollapsed ? 0 : sidebarWidthNow
        splitView.setPosition(sb, ofDividerAt: 0)
        splitView.setPosition((state.sidebarCollapsed ? 0 : sb + splitView.dividerThickness) + listTarget(), ofDividerAt: 1)
    }

    var sidebarWidthNow: CGFloat { (compact ?? false) ? G.sidebarCompactW : G.sidebarW }

    /// The sidebar slides while the list's width follows on the same curve (MacSlideTween): one motion. A second toggle
    /// mid-flight re-targets both from the widths on screen.
    func slideSidebar(collapsed: Bool) {
        slidingToCollapsed = collapsed
        slide.start(sidebarTo: collapsed ? 0 : sidebarWidthNow, listTo: state.listCollapsed ? 0 : listTarget())
    }

}
#endif
