#if os(macOS)
import Foundation

/// A chip's words. The second line is always the file's kind and size (or the download's progress), short enough
/// never to be cut; what just happened (saved, copied, opened) and an act waiting for the download are a small glyph
/// whose tooltip says it in full (owner on the 88 pt shot: no truncated sentence under a file name).
@MainActor
enum AttachmentChipText {
    static var store: AttachmentStore { .shared }

    static func subtitle(_ ref: AttachmentRef, sizeWords: String? = nil) -> String {
        let phase = store.peek(ref)
        if AttachmentRig.mutant("notice-subtitle"), let n = store.notice[ref] { return n }
        if phase.isFetching, !AttachmentRig.mutant("no-wait") {
            // a figure only when the source gives one: the real core does not, and "0 %" would be a lie
            return phase.fraction.map { AttachmentWords.fetching(Int(($0 * 100).rounded())) } ?? AttachmentWords.fetchingBare
        }
        if case .unavailable(let why) = phase {
            return why == AttachmentWords.notOnThisMac || ref.isPending ? why : AttachmentWords.notFetched(why)
        }
        if store.isAbsent(ref) { return AttachmentWords.notOnThisMac }
        let kind = (ref.name as NSString).pathExtension.uppercased()
        let size = sizeWords ?? phase.url.flatMap { u in
            (try? u.resourceValues(forKeys: [.fileSizeKey]).fileSize).map { AttachmentSize.words(Int64($0)) }
        }
        return [kind, size].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: " · ")
    }

    /// The one grey line turns warn-coloured when the fetch failed.
    static func failed(_ ref: AttachmentRef) -> Bool {
        if case .unavailable(let why) = store.peek(ref), !ref.isPending, why != AttachmentWords.notOnThisMac { return true }
        return false
    }

    /// The glyph after the name and the words of its tooltip, or nil when nothing is waiting or just happened.
    static func badge(_ ref: AttachmentRef) -> (symbol: String, words: String)? {
        if let w = store.waiting[ref] {
            return ("hourglass", w == .quickLook ? AttachmentWords.waitingQuickLook : AttachmentWords.waitingOpen)
        }
        guard let n = store.notice[ref] else { return nil }
        if store.saved[ref] != nil, n == AttachmentWords.saved(store.saved[ref]?.deletingLastPathComponent().lastPathComponent ?? "") {
            return ("checkmark.circle", n)
        }
        if n == AttachmentWords.copied { return ("doc.on.doc", n) }
        return ("arrow.up.forward.app", n)
    }
}
#endif
