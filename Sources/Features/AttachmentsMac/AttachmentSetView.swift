#if os(macOS)
import SwiftUI

/// The reader's one mount for a message's attachments (hook commit "attachments hook: reader container"): the gallery
/// for its files and inline images; a count the core gives without names keeps the reader's own count tile.
struct AttachmentSetView: View {
    let thread: ReaderThread
    init(thread: ReaderThread) { self.thread = thread }

    var body: some View {
        let set = AttachmentSet.of(thread, core: AppModel.shared.core)
        if set.unnamedCount > 0 {
            AttachmentTile(words: ReaderWords.attachments(set.unnamedCount), index: 0)
        } else if !set.isEmpty {
            AttachmentGallery(set: set)
        }
    }
}
#endif
