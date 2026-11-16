import Foundation

/// A146: a mail wider than the reading pane is shown whole, scaled down to the pane's width as Apple Mail does with a fixed-width
/// newsletter; a mail that fits is shown at its own size (never enlarged). The owner's pinch zooms on top of the fit. Pure: the
/// page's numbers in, the scale out, so a test needs no web view (Tests/Unit/WideMailFitTests.swift).
enum WideMailFit {
    /// The largest shown scale a pinch reaches: three times the mail's own size (docs/CONSTANTS.md WIDE_MAIL_ZOOM_MAX).
    static let zoomMax: CGFloat = 3
    /// Half a point: a page whose width differs from the pane's by less is a page that fits (the same tolerance the height measure uses).
    static let tolerance: CGFloat = 0.5

    /// The scale that shows `content` points of natural width whole in `pane` points: pane / content when the mail is wider, else 1.
    static func scale(content: CGFloat, pane: CGFloat) -> CGFloat {
        guard content > 0, pane > 0, content > pane + tolerance else { return 1 }
        return pane / content
    }

    /// The scale shown: the fit times the owner's zoom, never below the fit (the whole mail stays reachable) and never above `zoomMax`.
    static func shown(fit: CGFloat, zoom: CGFloat) -> CGFloat {
        min(max(fit * zoom, fit), max(fit, zoomMax))
    }

    /// The height the reader gives the view: the page's own laid-out height at the scale shown, rounded up so nothing is cut at the bottom.
    static func height(layout: CGFloat, scale: CGFloat) -> CGFloat { (layout * scale).rounded(.up) }

    /// True when the mail at `scale` is wider than the pane: the card then scrolls sideways (only after a zoom; the fit alone never does).
    static func scrollsHorizontally(content: CGFloat, pane: CGFloat, scale: CGFloat) -> Bool {
        content > 0 && pane > 0 && content * scale > pane + tolerance
    }

    /// The owner's zoom after a pinch of `magnification` (AppKit's sum of the gesture: 0.5 is half as large again), clamped so the shown
    /// scale stays between the fit and `zoomMax`.
    static func zoom(after magnification: CGFloat, from zoom: CGFloat, fit: CGFloat) -> CGFloat {
        let wanted = zoom * (1 + magnification)
        let f = max(fit, 0.01)
        return min(max(wanted, 1), max(1, zoomMax / f))
    }

    /// A two-finger double tap: back to the fit when zoomed, else the mail at its own size (scale 1) when the fit had shrunk it.
    static func smartZoom(from zoom: CGFloat, fit: CGFloat) -> CGFloat {
        zoom > 1 + 0.001 || fit >= 1 ? 1 : 1 / max(fit, 0.01)
    }

    /// The app's own script (page script stays off) that shows the page at `scale`: a transform on the body from its top-left corner, so
    /// the layout, the HTML and the pictures stay as they are (no reload). The page's own vertical scroll is closed while scaled (the
    /// reader scrolls the thread); its sideways scroll is open only when `sideways` (a zoomed mail wider than the pane), else it is closed and
    /// put back to the left edge, so a fitted mail never shows a scroll bar or a shifted edge. Scale 1 puts the page back as loaded.
    static func script(scale: CGFloat, sideways: Bool) -> String {
        let s = String(format: "%.5f", Double(scale))
        let page = "var b=document.body,r=document.documentElement;"
        let side = sideways ? "r.style.overflowX='';" : "r.style.overflowX='hidden';(document.scrollingElement||r).scrollLeft=0;"
        if abs(scale - 1) < 0.0005 {
            return page + "b.style.transform='';b.style.transformOrigin='';delete b.dataset.ws;r.style.overflowY='';" + (sideways ? side : "r.style.overflowX='';")
        }
        return page + "b.style.transformOrigin='0 0';b.style.transform='scale(\(s))';b.dataset.ws='\(s)';r.style.overflowY='hidden';" + side
    }

    /// The measure: [laid-out height, where the written text ends, characters of text, the content's natural width, the page's width,
    /// the scale the page shows now]. Heights are read back at scale 1 (the page's own layout); the natural width is the body's own
    /// scroll width, which its transform does not change, so a measure on a scaled page reads the same width.
    /// A body that overflows reports a scroll width with its left padding and without its right one: the right padding (the paper's inner margin,
    /// BODY_PAPER_PAD_PX on a designed mail) is added, so a wide mail fitted to the pane keeps its margin on both sides (lead, 13 Nov: 716 for a
    /// 700-point mail, its right edge on the paper's edge).
    static let measureScript = "(function(){var b=document.body,m=document.getElementById('m')||b,s=parseFloat(b.dataset.ws||'1')||1,w=b.scrollWidth;if(w>b.clientWidth){w+=parseFloat(getComputedStyle(b).paddingRight)||0}return [b.getBoundingClientRect().height/s,m.getBoundingClientRect().bottom/s,b.innerText.length,w,document.documentElement.clientWidth,s]})()"
}
