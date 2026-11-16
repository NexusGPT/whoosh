import Foundation

/// gap-composer-rich: the test engine's record of the last mail the composer queued, for the fields of a signature with formatting and
/// a picture in the text. It reads the outbox entry the send made (its draft and the files that moved onto it) and the account's
/// signature, and answers what the real core's builder (core/crates/send/src/rich.rs `rich_texts`) makes of them: the HTML part (nil when
/// the mail needs none, as the core's plain path), the plain part, and how many inline parts and attachments it has. A model of the core
/// for the socket checks (checks/parity/par-composerrich.py); the core's own tests (core/crates/send/tests/rich_body.rs) prove the core.
///
/// Paths: `sent/last/html` (a JSON string, or no core value when the mail has no HTML part), `sent/last/plain`, `sent/last/inline_parts`,
/// `sent/last/attachments`, `sent/last/body` (the body as the composer handed it, markers included).
extension FixtureCore {
    /// The signatures the composer's rig verbs set (`mac-composer.dev.rich-signature`, `.plain-signature`): sample data, never shown as a sentence.
    static let rigRichSignature = "Assem\n**Whoosh** studio, *Paris*\n[whoosh.example](https://whoosh.example)"
    static let rigPlainSignature = "Assem\nWhoosh studio"

    struct RichSent { let body: String; let plain: String; let html: String?; let inline: Int; let attachments: Int }

    /// The record of the newest outbox entry. Called with the lock held.
    func richSentLocked() -> RichSent? {
        guard let e = entries.last else { return nil }
        let d = e.draft
        let files = draftFileRows[e.id] ?? draftFileRows[d.id] ?? []
        let sig = (accountSignatures[d.fromAccountId] ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        let marked = Self.richMarkedIds(d.body)
        let inline = files.filter { marked.contains($0.id) && $0.mediaType.lowercased().hasPrefix("image/") }
        let others = files.count - inline.count
        guard ComposerRichMail.isRich(sig) || !inline.isEmpty else {
            return RichSent(body: d.body, plain: d.body, html: nil, inline: 0, attachments: others)
        }
        let text = Self.richStrip(d.body, sig)
        let runs = ComposerRichMail.signatureRuns(sig)
        func body(_ html: Bool) -> String {
            var out = "", rest = Substring(text)
            while let open = rest.range(of: ComposerRichMail.pictureOpen) {
                let after = rest[open.upperBound...]
                if let close = after.firstIndex(of: "]"), let f = inline.first(where: { $0.id == String(after[..<close]) }) {
                    let before = String(rest[..<open.lowerBound])
                    out += html ? Self.richEscape(before) : before
                    out += html ? "<img src=\"cid:pic-\(Self.richEscape(f.id))\" alt=\"\(Self.richEscape(f.fileName))\">" : "[picture: \(f.fileName)]"
                    rest = after[after.index(after: close)...]
                } else {
                    let upto = String(rest[..<open.upperBound])
                    out += html ? Self.richEscape(upto) : upto
                    rest = after
                }
            }
            out += html ? Self.richEscape(String(rest)) : String(rest)
            return out
        }
        var plain = body(false)
        var html = "<!DOCTYPE html><html><body><div>" + body(true) + "</div>"
        if !sig.isEmpty {
            plain += "\n\n-- \n" + runs.map { r in if case .link(let u) = r.mark, u != r.text { return "\(r.text) <\(u)>" }; return r.text }.joined()
            html += "<br><div>-- <br>" + runs.map { r -> String in
                switch r.mark {
                case .plain: return Self.richEscape(r.text)
                case .bold: return "<b>" + Self.richEscape(r.text) + "</b>"
                case .italic: return "<i>" + Self.richEscape(r.text) + "</i>"
                case .link(let u): return "<a href=\"" + Self.richEscape(u) + "\">" + Self.richEscape(r.text) + "</a>"
                }
            }.joined() + "</div>"
        }
        html += "</body></html>"
        return RichSent(body: d.body, plain: plain, html: html, inline: inline.count, attachments: others)
    }

    static func richMarkedIds(_ body: String) -> Set<String> {
        var out = Set<String>(), rest = Substring(body)
        while let open = rest.range(of: ComposerRichMail.pictureOpen) {
            let after = rest[open.upperBound...]
            guard let close = after.firstIndex(of: "]") else { break }
            out.insert(String(after[..<close]))
            rest = after[after.index(after: close)...]
        }
        return out
    }

    /// The body without the signature the composer appended (the core's `strip_signature`).
    static func richStrip(_ body: String, _ sig: String) -> String {
        var b = body
        while b.last?.isWhitespace == true { b.removeLast() }
        guard !sig.isEmpty else { return b }
        for tail in ["-- \n" + sig, sig] where b.hasSuffix(tail) {
            let head = String(b.dropLast(tail.count))
            if tail != sig || head.isEmpty || head.hasSuffix("\n") {
                var h = head
                while h.last?.isWhitespace == true { h.removeLast() }
                return h
            }
        }
        return b
    }

    static func richEscape(_ s: String) -> String {
        var o = ""
        for ch in s {
            switch ch {
            case "&": o += "&amp;"
            case "<": o += "&lt;"
            case ">": o += "&gt;"
            case "\"": o += "&quot;"
            case "'": o += "&#39;"
            case "\n": o += "<br>"
            default: o.append(ch)
            }
        }
        return o
    }

    /// nil = not one of these paths.
    func stateRichServed(_ path: String) -> TestsServed? {
        guard path.hasPrefix("sent/last/") else { return nil }
        func json(_ v: Any) -> String? { (try? JSONSerialization.data(withJSONObject: v, options: [.fragmentsAllowed, .withoutEscapingSlashes])).flatMap { String(data: $0, encoding: .utf8) } }
        return TestsServed(value: lock.withLock { () -> String? in
            guard let r = richSentLocked() else { return nil }
            switch path {
            case "sent/last/html": return r.html.flatMap { json($0) }
            case "sent/last/plain": return json(r.plain)
            case "sent/last/body": return json(r.body)
            case "sent/last/inline_parts": return json(r.inline)
            case "sent/last/attachments": return json(r.attachments)
            default: return nil
            }
        })
    }
}
