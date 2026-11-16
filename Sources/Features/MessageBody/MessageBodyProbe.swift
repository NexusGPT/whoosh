#if MESSAGEBODY_PROBE
// Headless proof of the body view's four promises, until Tests/Unit/MessageBodyTests.swift (tests-j1) lands.
// Compiled only with `swift test -Xswiftc -DMESSAGEBODY_PROBE`; never part of the shipped build.
// Run: _captures/messagebody/probe-green.txt. Each control inside a test is the sensor's own mutant:
// the same page with the protection removed must show the leak, or the zero above it proves nothing.
#if os(macOS)
import AppKit
import Foundation
import SwiftUI
import Testing
import WebKit

/// A bare delegate with no policy: the mutant's world, where nothing forces script off.
@MainActor private final class PlainNav: NSObject, WKNavigationDelegate {
    var onFinish: () -> Void = {}
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) { onFinish() }
}

@MainActor
private func loadRaw(_ html: String, rules: Bool, guardOn: Bool = true, config: WKWebViewConfiguration = MessageBodyPolicy.makeConfiguration()) async -> WKWebView {
    let view = WKWebView(frame: NSRect(x: 0, y: 0, width: 600, height: 400), configuration: config)
    if rules { _ = MessageBodyPolicy.install(rules: await MessageBodyPolicy.ruleList(), on: view) }
    let nav = BodyNavigationGuard()
    if guardOn { view.navigationDelegate = nav }
    let finish = PlainNav()
    if !guardOn { view.navigationDelegate = finish }
    await withCheckedContinuation { (k: CheckedContinuation<Void, Never>) in
        nav.onFinish = { k.resume() }
        finish.onFinish = { k.resume() }
        view.loadHTMLString(html, baseURL: nil)
    }
    withExtendedLifetime((nav, finish)) {}
    try? await Task.sleep(nanoseconds: 400_000_000)   // let any subresource request show up
    return view
}

/// A local TCP server that counts every connection made to it (plain POSIX sockets, 127.0.0.1, random port).
private final class Sensor: @unchecked Sendable {
    private let lock = NSLock()
    private var n = 0
    private let fd: Int32
    let port: UInt16
    var hits: Int { lock.withLock { n } }
    init() throws {
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        self.fd = fd
        var addr = sockaddr_in()
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_addr.s_addr = inet_addr("127.0.0.1")
        addr.sin_port = 0
        let ok = withUnsafePointer(to: &addr) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) } }
        guard ok == 0, listen(fd, 16) == 0 else { throw POSIXError(.EADDRINUSE) }
        var bound = sockaddr_in(); var len = socklen_t(MemoryLayout<sockaddr_in>.size)
        _ = withUnsafeMutablePointer(to: &bound) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { getsockname(fd, $0, &len) } }
        port = UInt16(bigEndian: bound.sin_port)
        Thread.detachNewThread { [weak self] in
            while true {
                let c = accept(fd, nil, nil)
                if c < 0 { return }
                self?.lock.withLock { self?.n += 1 }
                close(c)
            }
        }
    }
    func start() async {}
    deinit { close(fd) }
}

private func leakyPage(_ port: UInt16) -> String {
    let u = "http://127.0.0.1:\(port)"
    return "<html><head><link rel=stylesheet href=\"\(u)/a.css\"></head><body style=\"background:url(\(u)/bg.png)\"><img src=\"\(u)/t.gif\"><p>hello</p></body></html>"
}

@MainActor @Test func pageScriptNeverRunsAndThePreferenceReadsBackFalse() async throws {
    let view = await loadRaw("<html><body><script>document.title='ran'</script><p>x</p></body></html>", rules: false)
    #expect(MessageBodyPolicy.javaScriptAllowed(in: view) == false)
    #expect(try await view.evaluateJavaScript("document.title") as? String == "")        // page script did not run
    #expect(try await view.evaluateJavaScript("document.body.getBoundingClientRect().height") as? Double != nil) // the app's own script does
    // mutant: JavaScript on
    let on = MessageBodyPolicy.makeConfiguration()
    on.defaultWebpagePreferences.allowsContentJavaScript = true
    let mutant = await loadRaw("<html><body><script>document.title='ran'</script></body></html>", rules: false, guardOn: false, config: on)
    #expect(MessageBodyPolicy.javaScriptAllowed(in: mutant) == true)
    #expect(try await mutant.evaluateJavaScript("document.title") as? String == "ran")
}

