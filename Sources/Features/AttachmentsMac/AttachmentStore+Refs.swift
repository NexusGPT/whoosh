#if os(macOS)
import Foundation

/// Which attachments the keys act on: the reader's open message, or the composer's pending files.
extension AttachmentStore {
    /// The open (or resting) message's attachments in the reader, in display order: the gallery, then the documents.
    public func readerRefs(_ m: AppModel) -> [AttachmentRef] { AttachmentSet.reader(m)?.all ?? [] }

    /// The composer's pending files.
    public func composerRefs() -> [AttachmentRef] {
        ComposerModel.shared.attachments.enumerated().map { AttachmentRef(messageId: AttachmentRef.composer, index: $0.offset, name: $0.element.name) }
    }

    /// The set the selected chip belongs to (the composer's, or the reader's open message).
    public func siblings(_ m: AppModel = .shared) -> [AttachmentRef] {
        if selected?.isPending == true { return composerRefs() }
        return readerRefs(m)
    }

    /// The selection, dropped when its message is no longer the one on screen (J/K moved on, the composer closed).
    public func liveSelection(_ m: AppModel = .shared) -> AttachmentRef? {
        guard let s = selected else { return nil }
        if siblings(m).contains(s) { return s }
        selected = nil
        return nil
    }

}
#endif
