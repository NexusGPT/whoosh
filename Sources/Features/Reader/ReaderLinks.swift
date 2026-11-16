import SwiftUI
import Observation
#if canImport(AppKit)
import AppKit
#endif

/// One link or date in a message, as Tab cycles through them.
struct ReaderLinkItem: Equatable {
    enum Kind: String { case link, date }
    let kind: Kind
    let text: String
    let url: URL?
}

/// One thing ⌘O or Enter opened. `done` is false where the action is only logged: the fixture never starts a browser,
/// and no attachment file reaches the reader yet (the core serves names, not bytes), so Quick Look is logged, not run.
struct ReaderOpened: Equatable {
    let n: Int
    let kind: String
    let target: String
    let messageId: String
    let done: Bool
}

/// The links and dates of the focused message (Tab), the ring on one of them, and the log of what was opened.
@MainActor @Observable
final class ReaderLinks {
    static let shared = ReaderLinks()

    private(set) var messageId: String?
    private(set) var items: [ReaderLinkItem] = []
    /// The item holding the ring; nil while the ring is off.
    private(set) var cursor: Int?
    private(set) var opened: [ReaderOpened] = []

    /// Written only when it changes (perf-archive r3): an equal write still marks the views that read the ring changed.
    func reset() {
        if messageId != nil { messageId = nil }
        if !items.isEmpty { items = [] }
        if cursor != nil { cursor = nil }
    }

    /// Tab: the ring goes to the next link or date of the message, then off after the last one. False when the
    /// message has none, so the key passes on to the next split.
    func cycle(_ m: AppModel, messageId id: String) -> Bool {
        load(m, messageId: id)
        guard !items.isEmpty else { return false }
        cursor = cursor.map { $0 + 1 < items.count ? $0 + 1 : nil } ?? 0
        return true
    }

    /// ⇧Tab: the ring goes to the item before, then off before the first one; from off it lands on the last. The same
    /// walk as Tab, backwards. False when the message has none.
    func cycleBack(_ m: AppModel, messageId id: String) -> Bool {
        load(m, messageId: id)
        guard !items.isEmpty else { return false }
        cursor = cursor.map { $0 > 0 ? $0 - 1 : nil } ?? items.count - 1
        return true
    }

    /// The links and dates of the message, found once: a second key on the same message (Tab, ⇧Tab, ⌘O) reads no body.
    /// Another message drops the ring, which belongs to one message at a time.
    private func load(_ m: AppModel, messageId id: String) {
        if messageId != id { reset(); messageId = id; items = Self.find(m, messageId: id) }
    }

    /// Enter with the ring on, or a click on a chip: a real link opens in the default browser; a date is logged
    /// (the calendar is not built).
    func openCursor(_ m: AppModel, at index: Int? = nil) -> Bool {
        if let index, items.indices.contains(index) { cursor = index }
        guard let c = cursor, items.indices.contains(c), let id = messageId else { return false }
        let item = items[c]
        guard item.kind == .link, let url = item.url else { log("calendar", item.text, id, done: false); return true }
        log("browser", url.absoluteString, id, done: launch(url))
        return true
    }

    /// ⌘O: the first link in the browser, every attachment to Quick Look. False when the message has neither.
    func openAll(_ m: AppModel, messageId id: String, attachments: [String]) -> Bool {
        load(m, messageId: id)
        let first = items.first { $0.kind == .link }
        if let url = first?.url { log("browser", url.absoluteString, id, done: launch(url)) }
        for name in attachments { log("quicklook", name, id, done: false) }
        return first != nil || !attachments.isEmpty
    }

    private func log(_ kind: String, _ target: String, _ id: String, done: Bool) {
        opened.append(ReaderOpened(n: opened.count + 1, kind: kind, target: target, messageId: id, done: done))
    }

    private func launch(_ url: URL) -> Bool {
        #if canImport(AppKit)
        guard CoreHost.shared.kind != .fixture else { return false }
        return NSWorkspace.shared.open(url)
        #else
        return false
        #endif
    }

    private static let linkPattern = try! NSRegularExpression(pattern: #"https?://[^\s<>"')\]]+"#)
    private static let hrefPattern = try! NSRegularExpression(pattern: #"href\s*=\s*"(https?://[^"]+)""#, options: .caseInsensitive)
    private static let months = "January|February|March|April|May|June|July|August|September|October|November|December|Jan|Feb|Mar|Apr|Jun|Jul|Aug|Sept|Sep|Oct|Nov|Dec"
    private static let datePattern = try! NSRegularExpression(
        pattern: #"\b(?:\d{1,2}(?:st|nd|rd|th)?\s+(?:"# + ReaderLinks.months + #")(?:\s+\d{4})?|Monday|Tuesday|Wednesday|Thursday|Friday|Saturday|Sunday)\b"#)

    /// The links (http and https, trailing punctuation dropped) and dates (a weekday name, or a day and a month) of a
    /// message body in reading order; links only in the HTML (a hidden href) follow.
    static func find(_ m: AppModel, messageId: String) -> [ReaderLinkItem] {
        guard let body = ReaderBodies.shared.body(m, messageId) else { return [] }
        return find(plain: body.plainText, html: body.safeHtml)
    }

    static func find(plain: String, html: String?) -> [ReaderLinkItem] {
        let ns = plain as NSString
        let whole = NSRange(location: 0, length: ns.length)
        var found: [(at: Int, item: ReaderLinkItem)] = []
        for r in linkPattern.matches(in: plain, range: whole) {
            let text = trimmed(ns.substring(with: r.range))
            found.append((r.range.location, ReaderLinkItem(kind: .link, text: text, url: URL(string: text))))
        }
        for r in datePattern.matches(in: plain, range: whole) {
            found.append((r.range.location, ReaderLinkItem(kind: .date, text: ns.substring(with: r.range), url: nil)))
        }
        var items = found.sorted { $0.at < $1.at }.map(\.item)
        if let html {
            let h = html as NSString
            for r in hrefPattern.matches(in: html, range: NSRange(location: 0, length: h.length)) where r.numberOfRanges > 1 {
                let text = trimmed(h.substring(with: r.range(at: 1)))
                if !items.contains(where: { $0.url?.absoluteString == text }) { items.append(ReaderLinkItem(kind: .link, text: text, url: URL(string: text))) }
            }
        }
        return items
    }

    private static func trimmed(_ s: String) -> String {
        var t = Substring(s)
        while let last = t.last, ".,;:!?".contains(last) { t = t.dropLast() }
        return String(t)
    }
}