@MainActor @Test func noRequestLeavesTheViewAndTheSensorSeesALeakWithoutTheList() async throws {
    let blocked = try Sensor(); await blocked.start()
    _ = await loadRaw(leakyPage(blocked.port), rules: true)
    #expect(blocked.hits == 0)
    let open = try Sensor(); await open.start()
    _ = await loadRaw(leakyPage(open.port), rules: false)
    #expect(open.hits > 0)       // control: the sensor works, so the zero above means something
}

@MainActor @Test func theDocumentItselfBlocksNetworkEvenIfTheListFails() async throws {
    let s = try Sensor(); await s.start()
    let raw = leakyPage(s.port).replacingOccurrences(of: "<img", with: "<img onerror=x")
    let content = MessageBodyContent.sanitised(id: "m", html: raw, trackersBlocked: 1, quoteOffset: nil)
    _ = await loadRaw(MessageBodyDocument.html(for: content, quotedOpen: false, scheme: .light), rules: false)
    #expect(s.hits == 0)         // CSP alone, no rule list
    #expect(!MessageBodyDocument.html(for: content, quotedOpen: false, scheme: .light).contains("<img"))
}

@Test func darkBodyTextIsReadable() {
    let p = WhooshPalette.self
    let text = p.label.dark, ground = p.background.dark
    #expect(RGBA.contrast(text, ground) >= MessageBodyMetrics.minimumContrast)
    #expect(RGBA.contrast(p.secondaryLabel.dark, ground) >= MessageBodyMetrics.minimumContrast)
    #expect(RGBA.contrast(p.accent.dark, ground) >= MessageBodyMetrics.minimumContrast)
}

@MainActor @Test func hostileColoursInMailCannotMakeDarkTextUnreadable() async throws {
    let raw = "<p>plain</p><p><font color=\"#111111\" style=\"color:#111;background:#fff\">dark on dark</font></p>"
    let content = MessageBodyContent.sanitised(id: "m", html: raw, trackersBlocked: 0, quoteOffset: nil)
    let view = await loadRaw(MessageBodyDocument.html(for: content, quotedOpen: false, scheme: .dark), rules: false)
    let rgb = try await view.evaluateJavaScript("getComputedStyle(document.querySelector('font')).color") as? String
    let want = WhooshPalette.label.dark
    #expect(rgb == "rgb(\(Int((want.red * 255).rounded())), \(Int((want.green * 255).rounded())), \(Int((want.blue * 255).rounded())))")
    let bg = try await view.evaluateJavaScript("getComputedStyle(document.querySelector('font')).backgroundColor") as? String
    #expect(bg == "rgba(0, 0, 0, 0)")
}

@Test func quotedTextIsFoldedUntilAsked() {
    let html = "<p>Thanks, see below.</p><div class=\"gmail_quote\"><p>On Monday Karen wrote: OLDTEXT</p></div>"
    let off = html.utf8.count - "<div class=\"gmail_quote\"><p>On Monday Karen wrote: OLDTEXT</p></div>".utf8.count
    let c = MessageBodyContent.sanitised(id: "m", html: html, trackersBlocked: 0, quoteOffset: off)
    #expect(c.hasQuote)
    #expect(MessageBodyDocument.html(for: c, quotedOpen: false, scheme: .light).contains("id=\"q\" hidden"))   // in the page, hidden
    #expect(!MessageBodyDocument.html(for: c, quotedOpen: true, scheme: .light).contains("id=\"q\" hidden"))
    #expect(c.visibleParagraphs(quotedOpen: false) == ["Thanks, see below."])
    // a quote with no text before it is not folded away (the parse crate returns no offset then; a bad offset is ignored)
    #expect(!MessageBodyContent.sanitised(id: "m", html: html, trackersBlocked: 0, quoteOffset: 0).hasQuote)
    #expect(!MessageBodyContent.sanitised(id: "m", html: html, trackersBlocked: 0, quoteOffset: 9_999).hasQuote)
}

