#if os(macOS)
import AppKit

/// The socket surface par-attachments reads and drives. Regions: `mac-main.attachments` (the reader's chips),
/// `mac-composer.attachment-files` (the composer's pending files), `mac-main.attachments.panel` (Quick Look),
/// `mac-main.attachments.menu` (the selected chip's menu, as built) and `mac-main.attachments.state`.
/// Every click runs the same function as the real control (AttachmentActs, AttachmentKeys, AttachmentMenu).
@MainActor
enum AttachmentSocket {
    static let reader = "mac-main.attachments"
    static let composer = "mac-composer.attachment-files"
    static var store: AttachmentStore { .shared }

    static func ax(_ ref: AttachmentRef) -> String { "\(ref.isPending ? composer : reader).\(ref.index)" }

    static func row(_ ref: AttachmentRef) -> ControlRow {
        let p = store.peek(ref), ql = QuickLookController.shared
        let thumb = AttachmentThumbnails.shared.images[ref] != nil ? "thumbnail" : (ref.kind.hasThumbnail ? "pending" : "icon")
        let o = store.opened[ref]
        var r: ControlRow = [
            "ax": ax(ref), "index": ref.index, "message": ref.messageId, "name": ref.name, "kind": ref.kind.rawValue,
            "download": p.word, "progress": p.progress.map { Int(($0 * 100).rounded()) } ?? (p.url != nil ? 100 : 0),
            "thumbnail": thumb, "selected": store.selected == ref, "preview_open": ql.current == ref,
            "waiting": store.waiting[ref]?.rawValue ?? "", "opened": o?.path ?? "", "opened_with": o?.app ?? "",
            "launched": o?.launched ?? false, "open_count": o?.count ?? 0, "reveal_ms": WhooshConstants.HOVER_REVEAL_MS, "saved_path": store.saved[ref]?.path ?? "", "revealed": store.revealed[ref] ?? "",
            "shared": store.shared[ref] ?? [], "notice": store.notice[ref] ?? "",
            "tooltip": AttachmentChipFrames.view(ref)?.toolTip ?? "",
            "keys_shown": store.keysShown(ref),
            // the width the name needs in one line (body, semibold): the check compares it with the name's own box (a name is cut only when the chip has no more room)
            "name_need": (ref.name as NSString).size(withAttributes: [.font: NSFont.systemFont(ofSize: WhooshTypography.body.size, weight: .semibold)]).width.rounded(.up),
        ]
        if let f = AttachmentChipFrames.screenFrame(ref) { r["screen_frame"] = ["x": f.minX, "y": f.minY, "w": f.width, "h": f.height] }
        if let d = lastDrag[ref] { r.merge(d) { $1 } }
        // delta 23, what a tile is now: present (the bytes are here) · fetching (with `percent`; the real core gives none, so 0) ·
        // absent (not here, nothing asked yet) · failed (`why` says it in words). The same four for chips and tiles.
        let ph = store.peek(ref)
        r["fetches"] = FixtureAttachmentSource.shared.starts[ref.name] ?? 0
        r["state"] = ph.url != nil ? "present" : (ph.progress != nil ? "fetching" : (ph.word == "unavailable" ? "failed" : "absent"))
        let percent: Any   // null: the source gives no figure
        if ph.isFetching { if let f = ph.fraction { percent = Int((f * 100).rounded()) } else { percent = NSNull() } } else { percent = ph.url != nil ? 100 : 0 }
        r["percent"] = percent
        r["draws"] = ref.isPending ? "chip" : (store.isAbsent(ref) ? "chip" : (ref.kind == .image || ref.kind == .video ? "tile" : "chip"))   // what the gallery draws for it
        r["glyph"] = store.isAbsent(ref) ? (ph.isFetching ? "type" : "cloud") : (ph.url != nil ? "picture" : "icon")
        r["failed_line_warn"] = AttachmentChipText.failed(ref)
        if case .unavailable(let why) = ph { r["why"] = why } else { r["why"] = "" }
        r["group"] = ref.kind == .image || ref.kind == .video ? "media" : "document"
        r["subtitle"] = AttachmentChipText.subtitle(ref)
        r["badge"] = AttachmentChipText.badge(ref)?.symbol ?? ""
        r["tile_picture"] = AttachmentTileImages.shared.images[ref] != nil
        return r
    }

    static var lastDrag: [AttachmentRef: [String: Any]] = [:]

    static var rows: [String: (AppModel) throws -> [ControlRow]] {
        [
            reader: { m in store.readerRefs(m).map(row) },
            composer: { _ in store.composerRefs().map(row) },
            "\(reader).panel": { _ in [AttachmentSocketPanels.panel()] },
            "\(reader).menu": { m in AttachmentSocketPanels.menu(store.liveSelection(m)) },
            "\(reader).state": { m in [AttachmentSocketPanels.state(m)] },
            "\(reader).gallery": { m in AttachmentSocketGallery.row(m) },
            "\(reader).header": { m in AttachmentSet.reader(m).map { [AttachmentHeaderFit.row($0)] } ?? [] },
        ]
    }

