#if os(macOS)
import Foundation

/// One message's attachments as the gallery lays them out: images and videos (the gallery), then documents (compact
/// chips), each in the message's order. An image placed inline in the body (cid: or data:) joins the gallery once: a
/// cid naming a file that is also attached is that attachment, not a second tile.
public struct AttachmentSet: Equatable {
    public let messageId: String
    public let media: [AttachmentRef]
    public let documents: [AttachmentRef]
    /// The inline images among `media`, with the bytes of a data: image.
    public let inline: [AttachmentRef: Data?]
    /// A count the core gave without names ("3 attachments"); 0 when the names are known.
    public let unnamedCount: Int

    public var all: [AttachmentRef] { media + documents }
    public var isEmpty: Bool { media.isEmpty && documents.isEmpty }

    public static func make(messageId: String, names: [String], html: String?, count: Int) -> AttachmentSet {
        let files = names.enumerated().compactMap { i, w in
            AttachmentName.isFile(w) ? AttachmentRef(messageId: messageId, index: i, name: AttachmentName.split(w).file) : nil
        }
        var inline: [AttachmentRef: Data?] = [:]
        var media = files.filter { $0.kind == .image || $0.kind == .video }
        let fileNames = Set(files.map(\.name))
        for (k, src) in (AttachmentRig.mutant("no-inline") ? [] : inlineSources(html ?? "")).enumerated() {
            let ref: AttachmentRef
            let data: Data?
            if src.hasPrefix("cid:") {
                var name = String(src.dropFirst(4))
                if fileNames.contains(name) { continue }                         // the attached file itself
                if (name as NSString).pathExtension.isEmpty { name += ".png" }
                ref = AttachmentRef(messageId: messageId, index: names.count + k, name: name); data = nil
            } else {
                ref = AttachmentRef(messageId: messageId, index: names.count + k, name: Words.text("Inline image %lld", k + 1) + ".png")
                data = src.range(of: "base64,").flatMap { Data(base64Encoded: String(src[$0.upperBound...])) }
            }
            if inline.keys.contains(where: { $0.name == ref.name }) { continue }
            inline[ref] = data
            media.append(ref)
        }
        return AttachmentSet(messageId: messageId, media: media, documents: files.filter { $0.kind != .image && $0.kind != .video },
                             inline: inline, unnamedCount: files.isEmpty && inline.isEmpty ? count : 0)
    }

    /// What the core puts in place of a remote picture it blocked (parse::BLOCKED_IMAGE_SRC). It names no file of the
    /// message: a blocked tracker listed as "whoosh-blocked.png" was the owner's report of 2026-11-10 22:28.
    static let blockedImageSource = "cid:whoosh-blocked"

    /// The src of every <img> in the body that points at a cid: part or carries a data: image. A blocked remote picture
    /// is not one of them.
    static func inlineSources(_ html: String) -> [String] {
        guard !html.isEmpty, let re = try? NSRegularExpression(pattern: #"<img\b[^>]*\bsrc\s*=\s*["']((?:cid:|data:image/)[^"']+)["']"#,
                                                               options: [.caseInsensitive]) else { return [] }
        let ns = html as NSString
        return re.matches(in: html, range: NSRange(location: 0, length: ns.length)).map { ns.substring(with: $0.range(at: 1)) }
            .filter { $0.lowercased() != blockedImageSource }
    }
}

extension AttachmentSet {
    /// The set of the message the reader shows (open, or at rest on the focused row).
    @MainActor
    static func reader(_ m: AppModel) -> AttachmentSet? {
        guard let t = ReaderState.open(m) ?? ReaderFocus.shared.thread(m, resting: true) else { return nil }
        return of(t, core: m.core)
    }

    /// The set of a reader thread; the body is the core's, read here when the thread rests without one.
    @MainActor
    static func of(_ t: ReaderThread, core: WhooshCore) -> AttachmentSet {
        let html = core.body(messageId: t.messageId)?.safeHtml
        let set = make(messageId: t.messageId, names: t.attachmentNames, html: html, count: t.attachmentCount)
        AttachmentStore.shared.remember(set)
        return set
    }
}
#endif
