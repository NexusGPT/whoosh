import Foundation

/// Files on the draft (contract delta 22): the core holds them (`AddDraftAttachment`, `RemoveDraftAttachment`), the tiles are drawn from its
/// read `draft/<id>/attachments`, and a send carries them. The composer keeps only what is still being added. A tile for a file being
/// added is there at once; the core's answer settles it a turn later (never on the key's own turn, never on the network).
extension ComposerModel {
    private var fileCore: WhooshCore { AppModel.shared.core }

    /// A file dragged in, pasted, picked, or handed over by the notch's file drop (`NotchModel.fileSink`). True when a tile is drawn.
    @discardableResult
    public func attach(_ url: URL) -> Bool {
        guard !attachments.contains(where: { $0.url == url }) else { return false }
        pendingAdds.append(url)
        line = ComposerWords.attached(url.lastPathComponent)
        rebuildTiles()
        scheduleSettle()
        return true
    }

    /// The × on a tile: the tile goes at once, the core is told a turn later; a refusal brings the tile back with its sentence.
    public func removeAttachment(_ a: Attachment) {
        #if os(macOS)
        if let u = a.url { AttachmentPlaces.removePasted([u]) }   // a pasted picture's file goes with its tile; a Finder file stays
        #endif
        if a.state == .adding { pendingAdds.removeAll { "adding:" + $0.path == a.id } }
        else { fileTiles.removeAll { $0.id == a.id }; pendingRemovals.append(a.id) }
        rebuildTiles()
        scheduleSettle()
    }

    /// Every file off the draft (a template replaces them).
    func clearFiles() {
        pendingAdds = []
        pendingRemovals += fileTiles.map(\.id)
        fileTiles = []
        rebuildTiles()
        scheduleSettle()
    }

