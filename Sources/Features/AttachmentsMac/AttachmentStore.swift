#if os(macOS)
import Foundation
import Observation

/// The one home of the attachment proxy's state (frontend-architecture §1): every phase, the selected chip, the acts
/// waiting for a download, what was opened and saved. The chips, the menu, Quick Look and the socket read it here,
/// so a driven check exercises exactly what a click or a key does.
@Observable @MainActor
public final class AttachmentStore {
    public static let shared = AttachmentStore()

    public struct Opened: Equatable { public let path: String; public let app: String; public let launched: Bool; public let count: Int }

    public private(set) var phases: [AttachmentRef: AttachmentPhase] = [:]
    /// The chip that holds the keys (a click selects it, as in the Finder); nil when no chip is selected.
    public var selected: AttachmentRef?
    public private(set) var waiting: [AttachmentRef: AttachmentWaitingAct] = [:]
    public var opened: [AttachmentRef: Opened] = [:]
    public var saved: [AttachmentRef: URL] = [:]
    /// Under the rig, what Show in Finder would reveal and what Share would hand to the share sheet (no Finder window
    /// and no share service ever opens on the owner's screen from a test).
    public var revealed: [AttachmentRef: String] = [:]
    public var shared: [AttachmentRef: [String]] = [:]
    /// A short line the chip shows after an act ("Saved to Downloads", "Copied"); cleared by the next act.
    public var notice: [AttachmentRef: String] = [:]
    /// Pasted or dropped files the composer took, by file name, in arrival order.
    public var intakeLog: [String] = []
    /// Messages whose gallery (media) or document list shows every item past ATTACHMENT_GALLERY_MAX.
    public var expanded: Set<String> = []
    public var documentsExpanded: Set<String> = []
    /// The gallery's width in the reader, measured: how many tiles one row holds.
    public var galleryWidth: CGFloat = 0
    /// The bytes of inline data: images, by their ref (written to a local file the first time a tile asks).
    @ObservationIgnored var inlineData: [AttachmentRef: Data] = [:]

    /// The phase, starting the fetch the first time a chip asks.
    public func phase(_ ref: AttachmentRef) -> AttachmentPhase {
        if ref.isPending { return Self.pendingURL(ref).map { .ready($0) } ?? .unavailable(AttachmentWords.notOnThisMac) }
        if let p = phases[ref] { return p }
        if let data = inlineData[ref] { return landInline(ref, data) }
        guard let source = AttachmentSources.current else {
            phases[ref] = .unavailable(AttachmentWords.notOnThisMac)
            return .unavailable(AttachmentWords.notOnThisMac)
        }
        if !source.startsOnAppear(ref) {        // the real core: the file is here or it is not; nothing downloads until an act asks
            let p = source.initial(ref)
            if p.url != nil { land(ref, p) } else { phases[ref] = p }
            return p
        }
        phases[ref] = .remote
        source.fetch(ref) { [weak self] p in self?.land(ref, p) }
        return phases[ref] ?? .remote
    }

    public func url(_ ref: AttachmentRef) -> URL? { phase(ref).url }

    /// A file that is not here draws as the chip with a cloud, never as a picture tile: not fetched yet (the real core's files,
    /// or the rig's absent ones), fetching, or failed. A fixture file still in its first copy is not absent (it was never remote).
    public func isAbsent(_ ref: AttachmentRef) -> Bool {
        if ref.isPending || inlineData[ref] != nil { return false }
        switch peek(ref) {
        case .ready: return false
        case .downloading, .unavailable: return true
        case .remote: return AttachmentSources.current.map { !$0.startsOnAppear(ref) } ?? false
        }
    }

    /// Whether the chip or tile shows its key cap(s): a selected or hovered file, but an absent one only while selected
    /// (design Attachment T1: the space cap at the right of the selected chip).
    public func keysShown(_ ref: AttachmentRef) -> Bool {
        if AttachmentRig.mutant("no-keys") { return false }
        if isAbsent(ref) && !AttachmentRig.mutant("cap-when-hovered") { return selected == ref }
        return selected == ref || hovered == ref
    }

