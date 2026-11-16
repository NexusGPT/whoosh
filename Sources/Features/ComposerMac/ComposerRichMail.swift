import Foundation
#if canImport(UniformTypeIdentifiers)
import UniformTypeIdentifiers
#endif

/// gap-composer-rich: what the composer hands the core for a signature with formatting and a picture inside the text. The core builds
/// the HTML part (core/crates/send/src/rich.rs); the composer only shows the signature rendered and writes the picture's marker.
///
/// - A signature with formatting is the account's signature text with the core's small mark set: `**bold**`, `*italic*`,
///   `[words](https://...)` (http, https, mailto), line breaks as typed. A signature with no mark is plain: shown and sent as today.
/// - A picture in the text is a picture run of the body (`RichRun.cid` = the core's attachment id of the draft's file); the outgoing
///   text writes it as `[picture:<attachment id>]`; the file itself stays on the core's draft, so a saved draft keeps both.
enum ComposerRichMail {
    /// The largest picture that goes INSIDE the text, in bytes; a larger one is attached as a file, as a drop outside the text is.
    /// The ledger's number (docs/CONSTANTS.md INLINE_PICTURE_MAX_BYTES, 10 MiB), read from the generated constant: the core's cap (send/src/rich.rs) is the same row.
    static let inlinePictureMaxBytes: UInt64 = UInt64(WhooshConstants.INLINE_PICTURE_MAX_BYTES)
    static let pictureOpen = "[picture:"

    static func marker(_ id: String) -> String { pictureOpen + id + "]" }

    enum Mark: Equatable { case plain, bold, italic, link(String) }
    struct SigRun: Equatable { let text: String; let mark: Mark }

    /// Only web and mail targets make a link (the core's rule, rich.rs `safe_url`).
    static func safeURL(_ u: String) -> Bool {
        let l = u.trimmingCharacters(in: .whitespaces).lowercased()
        guard l.hasPrefix("http://") || l.hasPrefix("https://") || l.hasPrefix("mailto:") else { return false }
        return !u.contains { $0.isWhitespace || $0 == "\"" || $0 == "<" || $0 == ">" }
    }

    /// The signature read as runs, the same reading as the core's `signature_runs`: marks never cross a line break, an unclosed mark is text.
    static func signatureRuns(_ text: String) -> [SigRun] {
        let c = Array(text)
        var runs: [SigRun] = []
        var plain = ""
        func flush() { if !plain.isEmpty { runs.append(SigRun(text: plain, mark: .plain)); plain = "" } }
        func find(_ from: Int, _ pat: [Character]) -> Int? {
            guard from <= c.count - pat.count else { return nil }
            return (from...(c.count - pat.count)).first { Array(c[$0..<$0 + pat.count]) == pat }
        }
        func inLine(_ a: Int, _ b: Int) -> Bool { b > a && !c[a..<b].contains("\n") }
        var i = 0
        while i < c.count {
            if c[i] == "*", i + 1 < c.count, c[i + 1] == "*" {
                if let end = find(i + 2, ["*", "*"]), inLine(i + 2, end) {
                    flush(); runs.append(SigRun(text: String(c[i + 2..<end]), mark: .bold)); i = end + 2; continue
                }
            } else if c[i] == "*" {
                if let end = find(i + 1, ["*"]), inLine(i + 1, end) {
                    flush(); runs.append(SigRun(text: String(c[i + 1..<end]), mark: .italic)); i = end + 1; continue
                }
            } else if c[i] == "[", let close = find(i + 1, ["]"]), close + 1 < c.count, c[close + 1] == "(", let end = find(close + 2, [")"]) {
                let url = String(c[close + 2..<end])
                if inLine(i + 1, close), safeURL(url) {
                    flush(); runs.append(SigRun(text: String(c[i + 1..<close]), mark: .link(url.trimmingCharacters(in: .whitespaces)))); i = end + 1; continue
                }
            }
            plain.append(c[i]); i += 1
        }
        flush()
        return runs
    }

    /// True when the signature carries a mark (bold, italic or a link); a plain one is shown and sent exactly as before.
    static func isRich(_ text: String) -> Bool { signatureRuns(text).contains { $0.mark != .plain } }

    /// The signature as the composer shows it under the text: bold, italic and links drawn, the marks gone.
    static func shown(_ text: String) -> AttributedString {
        var out = AttributedString()
        for r in signatureRuns(text) {
            var a = AttributedString(r.text)
            switch r.mark {
            case .plain: break
            case .bold: a.inlinePresentationIntent = .stronglyEmphasized
            case .italic: a.inlinePresentationIntent = .emphasized
            case .link(let u): a.link = URL(string: u)
            }
            out += a
        }
        return out
    }

    /// The ids of the pictures in the text, in order.
    static func pictureIds(_ d: RichDocument) -> [String] { d.paragraphs.flatMap { $0.runs.compactMap(\.cid) } }

    /// The body as the core gets it: the plain words, each picture as its marker. With no picture this is exactly `plainText`.
    static func outgoingText(_ d: RichDocument) -> String {
        guard !pictureIds(d).isEmpty else { return d.plainText }
        return d.paragraphs.map { p in p.runs.map { $0.cid.map(marker) ?? $0.text }.joined() }.joined(separator: "\n")
    }

    /// True for a file the system calls a picture (by its name's type).
    static func isPicture(_ url: URL) -> Bool {
        #if canImport(UniformTypeIdentifiers)
        return UTType(filenameExtension: url.pathExtension)?.conforms(to: .image) == true
        #else
        return ["png", "jpg", "jpeg", "gif", "heic", "tiff", "webp"].contains(url.pathExtension.lowercased())
        #endif
    }
}
