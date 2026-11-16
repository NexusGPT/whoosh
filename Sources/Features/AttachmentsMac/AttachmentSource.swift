#if os(macOS)
import Foundation

/// Where an attachment's bytes come from: the fixture core's sample files (FixtureAttachmentSource) or, on the real core,
/// the engine's rows and `DownloadAttachment` (CoreAttachmentSource, contract delta 23).
@MainActor
public protocol AttachmentByteSource: AnyObject {
    /// Starts fetching the bytes and reports every change through `update`, ending in `.ready` or `.unavailable`.
    func fetch(_ ref: AttachmentRef, update: @escaping @MainActor (AttachmentPhase) -> Void)
    /// False: a chip that appears only asks `initial` and fetches nothing; the fetch starts when an act needs the bytes
    /// (Space, Quick Look, Open, Save, a drag). The real core's files are usually not on this Mac when a mail opens.
    func startsOnAppear(_ ref: AttachmentRef) -> Bool
    /// What a chip that appears shows before any fetch: `.ready` when the file is here already, else `.remote`.
    func initial(_ ref: AttachmentRef) -> AttachmentPhase
}

extension AttachmentByteSource {
    public func startsOnAppear(_ ref: AttachmentRef) -> Bool { true }
    public func initial(_ ref: AttachmentRef) -> AttachmentPhase { .remote }
}

@MainActor
public enum AttachmentSources {
    /// The fixture core's sample files under `--fixture-core`, the engine's files on the real core; nil when no core runs.
    public static var current: AttachmentByteSource? {
        switch AppModel.shared.host.kind {
        case .fixture: return FixtureAttachmentSource.shared
        case .rust: return CoreAttachmentSource.shared
        default: return nil
        }
    }
}

/// The fixture core's attachment bytes: every name maps to a sample file of its kind (docs/design/fixtures/attachments/,
/// drawn by make-fixtures.py), copied under the message's cache folder with the attachment's own name. Two attachments
/// download slowly so the progress ring and the waiting acts can be seen and driven: the third photo of the photo mail
/// (Quick Look waits for it) and the redlined term sheet (Open waits for it). They hold until `advance(to:)` moves
/// them (the socket's download clicks).
@MainActor
public final class FixtureAttachmentSource: AttachmentByteSource {
    public static let shared = FixtureAttachmentSource()
    /// The fixture's slow attachments (msg-08's third photo, msg-12's term sheet).
    public static let heldNames: Set<String> = ["IMG_2051.jpg", "term-sheet-v2-redlines.docx"]
    private var held: [AttachmentRef: @MainActor (AttachmentPhase) -> Void] = [:]
    private var done: Set<String> = []
    /// Rig door `dev.absent`: names the fixture holds as "not on this Mac yet". Like the real core's, they start no fetch when
    /// the chip appears; the first act that needs the bytes starts one, and it holds until `advance(to:)` moves it.
    private(set) var lazy: Set<String> = []
    /// Rig doors `dev.fail.on` / `dev.fail.off`: while set, a fetch ends at once in this reason, as a refused download does.
    var failure: String?
    /// Rig doors `dev.percent.off` / `.on`: a held download reports no figure (as the real core), or its fraction.
    var noFigure = false
    /// How many fetches each name has started (rig row `fetches`): a second start of a fetch already running is a defect.
    private(set) var starts: [String: Int] = [:]
    func resetStarts() { starts = [:] }

    public func startsOnAppear(_ ref: AttachmentRef) -> Bool { !lazy.contains(ref.name) || done.contains(ref.name) }

    /// Makes these files "not on this Mac yet" again: no fetch until an act asks, then a held download.
    func makeAbsent(_ names: [String]) {
        for n in names { lazy.insert(n); done.remove(n) }
    }

    /// Rig door `dev.present`: the inverse, so a later step meets the fixture as it was (these files fetch as they appear again).
    func makeHere(_ names: [String]) {
        for n in names { lazy.remove(n); done.insert(n) }
    }

    public func fetch(_ ref: AttachmentRef, update: @escaping @MainActor (AttachmentPhase) -> Void) {
        starts[ref.name, default: 0] += 1
        if let why = failure {
            update(.downloading(noFigure ? AttachmentPhase.noFigure : 0))
            update(.unavailable(why))
            return
        }
        if (Self.heldNames.contains(ref.name) || lazy.contains(ref.name)) && !done.contains(ref.name) {
            held[ref] = update
            update(.downloading(noFigure ? AttachmentPhase.noFigure : 0))
            return
        }
        copy(ref, update: update)
    }

    /// Moves every held download to `fraction` (0...1); at 1 their bytes land. False when nothing is downloading.
    public func advance(to fraction: Double) -> Bool {
        guard !held.isEmpty, fraction.isFinite else { return false }
        for (ref, update) in held {
            if fraction >= 1 {
                held[ref] = nil; done.insert(ref.name)
                copy(ref, update: update)
            } else {
                update(.downloading(noFigure ? AttachmentPhase.noFigure : max(0, fraction)))
            }
        }
        return true
    }

    public var isHolding: Bool { !held.isEmpty }

    /// The sample file of the name's kind; photos spread over three drawings so arrowing through them shows change.
    static func sample(for name: String) -> String? {
        switch (name as NSString).pathExtension.lowercased() {
        case "pdf": return "sample-2-pages.pdf"
        case "png": return "sample.png"
        case "jpg", "jpeg", "heic":
            let photos = ["photo-dawn.jpg", "photo-sea.jpg", "photo-dusk.jpg"]
            return photos[name.unicodeScalars.reduce(0) { $0 + Int($1.value) } % photos.count]
        case "mp4", "mov", "m4v": return "sample-3-seconds.mp4"
        case "doc", "docx": return "sample.docx"
        case "zip": return "sample.zip"
        case "svg": return "sample.svg"
        default: return nil
        }
    }

    private func copy(_ ref: AttachmentRef, update: @escaping @MainActor (AttachmentPhase) -> Void) {
        guard let dir = AttachmentPlaces.fixtures, let sample = Self.sample(for: ref.name) else {
            update(.unavailable(AttachmentWords.notOnThisMac)); return
        }
        let from = dir.appendingPathComponent(sample)
        let folder = AttachmentPlaces.cache(ref.messageId)
        let to = folder.appendingPathComponent(ref.name)
        Task { @MainActor in
            let phase = await Task.detached(priority: .userInitiated) { () -> AttachmentPhase in
                let fm = FileManager.default
                do {
                    try fm.createDirectory(at: folder, withIntermediateDirectories: true)
                    if fm.fileExists(atPath: to.path) { try fm.removeItem(at: to) }
                    try fm.copyItem(at: from, to: to)
                    return .ready(to)
                } catch {
                    return .unavailable(error.localizedDescription)
                }
            }.value
            update(phase)
        }
    }
}
#endif