    /// Starts the fetch of a file that is not here (the first Space or click that needs its bytes, or a retry after a failure).
    /// The tile shows the progress from this frame.
    func start(_ ref: AttachmentRef) {
        guard !ref.isPending, let source = AttachmentSources.current else { return }
        phases[ref] = .downloading(0)
        source.fetch(ref) { [weak self] p in self?.land(ref, p) }
    }

    /// `.remote` means two things: a lazy source's file nobody asked for (a fetch must start now), or an eager source's fetch already
    /// running from the chip's appearance (starting a second copy raced the first and left "…" on the chip: main71, gallery-30).
    private func needsStart(_ ref: AttachmentRef) -> Bool {
        AttachmentSources.current.map { !$0.startsOnAppear(ref) } ?? false
    }

    /// The file is not here any more (rig door): the next phase read starts from what the source says.
    func forget(_ ref: AttachmentRef) {
        phases[ref] = nil; saved[ref] = nil; opened[ref] = nil; notice[ref] = nil; waiting[ref] = nil
        AttachmentThumbnails.shared.forget(ref)
        AttachmentTileImages.shared.forget(ref)
    }

    /// The phase without starting anything: what a view body reads (a fetch starts from `phase` on appear).
    public func peek(_ ref: AttachmentRef) -> AttachmentPhase {
        if ref.isPending { return Self.pendingURL(ref).map { .ready($0) } ?? .unavailable(AttachmentWords.notOnThisMac) }
        return phases[ref] ?? .remote
    }

    /// Runs `act` now when the bytes are here, else as soon as they land (the chip says it is waiting).
    public func whenReady(_ ref: AttachmentRef, _ act: AttachmentWaitingAct) -> Bool {
        switch phase(ref) {
        case .ready: AttachmentActs.run(act, ref); return true
        case .unavailable:
            // a failed fetch says why on the tile; the same key asks again. Without a source (or for a pending file) nothing can come.
            guard !ref.isPending, AttachmentSources.current != nil else { return false }
            waiting[ref] = act
            start(ref)
            return true
        case .remote: waiting[ref] = act; if needsStart(ref) { start(ref) }; return true
        case .downloading: waiting[ref] = act; return true
        }
    }

    /// Hover and press of a chip, for its look; set by the chip's hit layer on the input frame.
    public var hovered: AttachmentRef?
    public var pressed: AttachmentRef?
    @ObservationIgnored private var fileWaiters: [AttachmentRef: [CheckedContinuation<URL?, Never>]] = [:]

    /// The local file once its bytes are here (a drag's file promise waits on this); nil when it cannot come.
    public func file(_ ref: AttachmentRef) async -> URL? {
        switch phase(ref) {
        case .ready(let u): return u
        case .unavailable: return nil
        case .remote, .downloading:
            if case .remote = phase(ref), needsStart(ref) { start(ref) }
            return await withCheckedContinuation { fileWaiters[ref, default: []].append($0) }
        }
    }

    private func land(_ ref: AttachmentRef, _ p: AttachmentPhase) {
        phases[ref] = p
        if p.url != nil || p.word == "unavailable" { fileWaiters.removeValue(forKey: ref)?.forEach { $0.resume(returning: p.url) } }
        if p.url != nil {
            AttachmentThumbnails.shared.request(ref)
            AttachmentTileImages.shared.request(ref)
            if let act = waiting.removeValue(forKey: ref) { AttachmentActs.run(act, ref) }
            QuickLookController.shared.refresh(ref)
        } else if case .unavailable = p {
            waiting[ref] = nil
        }
    }

    /// Keeps a set's inline data: images so a tile can make them a file.
    func remember(_ set: AttachmentSet) {
        for (ref, data) in set.inline { if let data { inlineData[ref] = data } }
    }

    private func landInline(_ ref: AttachmentRef, _ data: Data) -> AttachmentPhase {
        let folder = AttachmentPlaces.cache(ref.messageId), url = folder.appendingPathComponent(ref.name)
        do {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            try data.write(to: url)
        } catch { phases[ref] = .unavailable(error.localizedDescription); return phases[ref] ?? .remote }
        land(ref, .ready(url))
        return .ready(url)
    }

    static func pendingURL(_ ref: AttachmentRef) -> URL? {
        let files = ComposerModel.shared.attachments
        return files.indices.contains(ref.index) && files[ref.index].name == ref.name ? files[ref.index].url : nil
    }
}
#endif