    /// Per chip: `<base>.<i>` (a click: select), `.double` (double-click: open), `.menu` (the real menu at the chip),
    /// `.menu.<item>` (that item's act), `.drag` (the drag's pasteboard, rehearsed), `.hover`. Then the global ones.
    static func clicks(_ m: AppModel) -> [String: (AppModel) -> Bool] {
        var t: [String: (AppModel) -> Bool] = [:]
        for ref in store.readerRefs(m) + store.composerRefs() {
            let a = ax(ref)
            t[a] = { _ in AttachmentKeys.select(ref); return true }
            t["\(a).double"] = { _ in AttachmentKeys.select(ref); return AttachmentActs.open(ref) }
            t["\(a).hover"] = { _ in store.hovered = ref; return true }
            t["\(a).menu"] = { _ in AttachmentMenu.popUp(ref) }
            t["\(a).drag"] = { _ in
                guard let d = AttachmentDrag.rehearse(ref) else { lastDrag[ref] = nil; return false }
                lastDrag[ref] = d; return true
            }
            for (id, act) in AttachmentSocketPanels.menuActs(ref) { t["\(a).menu.\(id)"] = act }
            // doors-read: the xmark button on a pending file (the composer's chip); the same removal the menu's Remove runs
            if ref.isPending { t["\(a).remove"] = { _ in !AttachmentRig.mutant("remove-button-dead") && AttachmentActs.remove(ref) } }
        }
        t.merge(AttachmentSocketGallery.clicks(m)) { $1 }
        t["\(reader).menu.close"] = { _ in AttachmentMenu.close() }
        t["\(reader).save-as.save"] = { _ in AttachmentSocketPanels.answerSaveAs(save: true) }
        t["\(reader).save-as.cancel"] = { _ in AttachmentSocketPanels.answerSaveAs(save: false) }
        t["\(reader).download.half"] = { _ in FixtureAttachmentSource.shared.advance(to: 0.5) }
        t["\(reader).download.finish"] = { _ in FixtureAttachmentSource.shared.advance(to: 1) }
        t["\(reader).hover.none"] = { _ in store.hovered = nil; return true }
        if AttachmentPlaces.isRig {
            t["\(reader).dev.stage-image"] = { _ in AttachmentSocketPanels.stage(image: true) }
            t["\(reader).dev.stage-text"] = { _ in AttachmentSocketPanels.stage(image: false) }
            t["\(reader).dev.stage-file"] = { _ in AttachmentSocketPanels.stageFile() }
            // focus a message by its id (the list's order moves as drafts and arrivals land; an index does not name a mail)
            for r in m.rows { t["\(reader).dev.focus.\(r.messageId)"] = { m in m.focus(messageId: r.messageId); return m.focusedMessageId == r.messageId } }
            // the fixture's "not on this Mac yet" (no fixture verb holds a file absent yet: asked of probes-r1): every file of the shown
            // message becomes absent (no fetch until an act asks, then a held download), and a fetch can be made to fail with a reason
            t["\(reader).dev.absent"] = { m in
                guard let set = AttachmentSet.reader(m), !set.isEmpty else { return false }
                FixtureAttachmentSource.shared.makeAbsent(set.all.map(\.name))
                for ref in set.all { store.forget(ref) }
                return true
            }
            t["\(reader).dev.present"] = { m in
                guard let set = AttachmentSet.reader(m), !set.isEmpty else { return false }
                FixtureAttachmentSource.shared.makeHere(set.all.map(\.name))
                FixtureAttachmentSource.shared.noFigure = false; FixtureAttachmentSource.shared.failure = nil
                FixtureAttachmentSource.shared.resetStarts()
                for ref in set.all { store.forget(ref); _ = store.phase(ref) }   // the file is here again: its (eager) fetch starts now, not on the chip's next appearance (main107: the strip kept "not on this Mac yet" after the pictures' theme changes)
                return true
            }
            t["\(reader).dev.percent.off"] = { _ in FixtureAttachmentSource.shared.noFigure = true; return true }
            t["\(reader).dev.percent.on"] = { _ in FixtureAttachmentSource.shared.noFigure = false; return true }
            t["\(reader).dev.fail.on"] = { _ in FixtureAttachmentSource.shared.failure = "the server refused the download"; return true }
            t["\(reader).dev.fail.off"] = { _ in FixtureAttachmentSource.shared.failure = nil; return true }
            for name in ["chip", "quicklook", "menu", "composer"] { t["\(reader).dev.shot.\(name)"] = { _ in AttachmentShots.capture(name) } }
        }
        return t
    }
}
#endif
