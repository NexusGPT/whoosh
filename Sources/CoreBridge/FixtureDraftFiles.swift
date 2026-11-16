import Foundation

// Contracts delta 22 under --fixture-core: attachments on a draft (reply_ai_d22.rs). The rules are the contract's pure functions (planAddAttachment,
// forwardAttachments, checkDraftAttachmentsSendable, checkAttachmentDraftOpen); this file reads the disk and keeps the rows (`draftFileRows`: owner id -> files).
// A file of this Mac is read from the disk at Add and again when the draft or the send asks for its state (a file deleted meanwhile reads Gone).
// The send moves the rows to the outbox entry (`outbox/<id>/attachments` shows them); an undo of the send gives them back to the draft.

extension FixtureCore {
    private func diskSize(_ path: String) -> UInt64? {
        let full = (path as NSString).expandingTildeInPath
        var isDir: ObjCBool = false
        guard FileManager.default.fileExists(atPath: full, isDirectory: &isDir), !isDir.boolValue,
              let n = (try? FileManager.default.attributesOfItem(atPath: full))?[.size] as? NSNumber else { return nil }
        return n.uint64Value
    }
    private func mediaType(_ name: String) -> String { Self.mimeType(name) }

    /// the files with the state of the disk now: a local file that vanished reads Gone
    func refreshedFilesLocked(_ files: [OutgoingAttachment]) -> [OutgoingAttachment] {
        files.map { f in
            guard case .localFile(let path) = f.source else { return f }
            var g = f
            if diskSize(path) == nil { g.state = .gone }
            return g
        }
    }

    func addDraftAttachmentLocked(_ draftId: String, _ source: AttachmentSource) throws -> CommandOutcome {
        guard draftRows[draftId] != nil || entries.contains(where: { $0.id == draftId }) else { throw CoreError.NotFound(kind: "draft", id: draftId) }
        try checkAttachmentDraftOpen(open: draftRows[draftId] != nil)
        let existing = draftFileRows[draftId] ?? []
        var storedName: String? = nil
        var size: UInt64? = nil
        var local = false
        switch source {
        case .localFile(let path):
            size = diskSize(path)
        case .storedMessage(let messageId, let attachmentId):
            guard let a = attachmentFilesLocked(messageId).first(where: { $0.id == attachmentId }) else { throw CoreError.NotFound(kind: "attachment", id: attachmentId) }
            storedName = a.fileName; size = a.sizeBytes; local = a.localPath != nil
        }
        switch try planAddAttachment(existing: existing, source: source, storedFileName: storedName, sizeBytes: size, localOnThisMac: local) {
        case .alreadyThere(let id): return CommandOutcome(undoEntryId: nil, createdId: id)
        case .add(let state, let name):
            let id = newId("attachment")
            let type: String = { if case .storedMessage(let m, let a) = source, let f = attachmentFilesLocked(m).first(where: { $0.id == a }) { return f.mimeType }; return mediaType(name) }()
            draftFileRows[draftId, default: []].append(OutgoingAttachment(id: id, ownerId: draftId, source: source, fileName: name, mediaType: type, sizeBytes: size ?? 0, state: state))
            return CommandOutcome(undoEntryId: nil, createdId: id)
        }
    }

    func removeDraftAttachmentLocked(_ draftId: String, _ attachmentId: String) throws -> CommandOutcome {
        guard draftRows[draftId] != nil || entries.contains(where: { $0.id == draftId }) else { throw CoreError.NotFound(kind: "draft", id: draftId) }
        try checkAttachmentDraftOpen(open: draftRows[draftId] != nil)
        guard let i = draftFileRows[draftId]?.firstIndex(where: { $0.id == attachmentId }) else { throw CoreError.NotFound(kind: "attachment", id: attachmentId) }
        draftFileRows[draftId]?.remove(at: i)   // the file on the disk is never touched
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    // MARK: received files (delta 23): DownloadAttachment answers at once and the file lands a moment later, as the real core does (the two steps a check can read)

    func downloadAttachmentLocked(_ attachmentId: String) throws -> CommandOutcome {
        guard let r = attachmentId.range(of: "-att-", options: .backwards) else { throw CoreError.NotFound(kind: "attachment", id: attachmentId) }
        let messageId = String(attachmentId[..<r.lowerBound])
        let file = attachmentFilesLocked(messageId).first { $0.id == attachmentId }
        switch try planDownloadAttachment(attachmentId: attachmentId, file: file, inFlight: attachmentTransfers[attachmentId] != nil) {
        case .start:
            attachmentTransfers[attachmentId] = (clock.nowMs(), messageId)   // answers now; the file lands after a short while (settleAttachmentTransfersLocked)
        case .alreadyOnThisMac, .alreadyDownloading: break
        }
        return CommandOutcome(undoEntryId: nil, createdId: nil)
    }

    /// the fetches that have run their short while land: the file is on this Mac and the event fires
    func settleAttachmentTransfersLocked() {
        let now = clock.nowMs()
        for (id, t) in attachmentTransfers where now - t.startMs >= 300 {
            attachmentTransfers[id] = nil
            landedAttachments.insert(id)
            pending.append((.thread, .attachmentTransferChanged(attachmentId: id, messageId: t.messageId, transfer: .onThisMac)))
        }
    }

    /// `attachment/<id>/transfer`: a read of the fixture's own state, never a wait
    func attachmentTransferStateLocked(_ attachmentId: String) -> String? {
        settleAttachmentTransfersLocked()
        guard let r = attachmentId.range(of: "-att-", options: .backwards),
              let file = attachmentFilesLocked(String(attachmentId[..<r.lowerBound])).first(where: { $0.id == attachmentId }) else { return nil }
        return attachmentTransferJson(t: attachmentTransferOf(file: file, inFlight: attachmentTransfers[attachmentId] != nil, lastFailureWords: nil))
    }
}
