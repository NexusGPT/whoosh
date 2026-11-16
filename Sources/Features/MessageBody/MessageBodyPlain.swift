import Foundation

/// Plain-text mail as paragraphs with links and quotes (A129 part 5), never a block of raw text. The engine will send plain mail
/// as a document one day; until then this is the plain path: blank lines split paragraphs, hard-wrapped lines join, `>` runs
/// become quotes (the grey rule at the left), web and mail addresses become links. Input is escaped here; no markup passes through.
enum MessageBodyPlain {
    static func blocks(_ text: String) -> String {
        var out: [String] = []
        var para: [String] = []
        var quote: [String] = []     // the lines of the current `>` run, markers stripped
        func flushPara() {
            if !para.isEmpty { out.append("<p class=\"pt\">\(linkify(join(para)))</p>"); para = [] }
        }
        func flushQuote() {
            guard !quote.isEmpty else { return }
            let inner = split(quote).map { "<p class=\"pt\">\(linkify(join($0)))</p>" }.joined()
            if !inner.isEmpty { out.append("<blockquote>\(inner)</blockquote>") }
            quote = []
        }
        for raw in text.replacingOccurrences(of: "\r\n", with: "\n").replacingOccurrences(of: "\r", with: "\n").components(separatedBy: "\n") {
            if let stripped = unquote(raw) {
                flushPara()
                quote.append(stripped)
            } else if raw.trimmingCharacters(in: .whitespaces).isEmpty {
                flushPara(); flushQuote()
            } else {
                flushQuote()
                para.append(raw)
            }
        }
        flushPara(); flushQuote()
        return out.joined()
    }

    /// The line without its `>` markers, or nil when it is not a quote line.
    static func unquote(_ line: String) -> String? {
        let t = line.drop(while: { $0 == " " })
        guard t.first == ">" else { return nil }
        var rest = Substring(t)
        while rest.first == ">" { rest = rest.dropFirst(); if rest.first == " " { rest = rest.dropFirst() } }
        return String(rest)
    }

    /// Splits a quote's lines at its blank lines.
    private static func split(_ lines: [String]) -> [[String]] {
        var groups: [[String]] = [[]]
        for l in lines {
            if l.trimmingCharacters(in: .whitespaces).isEmpty { if !groups[groups.count - 1].isEmpty { groups.append([]) } } else { groups[groups.count - 1].append(l) }
        }
        return groups.filter { !$0.isEmpty }
    }

    /// Hard-wrapped lines (every line but the last long, none a list item or indented) join with a space; any other
    /// paragraph keeps its line breaks.
    static func join(_ lines: [String]) -> String {
        guard lines.count > 1 else { return lines.first ?? "" }
        let wrapped = lines.dropLast().allSatisfy { $0.count >= 50 } && !lines.contains { item.firstMatch(in: $0, range: NSRange($0.startIndex..., in: $0)) != nil }
        return wrapped ? lines.map { $0.trimmingCharacters(in: .whitespaces) }.joined(separator: " ") : lines.joined(separator: "\n")
    }

    private static let item = try! NSRegularExpression(pattern: "^(?:\\s{2,}|\\s*(?:[-*•]|\\d+[.)])\\s)")
    private static let link = try! NSRegularExpression(pattern: "(?:https?://|www\\.)[^\\s<>\"]+|[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+(?:\\.[A-Za-z0-9-]+)*\\.[A-Za-z]{2,}", options: .caseInsensitive)

    /// Escapes the text and wraps each web or mail address in a link. An address in angle brackets after its own words ("Read the full story <address>") folds into those
    /// words: the words are the link, the brackets and the address are not drawn. A web address longer than BODY_LINK_MAX_CHARS is drawn as a short link naming its site
    /// ("tmcnet.com link"), never its tracking code; the full address is the link's target and its tooltip (`title`).
    static func linkify(_ text: String) -> String {
        var out = "", at = text.startIndex
        for m in link.matches(in: text, range: NSRange(text.startIndex..., in: text)) {
            guard var r = Range(m.range, in: text) else { continue }
            // trailing punctuation belongs to the sentence, not the address
            while let last = text[r].last, ".,;:!?)]}'".contains(last) { r = r.lowerBound..<text.index(before: r.upperBound) }
            guard r.lowerBound >= at, !r.isEmpty else { continue }
            let shown = String(text[r])
            let lower = shown.lowercased()
            let isWeb = lower.hasPrefix("http") || lower.hasPrefix("www.")
            let href = shown.contains("@") && !lower.hasPrefix("http") ? "mailto:" + shown : (lower.hasPrefix("www.") ? "https://" + shown : shown)
            var before = String(text[at..<r.lowerBound])
            var after = r.upperBound
            var label = shown
            let bracketed = before.hasSuffix("<") && after < text.endIndex && text[after] == ">"
            if bracketed {
                before.removeLast(); after = text.index(after: after)
                if let phrase = foldablePhrase(before) { before.removeLast(phrase.count); label = phrase.trimmingCharacters(in: .whitespaces) }
                else if isWeb, shown.count > WhooshConstants.BODY_LINK_MAX_CHARS { label = siteLabel(shown) }
            } else if isWeb, shown.count > WhooshConstants.BODY_LINK_MAX_CHARS { label = siteLabel(shown) }
            out += MessageBodyDocument.escape(before)
            out += "<a href=\"\(MessageBodyDocument.escape(href))\" title=\"\(MessageBodyDocument.escape(shown))\">\(MessageBodyDocument.escape(label))</a>"
            at = after
        }
        return out + MessageBodyDocument.escape(String(text[at...]))
    }

    /// "tmcnet.com link": the address's site (host without `www.`) and the word link, in the app's language. Never the path or the query.
    static func siteLabel(_ address: String) -> String {
        let withScheme = address.lowercased().hasPrefix("www.") ? "https://" + address : address
        var host = URL(string: withScheme)?.host ?? String(address.drop(while: { $0 != "/" }).drop(while: { $0 == "/" }).prefix(while: { $0 != "/" && $0 != "?" }))
        if host.hasPrefix("www.") { host.removeFirst(4) }
        return host.isEmpty ? Words.text("Link") : "\(host) \(Words.text("Link").lowercased())"
    }

    /// The words right before an angle-bracketed address when they are clearly its label: 1 to 6 words after the last sentence end of the line, no digits, ending in a space.
    private static func foldablePhrase(_ before: String) -> String? {
        guard before.hasSuffix(" ") else { return nil }
        let line = before.split(separator: "\n", omittingEmptySubsequences: false).last.map(String.init) ?? before
        let stops: Set<Character> = [".", "!", "?", ":"]
        let tail = line.trimmingCharacters(in: .whitespaces)
        let cut = tail.lastIndex(where: { stops.contains($0) }).map { tail.index(after: $0) } ?? tail.startIndex
        let phrase = String(tail[cut...]).trimmingCharacters(in: .whitespaces)
        let words = phrase.split(separator: " ")
        guard (1...6).contains(words.count), !phrase.contains(where: \.isNumber) else { return nil }
        // the characters to take back from `before`: the phrase and the space after it
        return before.hasSuffix(phrase + " ") ? phrase + " " : nil
    }
}
