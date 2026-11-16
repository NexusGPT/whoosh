import WebKit

/// Keeps web views alive between messages. Opening a message used to build a new `WKWebView` (the
/// reader gives the body view `.id(content.id)`); now the view is checked out of this pool and its
/// content reloaded, so a pane keeps one web view. Every pooled view carries the same settings as a
/// fresh one (script off, nonpersistent store) and keeps its block list, installed once.
@MainActor
final class BodyWebPool {
    static let shared = BodyWebPool()
    private var idle: [WKWebView] = []
    private var ruled: Set<ObjectIdentifier> = []
    /// Views whose list is the one-document pictures list (A129 part 3); the next load of a blocked document puts the default one back.
    private var pictures: Set<ObjectIdentifier> = []

    func checkout(make: () -> WKWebView) -> WKWebView {
        let v = idle.popLast() ?? make()
        v.removeFromSuperview()
        v.setShown(false)           // the previous message must not flash while the new one loads
        return v
    }

    /// An idle-time warm-up (BodyWebGate.warmUp): one new view in the pool when it holds none.
    func prewarm(make: () -> WKWebView) {
        if idle.isEmpty { idle.append(make()) }
    }

    func checkin(_ v: WKWebView) {
        v.navigationDelegate = nil
        v.stopLoading()
        if !idle.contains(where: { $0 === v }) { idle.append(v) }
    }

    func hasRules(_ v: WKWebView) -> Bool { ruled.contains(ObjectIdentifier(v)) }
    func markRules(_ v: WKWebView) { ruled.insert(ObjectIdentifier(v)) }
    func carriesPicturesList(_ v: WKWebView) -> Bool { pictures.contains(ObjectIdentifier(v)) }
    func markPicturesList(_ v: WKWebView, _ on: Bool) { if on { pictures.insert(ObjectIdentifier(v)) } else { pictures.remove(ObjectIdentifier(v)) } }
    var idleCount: Int { idle.count }
}

extension WKWebView {
    /// What this view and the layers above it say: for the socket row `web` and for `painted`.
    func diagnose() -> [String: Any] {
        #if os(macOS)
        var hiddenAbove = false, v: NSView? = superview
        while let a = v { if a.isHidden || a.alphaValue < 1 { hiddenAbove = true }; v = a.superview }
        var filters = 0, minOpacity = 1.0, l: CALayer? = layer
        var chain: [String] = []
        while let c = l {
            filters += (c.filters?.count ?? 0) + (c.backgroundFilters?.count ?? 0) + (c.compositingFilter == nil ? 0 : 1)
            minOpacity = min(minOpacity, Double(c.opacity))
            chain.append(String(describing: type(of: c)))
            l = c.superlayer
        }
        return ["in_window": window != nil, "superview": superview.map { String(describing: type(of: $0)) } ?? "none", "hidden": isHidden,
                "ancestor_hidden": hiddenAbove, "alpha": Double(alphaValue), "width": Double(frame.width), "height": Double(frame.height),
                "visible_height": Double(visibleRect.height), "layer_filters": filters, "layer_min_opacity": minOpacity,
                "layer_chain": chain.prefix(8).joined(separator: ">"), "loading": isLoading, "window_visible": window?.isVisible ?? false,
                "window_occluded": !(window?.occlusionState.contains(.visible) ?? false)]
        #else
        return ["in_window": window != nil, "hidden": isHidden, "ancestor_hidden": false, "alpha": Double(alpha), "width": Double(frame.width),
                "height": Double(frame.height), "layer_filters": 0, "loading": isLoading]
        #endif
    }
    var isVisibleAndIdle: Bool {
        #if os(macOS)
        return alphaValue == 1 && !isLoading
        #else
        return alpha == 1 && !isLoading
        #endif
    }
    func setShown(_ on: Bool) {
        #if os(macOS)
        alphaValue = on ? 1 : 0
        #else
        alpha = on ? 1 : 0
        #endif
    }
}
