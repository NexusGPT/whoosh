import WebKit

/// The rules the web view runs under. Pattern: a plain `WKWebView` with the stock `loadHTMLString`
/// path (Apple's WebKit sample for hosting local HTML), hardened three ways so a single failure of the
/// parse crate's sanitiser still leaks nothing: (1) JavaScript off for page content, (2) a content rule
/// list that blocks every network scheme, (3) a Content-Security-Policy in the document itself.
@MainActor
public enum MessageBodyPolicy {
    /// The drawn facts of the page, as the web view computed them (a test copy only; six values joined by `|`). Read by MessageBodyView.
    static let drawnScript = "(function(){var d=document,c=function(e){return e?getComputedStyle(e):null},b=c(d.body),k=c(d.querySelector('.blocked:not(.mark)')),a=c(d.querySelector('.ad')),o=c(d.querySelector('.od')),q=d.getElementById('q'),e=q?(q.querySelector('blockquote')||q):null,n=0;for(;e&&e!==d.body;e=e.parentElement){if(parseFloat(c(e).borderLeftWidth)>0)n++}return [d.documentElement.className,b.paddingLeft,k?k.backgroundColor:'',a?a.display:'',o?o.display:'',b.backgroundColor,n].join('|')})()"

    /// Every scheme that could leave this Mac. `about:` (the document itself), `cid:` and `data:` stay allowed.
    static let blockedSchemes = ["https?", "wss?", "ftp"]