@Test func plainTextFallbackEscapesAndKeepsParagraphs() {
    let c = MessageBodyContent.paragraphs(id: "m", ["Hi Assem,", "1 < 2 & <script>x</script>"], trackersBlocked: 2)
    let doc = MessageBodyDocument.html(for: c, quotedOpen: false, scheme: .light)
    #expect(doc.contains("<p class=\"pt\">Hi Assem,</p>"))
    #expect(!doc.contains("<script>x") && doc.contains("&lt;script&gt;"))
    #expect(c.visibleParagraphs(quotedOpen: false) == ["Hi Assem,", "1 < 2 & <script>x</script>"])
    #expect(c.blockedLabel == MessageBodyWords.picturesLine(pictures: 2, trackers: 0))
    #expect(MessageBodyContent.paragraphs(id: "m", ["x"], trackersBlocked: 1).blockedLabel == MessageBodyWords.picturesLine(pictures: 1, trackers: 0))
    #expect(MessageBodyContent.paragraphs(id: "m", ["x"]).blockedLabel == nil)
    // an HTML part that sanitised to nothing falls back to the text part
    #expect(MessageBodyContent(id: "m", safeHtml: "  ", plainText: "text part", trackersBlocked: 0, quoteOffset: nil).safeHtml == nil)
}

@MainActor @Test func theHostedViewShowsBlockedCountMeasuresItselfAndEmitsShots() async throws {
    let html = "<p>Hi Assem,</p><p>The unit price at 12,000 is 0.84 EUR (10,080 EUR total, VAT excluded), delivery in week 4 to the Düsseldorf warehouse. Let me know by Tuesday so we can hold the press slot.</p><p><img src=\"cid:whoosh-blocked\"></p><p>Best,<br>Karen Bosch, Linea Supply <a href=\"https://example.com\">example.com</a></p><div class=\"gmail_quote\"><p>On Mon, Assem wrote: Could you quote 12,000 units?</p></div>"
    let off = html.utf8.count - "<div class=\"gmail_quote\"><p>On Mon, Assem wrote: Could you quote 12,000 units?</p></div>".utf8.count
    let content = MessageBodyContent.sanitised(id: "msg-04", html: html, trackersBlocked: 2, quoteOffset: off)
    for scheme in [ColorScheme.light, .dark] {
        let host = NSHostingView(rootView: MessageBodyView(content: content).environment(\.colorScheme, scheme)
            .padding(32).frame(width: 640).background(WhooshPalette.background.color(scheme)))
        host.frame = NSRect(x: 0, y: 0, width: 640, height: 420)
        host.appearance = NSAppearance(named: scheme == .dark ? .darkAqua : .aqua)
        let window = NSWindow(contentRect: host.frame, styleMask: .borderless, backing: .buffered, defer: false)
        window.contentView = host
        let before = MessageBodyProbe.shared.current
        for _ in 0..<60 {
            if let m = MessageBodyProbe.shared.current, m !== before, m.blockListInstalled, m.height > 30 { break }
            try await Task.sleep(nanoseconds: 100_000_000)
        }
        let m = try #require(MessageBodyProbe.shared.current)
        #expect(m.javaScriptAllowed == false && m.blockListInstalled && m.height > 30)
        #expect(m.content.blockedLabel == MessageBodyWords.picturesLine(pictures: 2, trackers: 0) && m.quotedOpen == false)
        if let dir = ProcessInfo.processInfo.environment["WHOOSH_CAPTURE_DIR"], let web = firstWebView(host) {
            let img = try await web.takeSnapshot(configuration: nil)
            if let tiff = img.tiffRepresentation, let rep = NSBitmapImageRep(data: tiff), let png = rep.representation(using: .png, properties: [:]) {
                try png.write(to: URL(fileURLWithPath: dir).appendingPathComponent("body-\(scheme == .dark ? "dark" : "light")-folded.png"))
            }
            m.quotedOpen = true
            try await Task.sleep(nanoseconds: 800_000_000)
            let open = try await web.takeSnapshot(configuration: nil)
            if let tiff = open.tiffRepresentation, let rep = NSBitmapImageRep(data: tiff), let png = rep.representation(using: .png, properties: [:]) {
                try png.write(to: URL(fileURLWithPath: dir).appendingPathComponent("body-\(scheme == .dark ? "dark" : "light")-quoted-open.png"))
            }
        }
        window.contentView = nil
    }
}

@MainActor private func firstWebView(_ v: NSView) -> WKWebView? {
    if let w = v as? WKWebView { return w }
    for s in v.subviews { if let w = firstWebView(s) { return w } }
    return nil
}
#endif
#endif
