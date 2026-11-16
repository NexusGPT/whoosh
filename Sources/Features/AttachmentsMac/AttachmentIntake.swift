#if os(macOS)
import AppKit
import UniformTypeIdentifiers

/// The composer's intake: files dropped on the compose area, and images (or copied files) pasted with ⌘V, become
/// pending attachments with the same preview, open and remove as a received one. A paste that holds text only is
/// left to the editor. Under the rig, paste reads the rig's private pasteboard and pasted images land in the run dir.
@MainActor
enum AttachmentIntake {
    static let types: [UTType] = [.fileURL, .image]

    /// The compose area's drop: file URLs first, else the image data of each item.
    static func drop(_ providers: [NSItemProvider]) -> Bool {
        installOnce()
        if AttachmentRig.mutant("no-intake") { return true }
        for p in providers {
            if p.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) {
                _ = p.loadObject(ofClass: URL.self) { url, _ in if let url { Task { @MainActor in take(url) } } }
            } else if p.hasItemConformingToTypeIdentifier(UTType.image.identifier) {
                p.loadDataRepresentation(forTypeIdentifier: UTType.image.identifier) { data, _ in
                    guard let data else { return }
                    takeImage(data)
                }
            }
        }
        return true
    }

    /// ⌘V while the composer holds the keys: copied files or an image become attachments; text passes on.
    static func paste(_ m: AppModel) -> Bool {
        guard ComposerModel.shared.isOpen, ComposerMacFeature.hasFocus(m) else { return false }
        let pb = AttachmentPasteboard.board
        let files = (pb.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL]) ?? []
        let image = pb.data(forType: .png) ?? pb.data(forType: .tiff)
        guard !files.isEmpty || image != nil else { return false }
        if AttachmentRig.mutant("no-intake") { return true }
        files.forEach(take)
        if files.isEmpty, let data = image { takeImageInline(data) }   // picture DATA goes into the text (lead ruling, round 7)
        return true
    }

    /// ⌘V in a text that the composer owns: the pasteboard's file URLs are attached; with none, a picture copied as DATA (no text, no
    /// HTML beside it: a copied web page keeps its HTML paste) is written to a file and attached. False when it is a text paste.
    static func takePasted(_ pb: NSPasteboard) -> Bool {
        let files = (pb.readObjects(forClasses: [NSURL.self], options: [.urlReadingFileURLsOnly: true]) as? [URL]) ?? []
        if !files.isEmpty {
            if AttachmentRig.mutant("paste-as-text") { return false }
            files.forEach(take)
            return true
        }
        guard pb.string(forType: .string) == nil, pb.string(forType: .html) == nil, let image = pb.data(forType: .png) ?? pb.data(forType: .tiff) else { return false }
        if AttachmentRig.mutant("paste-as-text") { return false }
        takeImageInline(image)
        return true
    }

    static func take(_ url: URL) {
        guard ComposerModel.shared.attach(url) else { return }
        AttachmentStore.shared.intakeLog.append(url.lastPathComponent)
    }

    /// A pasted or dropped image becomes a pending file. The decode and the PNG encode (a screenshot is megabytes) run on a utility queue, never on
    /// the main thread that holds the keys; the tile appears when the file is written.
    nonisolated static func takeImage(_ data: Data) {
        DispatchQueue.global(qos: .utility).async {
            guard let u = writeImage(data) else { return }
            Task { @MainActor in take(u) }
        }
    }

    /// A picture copied as DATA and pasted (lead ruling, round 7, as mail clients do): it goes INTO THE TEXT at the caret, the way a drop
    /// on the text does (`ComposerModel.insertPicture`; one too large, or not a picture, is attached as a file there). Its file is written under
    /// the app's data folder first. A copied FILE, a picture file included, stays a tile (`take`).
    nonisolated static func takeImageInline(_ data: Data) {
        DispatchQueue.global(qos: .utility).async {
            guard let u = writeImage(data) else { return }
            Task { @MainActor in
                if AttachmentRig.mutant("picture-as-tile") { take(u); return }   // the broken copy: the old rule, a tile
                _ = ComposerModel.shared.insertPicture(u)
                AttachmentStore.shared.intakeLog.append(u.lastPathComponent)
            }
        }
    }

    /// A pasted or dropped image as a PNG file, named as the system names a pasted image. Safe off the main thread.
    nonisolated static func writeImage(_ data: Data) -> URL? {
        guard let rep = NSBitmapImageRep(data: data), let png = rep.representation(using: .png, properties: [:]) else { return nil }
        let temp = FileManager.default.temporaryDirectory.appendingPathComponent("whoosh-pasted", isDirectory: true)   // the mutant's old home
        guard let dir = AttachmentRig.mutant("paste-to-temp") ? ((try? FileManager.default.createDirectory(at: temp, withIntermediateDirectories: true)) != nil ? temp : nil)
                                                              : AttachmentPlaces.newPastedFolder() else { return nil }   // the app's own data folder, never the system's temporary one
        let url = AttachmentPlaces.free(Words.text("Pasted image") + ".png", in: dir)
        return (try? png.write(to: url)) != nil ? url : nil
    }

    /// The rig's `fixture drop-file mac-composer… <file>` lands here, as a drop on the compose area would.
    static func installOnce() { _ = observer }
    private static let observer: NSObjectProtocol = NotificationCenter.default.addObserver(
        forName: FixtureVerbs.dropFile, object: nil, queue: .main) { n in
        guard (n.userInfo?["target"] as? String)?.hasPrefix("mac-composer") == true, let f = n.userInfo?["file"] as? String else { return }
        MainActor.assumeIsolated {
            if AttachmentRig.mutant("no-intake") { return }
            if !ComposerModel.shared.isOpen { _ = ComposerModel.shared.openNew(.shared) }
            take(URL(fileURLWithPath: f))
        }
    }
}
#endif
