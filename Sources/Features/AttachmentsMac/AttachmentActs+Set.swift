#if os(macOS)
import AppKit

extension AttachmentActs {
    /// Quick Look over the whole set: the selected item if it is in the set, else the first one; arrows step through all.
    static func quickLookAll(_ set: AttachmentSet) -> Bool {
        guard let start = store.selected.flatMap({ set.all.contains($0) ? $0 : nil }) ?? set.all.first else { return false }
        AttachmentKeys.select(start)
        return QuickLookController.shared.show(start)
    }

    /// Save all: every file into Downloads under a free name, each as soon as its bytes are here.
    static func saveAll(_ set: AttachmentSet) -> Bool {
        guard !set.all.isEmpty else { return false }
        for ref in set.all {
            if let url = store.peek(ref).url { _ = save(ref, url, to: AttachmentPlaces.free(ref.name, in: AttachmentPlaces.downloads)); continue }
            Task { @MainActor in
                if let url = await store.file(ref) { _ = save(ref, url, to: AttachmentPlaces.free(ref.name, in: AttachmentPlaces.downloads)) }
            }
        }
        return true
    }
}
#endif