    func rebuildTiles() {
        let inText = Set(ComposerRichMail.pictureIds(session.document))   // a picture in the text is not a tile (gap-composer-rich)
        attachments = fileTiles.filter { !inText.contains($0.id) } + pendingAdds.map {
            let size = (try? $0.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
            return Attachment(id: "adding:" + $0.path, name: $0.lastPathComponent, url: $0, sizeBytes: UInt64(size), state: .adding)
        }
    }

    private func scheduleSettle() {
        Task { @MainActor in
            await Task.yield()
            self.settleFiles()
        }
    }

    /// What a read of the tiles sees: the adds and removals run now, and the tiles are the core's read of the disk now (a file deleted
    /// meanwhile reads `gone`). The socket's reads call it; a held composer (`filesHeld`) shows its adds as being added.
    func syncFiles() {
        if filesHeld { return }
        if pendingAdds.isEmpty && pendingRemovals.isEmpty { if coreDraftId != nil { refreshFiles() }; return }
        settleFiles()
    }

    /// Runs the adds and removals now. The send calls it first, so what it sends is settled.
    func settleFiles() {
        guard !filesHeld, !pendingAdds.isEmpty || !pendingRemovals.isEmpty else { return }
        let adds = pendingAdds, removals = pendingRemovals
        pendingAdds = []; pendingRemovals = []
        if let d = coreDraftId {
            for id in removals {
                do { _ = try fileCore.run(command: .removeDraftAttachment(draftId: d, attachmentId: id)) }
                catch CoreError.NotFound { continue }          // the draft or the file is already gone: nothing to remove
                catch { line = Self.words(error) }
            }
        }
        for url in adds {
            do {
                try addSource(.localFile(path: url.path))
                library.noteRecent(url.path)
            } catch { line = Self.words(error) }
        }
        refreshFiles()
    }

    /// One file onto the core's draft. A draft the core no longer has (a send that was undone, a reset) is saved again and the file goes on it.
    /// Returns the core's id of the file (nil when the core named none); a picture in the text is placed by this id.
    @discardableResult
    func addSource(_ src: AttachmentSource) throws -> String? {
        let d = try ensureCoreDraft()
        do { return try fileCore.run(command: .addDraftAttachment(draftId: d, source: src)).createdId }
        catch CoreError.NotFound(let kind, _) where kind == "draft" {
            coreDraftId = nil
            return try fileCore.run(command: .addDraftAttachment(draftId: try ensureCoreDraft(), source: src)).createdId
        }
    }

    /// What a queued message carries, as sources (read before its send is taken back).
    func outboxSources(_ outboxId: String) -> [AttachmentSource] {
        guard let raw = fileCore.state(path: "outbox/\(outboxId)/attachments"), let data = raw.data(using: .utf8) else { return [] }
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        guard let read = try? decoder.decode(DraftFilesRead.self, from: data) else { return [] }
        return read.attachments.compactMap {
            switch $0.source.kind {
            case "local_file": return $0.source.path.map { AttachmentSource.localFile(path: $0) }
            case "stored_message": if let m = $0.source.messageId, let a = $0.source.attachmentId { return .storedMessage(messageId: m, attachmentId: a) } else { return nil }
            default: return nil
            }
        }
    }

    /// The send was taken back: the files go back onto the draft. A core that kept the draft answers "already there"; one that
    /// dropped it (the fixture's cancel) gets a new draft with the same files.
    func restoreFiles(_ sources: [AttachmentSource]) {
        for src in sources { do { try addSource(src) } catch { line = Self.words(error) } }
        refreshFiles()
    }

    /// The tiles, as the core reads the draft's files now (a file deleted meanwhile reads `gone`).
    func refreshFiles() {
        fileTiles = []
        if let d = coreDraftId, let raw = fileCore.state(path: "draft/\(d)/attachments"), let data = raw.data(using: .utf8) {
            let decoder = JSONDecoder()
            decoder.keyDecodingStrategy = .convertFromSnakeCase
            if let read = try? decoder.decode(DraftFilesRead.self, from: data) {
                fileTiles = read.attachments.map {
                    let url = $0.source.path.map { URL(fileURLWithPath: ($0 as NSString).expandingTildeInPath) }
                    return Attachment(id: $0.id, name: $0.fileName, url: url, sizeBytes: $0.sizeBytes, state: FileTile(rawValue: $0.state) ?? .ready)
                }
            }
        }
        rebuildTiles()
    }

    private struct DraftFilesRead: Decodable {
        struct Row: Decodable {
            struct Source: Decodable { let kind: String; let path: String?; let messageId: String?; let attachmentId: String? }
            let id: String; let fileName: String; let sizeBytes: UInt64; let state: String; let source: Source
        }
        let attachments: [Row]
    }

    /// The core's draft: saved on the first attach (a message with nothing in it yet takes a subject so the core accepts it).
    private func ensureCoreDraft() throws -> String {
        if let d = coreDraftId { return d }
        let empty = to.isEmpty && subject.isEmpty && bodyText.isEmpty
        let out = try fileCore.run(command: .saveDraft(fromAccountId: fromId.isEmpty ? nil : fromId, to: to, cc: cc, subject: empty ? ComposerWords.newMessage : subject,
                                                       body: bodyText, replyToThreadId: threadId))
        guard let id = out.createdId else { throw CoreError.Invalid(field: "draft", reasonWords: ComposerWords.notSaved) }
        coreDraftId = id
        return id
    }

    /// The draft is thrown away: the core's draft (and its files) go too.
    func dropCoreDraft() {
        if let d = coreDraftId { _ = try? fileCore.run(command: .deleteDraft(draftId: d)) }
        coreDraftId = nil; autosaveSaved = ""; fileTiles = []; pendingAdds = []; pendingRemovals = []
        rebuildTiles()
    }

    /// A forward of a message with files: the core's `Forward` draft carries them, each removable.
    private static var forwardRefusedWords: String { Words.text("This message could not be forwarded.") }

    /// A forward the core refuses is said in the composer's one line, as a refused send is: the core's own words when it gives them, else one sentence.
    func adoptForwardDraft(_ messageId: String) {
        do {
            guard let d = try fileCore.run(command: .forward(messageId: messageId)).createdId else { line = Self.forwardRefusedWords; return }
            coreDraftId = d
            refreshFiles()
        } catch {
            switch error {
            case CoreError.Action, CoreError.Invalid: line = Self.words(error)
            default: line = Self.forwardRefusedWords
            }
        }
    }

    /// The code of an attachment refusal, nil for any other error.
    static func refusalCode(_ e: Error) -> String? {
        // the reply's own refusal is not a file's: it keeps the composer failed, with Retry
        if case CoreError.Invalid(let field, _) = e, field != ComposerWords.replyThreadField, ComposerWords.refusal(field) != nil { return field }
        return nil
    }
}
