#if os(macOS)
import Foundation

/// The real core's attachment bytes (contract delta 23). A received file is usually not on this Mac when its mail opens:
/// `core.attachments(messageId:)` says so with `localPath` None, and nothing downloads until an act needs the bytes.
/// Then `Command.downloadAttachment` answers at once and the file lands later; the thread stream's
/// `attachmentTransferChanged` ends the wait (`onThisMac` or `failed` in words). The chip shows `.downloading` meanwhile
/// (the contract carries no percent: `.downloading(noFigure)`, a spinning ring and the word "Fetching" alone), and a failure says why on the tile; the next Space asks again.
@MainActor
public final class CoreAttachmentSource: AttachmentByteSource {
    public static let shared = CoreAttachmentSource()
    private var pending: [String: (ref: AttachmentRef, update: @MainActor (AttachmentPhase) -> Void)] = [:]
    private var sink: CoreAttachmentSink?

    public func startsOnAppear(_ ref: AttachmentRef) -> Bool { false }

    public func initial(_ ref: AttachmentRef) -> AttachmentPhase {
        guard let f = file(for: ref) else { return .unavailable(AttachmentWords.notOnThisMac) }
        return Self.url(of: f).map { .ready($0) } ?? .remote
    }

    public func fetch(_ ref: AttachmentRef, update: @escaping @MainActor (AttachmentPhase) -> Void) {
        guard let f = file(for: ref) else { update(.unavailable(AttachmentWords.notOnThisMac)); return }
        if let u = Self.url(of: f) { update(.ready(u)); return }
        listen()
        pending[f.id] = (ref, update)
        update(.downloading(AttachmentPhase.noFigure))
        let m = AppModel.shared
        if !m.perform(.downloadAttachment(attachmentId: f.id)) {
            pending[f.id] = nil
            update(.unavailable(m.lastFailure?.reasonWords ?? AttachmentWords.notOnThisMac))
        }
    }

    /// The thread stream's word about one file's transfer (main actor; only the id and the enum cross from the stream).
    func transferChanged(_ id: String, _ transfer: AttachmentTransfer) {
        guard let p = pending[id] else { return }
        switch transfer {
        case .onThisMac:
            pending[id] = nil
            if let f = file(for: p.ref), let u = Self.url(of: f) { p.update(.ready(u)) } else { p.update(.unavailable(AttachmentWords.notOnThisMac)) }
        case .failed(let why):
            pending[id] = nil
            p.update(.unavailable(why))
        case .downloading: p.update(.downloading(AttachmentPhase.noFigure))
        case .notOnThisMac: break
        }
    }

    /// The core's row for the reference: the same place in the list and the same name, else the first of that name.
    private func file(for ref: AttachmentRef) -> AttachmentFile? {
        let files = AppModel.shared.core.attachments(messageId: ref.messageId)
        if files.indices.contains(ref.index), files[ref.index].fileName == ref.name { return files[ref.index] }
        return files.first { $0.fileName == ref.name }
    }

    /// The file on disk when the row names one that exists; a path that is gone counts as not here (the next act fetches again).
    static func url(of f: AttachmentFile) -> URL? {
        guard let path = f.localPath, path.hasPrefix("/"), FileManager.default.fileExists(atPath: path) else { return nil }
        return URL(fileURLWithPath: path)
    }

    private func listen() {
        guard sink == nil else { return }
        let s = CoreAttachmentSink()
        s.owner = self
        sink = s
        _ = AppModel.shared.core.subscribe(stream: .thread, sink: s)
    }
}

/// Receives the thread stream on any thread; only the transfer events pass, as an id and the Sendable enum.
final class CoreAttachmentSink: EventSink, @unchecked Sendable {
    weak var owner: CoreAttachmentSource?

    func onEvent(event: CoreEvent) {
        guard case .attachmentTransferChanged(let id, _, let transfer) = event else { return }
        Task { @MainActor [weak owner] in owner?.transferChanged(id, transfer) }
    }
}
#endif
