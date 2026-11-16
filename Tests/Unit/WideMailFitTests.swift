import CoreGraphics
import Foundation
import Testing
@testable import WhooshUI

// A146: a mail whose HTML is wider than the reading pane is scaled down to the pane's width (WideMailFit, the pure part of the fit) and the
// reader's height follows the scaled page (MessageBodyModel.fit). No web view: the numbers a measure answers go in, the scale comes out.
// BROKEN COPY: each test names, in its trailing comment, the one-line change to Sources/Features/MessageBody that must turn it red.

private let repoRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
private func near(_ a: CGFloat, _ b: CGFloat) -> Bool { abs(a - b) < 0.0001 }
@MainActor private func model() -> MessageBodyModel { MessageBodyModel(content: .paragraphs(id: "wide-test", ["x"])) }

@Test func aMailOf700InAPaneOf560IsScaledTo0Point8() {
    #expect(near(WideMailFit.scale(content: 700, pane: 560), 0.8))   // broken copy: MessageBodyFit.swift `return pane / content` -> `return 1`
    #expect(near(700 * WideMailFit.scale(content: 700, pane: 560), 560))
}

@Test func aMailOf400IsShownAtItsOwnSizeNeverEnlarged() {
    #expect(WideMailFit.scale(content: 400, pane: 560) == 1)   // broken copy: MessageBodyFit.swift `guard content > 0, pane > 0, content > pane + tolerance else` -> `guard content > 0, pane > 0 else`
    #expect(WideMailFit.scale(content: 560, pane: 560) == 1)
    #expect(WideMailFit.scale(content: 0, pane: 560) == 1)
}

@MainActor @Test func theReaderHeightFollowsTheScaledHeight() {
    let m = model()
    m.fit(natural: 700, pane: 560, layoutHeight: 1000, layoutMainBottom: 500)
    #expect(m.height == 800)   // broken copy: MessageBodyView.swift `let h = WideMailFit.height(layout: layoutHeight, scale: s)` -> `let h = WideMailFit.height(layout: layoutHeight, scale: 1)`
    #expect(m.mainBottom == 400)
    let fits = model()
    fits.fit(natural: 400, pane: 560, layoutHeight: 1000, layoutMainBottom: 500)
    #expect(fits.height == 1000 && fits.scale == 1)
}

@MainActor @Test func aWidthChangeRecomputesTheScale() {
    let m = model()
    m.fit(natural: 700, pane: 560, layoutHeight: 1000, layoutMainBottom: 500)
    #expect(near(m.scale, 0.8))
    m.fit(natural: 700, pane: 700, layoutHeight: 1000, layoutMainBottom: 500)
    #expect(m.scale == 1 && m.height == 1000)   // broken copy: MessageBodyView.swift `naturalWidth = natural; paneWidth = pane` -> `naturalWidth = natural`
    m.fit(natural: 700, pane: 350, layoutHeight: 1000, layoutMainBottom: 500)
    #expect(near(m.scale, 0.5) && m.height == 500 && near(m.naturalWidth * m.scale, m.paneWidth))
}

@MainActor @Test func aZoomedWideMailScrollsSidewaysAndTheFitAloneDoesNot() {
    let m = model()
    m.fit(natural: 700, pane: 560, layoutHeight: 1000, layoutMainBottom: 500)
    #expect(!m.scrollsHorizontally)
    m.zoom = WideMailFit.zoom(after: 1, from: m.zoom, fit: m.fitScale)   // a pinch that doubles the size
    m.fit(natural: 700, pane: 560, layoutHeight: 1000, layoutMainBottom: 500)
    #expect(near(m.scale, 1.6) && m.scrollsHorizontally && m.height == 1600)   // broken copy: MessageBodyFit.swift `min(max(fit * zoom, fit), max(fit, zoomMax))` -> `fit`
    #expect(WideMailFit.zoom(after: -0.9, from: 2, fit: 0.8) == 1)   // a pinch out never goes below the fit
    #expect(WideMailFit.shown(fit: 0.8, zoom: 100) == WideMailFit.zoomMax)
    #expect(near(WideMailFit.smartZoom(from: 1, fit: 0.8), 1.25) && WideMailFit.smartZoom(from: 1.25, fit: 0.8) == 1)
}

@Test func theScaleIsAppliedInThePageWithoutAReload() {
    let s = WideMailFit.script(scale: 0.8, sideways: false)
    #expect(s.contains("scale(0.80000)") && s.contains("overflowY='hidden'") && s.contains("overflowX='hidden'"))   // broken copy: MessageBodyFit.swift `b.style.transform='scale(\(s))'` -> `b.style.transform=''`
    #expect(!s.contains("location") && !s.contains("reload") && !s.contains("src="))
    #expect(WideMailFit.script(scale: 1, sideways: false).contains("b.style.transform=''"))
    #expect(WideMailFit.script(scale: 1.6, sideways: true).contains("overflowX=''"))
}

@Test func theWideFixtureMailIsServedWiderThanThePane() throws {
    setenv("WHOOSH_FIXTURE_PATH", repoRoot.appendingPathComponent("docs/design/fixtures/mac-main-sample.json").path, 1)
    let (content, sender) = try #require(MessageBodySamples.make("wide"))   // broken copy: MessageBodySamples.swift `["designed", "written", "plain", "wide"]` -> `["designed", "written", "plain"]`
    let html = try #require(content.safeHtml)
    #expect(!sender.isEmpty && html.contains("min-width:700px") && !html.contains("<img") && content.trackersBlocked == 0)
}
