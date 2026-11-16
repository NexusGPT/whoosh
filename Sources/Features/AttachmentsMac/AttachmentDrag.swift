#if os(macOS)
import AppKit
import UniformTypeIdentifiers

/// What a chip carries when dragged out: the file URL when the bytes are here (the Finder, the Desktop or another app
/// copies the file), else a file promise the receiver fulfils once the download lands. The drag image is the file's
/// own picture: its thumbnail, or the system icon for its type, as the Finder shows a dragged file.
@MainActor
enum AttachmentDrag {
    static func writer(_ ref: AttachmentRef) -> NSPasteboardWriting? {
        if AttachmentRig.mutant("no-drag") { return nil }
        if let url = AttachmentStore.shared.peek(ref).url { return url as NSURL }
        let type = UTType(filenameExtension: (ref.name as NSString).pathExtension.lowercased()) ?? .data
        return NSFilePromiseProvider(fileType: type.identifier, delegate: AttachmentPromises.delegate(for: ref))
    }

    static func item(_ ref: AttachmentRef, frame: NSRect) -> NSDraggingItem? {
        guard let w = writer(ref) else { return nil }
        let item = NSDraggingItem(pasteboardWriter: w)
        let image: NSImage
        if let url = AttachmentStore.shared.peek(ref).url { image = NSWorkspace.shared.icon(forFile: url.path) } else { image = AttachmentThumbnails.icon(ref) }
        let side = min(frame.height, frame.width)
        item.setDraggingFrame(NSRect(x: frame.minX, y: frame.midY - side / 2, width: side, height: side), contents: AttachmentThumbnails.shared.images[ref] ?? image)
        return item
    }

    /// The same writer a drag would carry, put on the rig's drag pasteboard so a check can read what a drop receives.
    static func rehearse(_ ref: AttachmentRef) -> [String: Any]? {
        guard let w = writer(ref) else { return nil }
        let pb = NSPasteboard(name: NSPasteboard.Name("io.nexus.whoosh.rig.drag"))
        pb.clearContents()
        guard pb.writeObjects([w]) else { return nil }
        let file = (pb.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL])?.first
        let exists = file.map { FileManager.default.fileExists(atPath: $0.path) } ?? false
        return ["drag_kind": w is NSFilePromiseProvider ? "promise" : "file", "drag_types": (pb.types ?? []).map(\.rawValue),
                "drag_file": file?.lastPathComponent ?? "", "drag_file_exists": exists]
    }
}

/// TEST COPY ONLY (`drag <attachment> desktop` on a rig, J2.4): what the Finder's desktop does with a dropped attachment, done in
/// `$WHOOSH_SOCKET_DIR/desktop` (never ~/Desktop) from the SAME `writer` the real drag carries: a file URL is copied under a free name
/// (as the Finder does), a file promise is asked to write there. `core state desktop/files` lists the names, `desktop/bytes` each size.
/// Off a rig there is no folder, so nothing is written and the drop is refused.
@MainActor
enum AttachmentDesk {
    static let target = "desktop"
    static var folder: URL? { AttachmentPlaces.rigDir?.appendingPathComponent("desktop", isDirectory: true) }

    /// `from`: a reader tile `mac-main.reader.attachment.<i>` or its chip `mac-main.attachments.<i>`.
    static func drop(_ from: String, _ m: AppModel) -> Bool {
        guard let dir = folder, from.hasPrefix("mac-main.reader.attachment.") || from.hasPrefix(AttachmentSocket.reader + "."),
              let i = Int(from.split(separator: ".").last ?? ""), let ref = AttachmentStore.shared.readerRefs(m).first(where: { $0.index == i }),
              let w = AttachmentDrag.writer(ref), (try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)) != nil else { return false }
        if let url = (w as? NSURL).map({ $0 as URL }) {
            return (try? FileManager.default.copyItem(at: url, to: AttachmentPlaces.free(url.lastPathComponent, in: dir))) != nil
        }
        guard let p = w as? NSFilePromiseProvider, let d = p.delegate else { return false }
        d.filePromiseProvider(p, writePromiseTo: AttachmentPlaces.free(d.filePromiseProvider(p, fileNameForType: p.fileType), in: dir)) { _ in }
        return true
    }

    /// The value at `desktop/files` or `desktop/bytes`; nil off a rig or for another path.
    static func value(_ path: String) -> Any? {
        guard let dir = folder else { return nil }
        let names = ((try? FileManager.default.contentsOfDirectory(atPath: dir.path)) ?? []).filter { !$0.hasPrefix(".") }.sorted()
        switch path {
        case "desktop/files": return names
        case "desktop/bytes":
            return names.map { n -> [String: Any] in
                ["name": n, "bytes": (try? FileManager.default.attributesOfItem(atPath: dir.appendingPathComponent(n).path)[.size] as? Int) ?? 0] }
        default: return nil
        }
    }
}

/// File promises for attachments not downloaded yet: the receiver names the folder; the bytes are copied there when
/// they land (or the promise fails in words when they cannot come).
final class AttachmentPromises: NSObject, NSFilePromiseProviderDelegate, @unchecked Sendable {
    let ref: AttachmentRef
    private init(_ ref: AttachmentRef) { self.ref = ref }
    @MainActor private static var live: [AttachmentRef: AttachmentPromises] = [:]
    @MainActor static func delegate(for ref: AttachmentRef) -> AttachmentPromises {
        if let d = live[ref] { return d }
        let d = AttachmentPromises(ref); live[ref] = d; return d
    }

    func filePromiseProvider(_ p: NSFilePromiseProvider, fileNameForType fileType: String) -> String { ref.name }

    func filePromiseProvider(_ p: NSFilePromiseProvider, writePromiseTo url: URL, completionHandler: @escaping (Error?) -> Void) {
        let ref = self.ref
        nonisolated(unsafe) let done = completionHandler
        Task { @MainActor in
            guard let src = await AttachmentStore.shared.file(ref) else { done(CocoaError(.fileReadNoSuchFile)); return }
            do { try FileManager.default.copyItem(at: src, to: url); done(nil) } catch { done(error) }
        }
    }
}
#endif