    static var ruleListJSON: String {
        let rules = blockedSchemes.map { #"{"trigger":{"url-filter":"^\#($0)://"},"action":{"type":"block"}}"# }
        return "[" + rules.joined(separator: ",") + "]"
    }

    /// "Load once" / "Always" (A129 part 3): the list of ONE document whose pictures the owner asked for. Images over https may load;
    /// every other resource over every scheme, and every ws, wss or ftp request, stays blocked. The page still runs no script.
    /// The sanitiser keeps an https picture's address as `data-whoosh-src` (core/crates/parse/src/sanitise.rs:11), so once asked the page
    /// asks the sender's server for it. On a test copy (`RigNet`) only a picture on this Mac may load: every other http(s) picture is
    /// blocked here too, and `MessageBodyDocument` never writes its address into the page (rig network audit, gap 3).
    static var picturesRuleListJSON: String { picturesRuleListJSON(rig: RigNet.onRig) }

    nonisolated static func picturesRuleListJSON(rig: Bool) -> String {
        let other = #"{"trigger":{"url-filter":"^https?://","resource-type":["document","style-sheet","script","font","raw","svg-document","media","popup"]},"action":{"type":"block"}}"#
        let sockets = ["wss?", "ftp"].map { #"{"trigger":{"url-filter":"^\#($0)://"},"action":{"type":"block"}}"# }
        guard rig else { return "[" + ([other] + sockets).joined(separator: ",") + "]" }
        // WebKit's rule regex has no alternation: one block for every http(s) address, then one exception per name of this Mac, pictures only.
        let all = #"{"trigger":{"url-filter":"^https?://"},"action":{"type":"block"}}"#
        let thisMac = [#"127\\.0\\.0\\.1"#, "localhost", ".::1."].map {
            #"{"trigger":{"url-filter":"^https?://\#($0)[:/]","resource-type":["image"]},"action":{"type":"ignore-previous-rules"}}"#
        }
        return "[" + ([other] + sockets + [all] + thisMac).joined(separator: ",") + "]"
    }

    static func picturesRuleList() async -> WKContentRuleList? {
        await obtain(RigNet.onRig ? "whoosh.messagebody.pictures-once.rig" : "whoosh.messagebody.pictures-once", picturesRuleListJSON, isDefault: false)
    }

    /// The block list: ONE result per app run, shared by every body view (none compiles). Looked up in WebKit's store first; compiled only when the stored one is
    /// missing or its source changed (the identifier carries a hash of the rules, so a new rule set is a new identifier). The store is per container: an installed
    /// app keeps the compiled list between launches, so only the first launch after an install (or after the rules change) compiles.
    static func ruleList() async -> WKContentRuleList? {
        await obtain("whoosh.messagebody.no-network", ruleListJSON, isDefault: true)
    }

    /// What the picture tool prints beside the web view's state: `rules: looked up | compiled in N ms | not ready | failed`.
    static var rulesNote = "rules: not ready"
    private static var shared: [String: Task<WKContentRuleList?, Never>] = [:]

    /// Starts the one lookup or compile at launch (after the first frame: the warm-up of the body views calls it); a body that opens first awaits the same task.
    static func startRules() { Task { _ = await ruleList() } }

    private static func stableHash(_ s: String) -> String {
        var h: UInt64 = 5381
        for b in s.utf8 { h = (h &* 33) &+ UInt64(b) }
        return String(h, radix: 16)
    }

    private static func obtain(_ baseId: String, _ json: String, isDefault: Bool) async -> WKContentRuleList? {
        if let t = shared[baseId] { return await t.value }
        let id = baseId + "." + stableHash(json)
        let t = Task { @MainActor () -> WKContentRuleList? in
            let store = WKContentRuleListStore.default()
            let t0 = Date()
            if let list = try? await store?.contentRuleList(forIdentifier: id) {   // the async name of lookUpContentRuleList(forIdentifier:completionHandler:)
                if isDefault { rulesNote = "rules: looked up" }
                return list
            }
            let list = try? await store?.compileContentRuleList(forIdentifier: id, encodedContentRuleList: json)
            if isDefault { rulesNote = list == nil ? "rules: failed" : "rules: compiled in \(Int(Date().timeIntervalSince(t0) * 1000)) ms" }
            return list
        }
        shared[baseId] = t
        let list = await t.value
        if list == nil { shared[baseId] = nil }   // a failure is asked again by the next body, never kept
        return list
    }

    /// The one store of every mail view: in memory only, nothing on disk. Shared, so a picture fetched for one mail is in the cache for the next view of
    /// it and for the sender's next mail while the app runs (each view had its own empty store: every open fetched every picture again; owner 13 Nov,
    /// "difficult to load").
    static let pictureStore = WKWebsiteDataStore.nonPersistent()

    /// A configuration that cannot run script and keeps nothing on disk. The block list is added by
    /// `install(rules:on:)` before the first load.
    public static func makeConfiguration() -> WKWebViewConfiguration {
        let c = WKWebViewConfiguration()
        c.defaultWebpagePreferences.allowsContentJavaScript = false
        c.websiteDataStore = pictureStore
        c.suppressesIncrementalRendering = false
        return c
    }

    /// The preference as the live view reports it, the value the check reads back.
    public static func javaScriptAllowed(in view: WKWebView) -> Bool {
        view.configuration.defaultWebpagePreferences.allowsContentJavaScript
    }

    static func install(rules: WKContentRuleList?, on view: WKWebView) -> Bool {
        guard let rules else { return false }
        view.configuration.userContentController.add(rules)
        return true
    }

    /// Replaces whatever list the view carries with `rules` (the one document's list, or the default one back).
    static func replace(rules: WKContentRuleList?, on view: WKWebView) -> Bool {
        view.configuration.userContentController.removeAllContentRuleLists()
        return install(rules: rules, on: view)
    }

    /// The CSP of the rendered document: nothing loads except inline style and `cid:`/`data:` images.
    nonisolated static let contentSecurityPolicy = "default-src 'none'; img-src cid: data:; style-src 'unsafe-inline'"
    /// The same for the one document whose pictures were asked for: images over https too, nothing else.
    nonisolated static let picturesContentSecurityPolicy = "default-src 'none'; img-src cid: data: https:; style-src 'unsafe-inline'"
    /// The same on a test copy: images over https from this Mac only.
    nonisolated static let rigPicturesContentSecurityPolicy = "default-src 'none'; img-src cid: data: https://127.0.0.1:* https://localhost:* http://127.0.0.1:* http://localhost:*; style-src 'unsafe-inline'"
}

/// A link click never navigates the view; the host decides whether to open it.
@MainActor
final class BodyNavigationGuard: NSObject, WKNavigationDelegate {
    var onOpenLink: (URL) -> Void = { _ in }
    var onFinish: () -> Void = {}
    var onTerminate: () -> Void = {}

    func webView(_ webView: WKWebView, decidePolicyFor action: WKNavigationAction,
                 preferences: WKWebpagePreferences) async -> (WKNavigationActionPolicy, WKWebpagePreferences) {
        preferences.allowsContentJavaScript = false
        if action.navigationType == .linkActivated {
            if let url = action.request.url, ["http", "https", "mailto", "tel"].contains(url.scheme?.lowercased() ?? "") {
                onOpenLink(url)
            }
            return (.cancel, preferences)
        }
        // Only the document we loaded ourselves (about:blank / loadHTMLString) may navigate.
        let scheme = action.request.url?.scheme?.lowercased() ?? "about"
        return (["about", "data"].contains(scheme) ? .allow : .cancel, preferences)
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) { onFinish() }
    func webViewWebContentProcessDidTerminate(_ webView: WKWebView) { onTerminate() }
    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) { onFinish() }
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) { onFinish() }
}
