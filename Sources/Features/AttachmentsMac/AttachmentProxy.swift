#if os(macOS)
import SwiftUI

/// The one mount the reader and the composer make (their hook commits): a tile that names a file becomes a real file
/// proxy (Quick Look on Space, open on double-click, Return and ⌘O, drag out, the right-click menu, a real thumbnail,
/// the download's progress); a tile that only counts attachments ("3 attachments") keeps the owner's own drawing.
public struct AttachmentProxy<Fallback: View>: View {
    private let ref: AttachmentRef?
    private let sizeWords: String?
    private let onRemove: (() -> Void)?
    private let fallback: Fallback

    /// The reader: the open message's tile `index`, whose words are the file name (and its size in brackets).
    public init(readerWords words: String, index: Int, @ViewBuilder fallback: () -> Fallback) {
        let split = AttachmentName.split(words)
        let message = AppModel.shared.shownMessageId
        ref = AttachmentName.isFile(words) && message != nil ? AttachmentRef(messageId: message ?? "", index: index, name: split.file) : nil
        sizeWords = split.size.map { "\((split.file as NSString).pathExtension.uppercased()) · \($0)" }
        onRemove = nil
        self.fallback = fallback()
    }

    /// The composer: pending file `index`, with its remove button.
    public init(pendingName name: String, index: Int, remove: @escaping () -> Void, @ViewBuilder fallback: () -> Fallback) {
        ref = AttachmentRef(messageId: AttachmentRef.composer, index: index, name: name)
        sizeWords = nil
        onRemove = remove
        self.fallback = fallback()
    }

    public var body: some View {
        if let ref {
            AttachmentChip(ref: ref, sizeWords: sizeWords, onRemove: onRemove)
                .whooshAx(AttachmentSocket.ax(ref))
        } else {
            fallback
        }
    }
}
#endif
