#if os(macOS)
import AppKit

/// The gallery's socket surface: `mac-main.attachments.gallery` (what the gallery shows) and its clicks.
@MainActor
enum AttachmentSocketGallery {
    static var store: AttachmentStore { .shared }

    static func row(_ m: AppModel) -> [ControlRow] {
        guard let set = AttachmentSet.reader(m), !set.isEmpty else { return [] }
        let open = store.expanded.contains(set.messageId), docsOpen = store.documentsExpanded.contains(set.messageId)
        let images = AttachmentTileImages.shared
        let (tiles, chips) = AttachmentGallery.split(set)
        return [[
            "message": set.messageId, "header": AttachmentGalleryHeader.words(set),
            "media": set.media.map(\.name), "documents": set.documents.map(\.name),
            "tiles_shown": AttachmentGallery.shown(tiles, open: open).count,
            "more": AttachmentGallery.hidden(tiles, open: open).map { "+\($0)" } ?? "", "expanded": open,
            "documents_shown": AttachmentGallery.shown(chips, open: docsOpen).count,
            "documents_more": AttachmentGallery.hidden(chips, open: docsOpen) ?? 0,
            // design Attachment T1: the files drawn as chips (documents, and any file not on this Mac) and the tiles, by name
            "chips": chips.map(\.name), "tiles": tiles.map(\.name),
            "inline": set.inline.keys.sorted { $0.index < $1.index }.map(\.name),
            "videos": set.media.filter { $0.kind == .video }.map { v in
                ["name": v.name, "first_frame": images.firstFrame.contains(v), "duration": images.durations[v].map(AttachmentTileImages.durationWords) ?? ""] as [String: Any]
            },
            "tile_side": Int(AttachmentTileImages.side), "cells": AttachmentGallery.cells, "per_row": AttachmentGallery.perRow, "tiles_with_picture": set.media.filter { images.images[$0] != nil }.count,
        ]]
    }

    static func clicks(_ m: AppModel) -> [String: (AppModel) -> Bool] {
        let r = AttachmentSocket.reader
        return [
            "\(r).gallery.more": { m in
                guard let set = AttachmentSet.reader(m), AttachmentGallery.hidden(set.media, open: store.expanded.contains(set.messageId)) != nil else { return false }
                store.expanded.insert(set.messageId); return true
            },
            "\(r).documents.more": { m in
                guard let set = AttachmentSet.reader(m), AttachmentGallery.hidden(set.documents, open: store.documentsExpanded.contains(set.messageId)) != nil else { return false }
                store.documentsExpanded.insert(set.messageId); return true
            },
            "\(r).gallery.quick-look": { m in AttachmentSet.reader(m).map(AttachmentActs.quickLookAll) ?? false },
            "\(r).gallery.save-all": { m in AttachmentSet.reader(m).map(AttachmentActs.saveAll) ?? false },
            // rig only: back to the collapsed gallery, so the collapsed state can be photographed in each theme
            "\(r).dev.collapse": { m in
                guard AttachmentPlaces.isRig, let set = AttachmentSet.reader(m) else { return false }
                store.expanded.remove(set.messageId); store.documentsExpanded.remove(set.messageId); return true
            },
        ]
    }
}
#endif
