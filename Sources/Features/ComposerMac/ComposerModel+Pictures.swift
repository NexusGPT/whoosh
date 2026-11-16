import Foundation
import SwiftUI
#if os(macOS)
import UniformTypeIdentifiers
#endif

/// gap-composer-rich: a picture pasted or dropped INTO the text, and the signature shown with its formatting.
/// The picture is one of the core draft's files (`AddDraftAttachment`, as a tile's file is); the text holds a picture run naming the
/// file's core id, put at the caret by the editor (`RichTextViewHooks.insertPicture`). The composer reads only the file's size from the
/// disk (no decode here, nothing on the key's turn but the run itself); a picture over `ComposerRichMail.inlinePictureMaxBytes` or a
/// file that is not a picture is attached as a file, exactly as a drop outside the text is.
extension ComposerModel {
    /// The signature under the text: a plain one as plain text (as before), one with marks drawn with its bold, italic and links.
    var signatureText: Text {
        let s = signature
        return ComposerRichMail.isRich(s) ? Text(ComposerRichMail.shown(s)) : Text(s)
    }

    /// A picture into the text at the caret. True when it went into the text; false when it went as a file instead (or nowhere).
    @discardableResult
    public func insertPicture(_ url: URL) -> Bool {
        guard isOpen, url.isFileURL else { return false }
        let size = UInt64((try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0)
        guard ComposerRichMail.isPicture(url), size > 0, size <= ComposerRichMail.inlinePictureMaxBytes else { attach(url); return false }
        do {
            guard let id = try addSource(.localFile(path: url.path)) else { attach(url); return false }
            RichPictures.shared.register(cid: id, url: url)   // the text draws this picture's thumbnail, not the symbol
            placePicture(id)
            refreshFiles()
            return true
        } catch {
            line = Self.words(error)
            return false
        }
    }

    /// The picture run goes in at the caret through the editor; with no editor on screen (a closed window, a unit test) it ends the text.
    func placePicture(_ id: String) {
        session.hooks.insertPicture(id)
        if !ComposerRichMail.pictureIds(session.document).contains(id) {
            let safe = id.filter { $0.isLetter || $0.isNumber || "-._:@".contains($0) }
            session.paste(html: "<img src=\"cid:\(safe)\">")
        }
    }

    #if os(macOS)
    /// A drop ON THE TEXT: each picture goes into the text; any other file is attached as a drop on the composer is. Image data with no
    /// file (a picture dragged out of a browser) is written to a file first, off the main thread.
    func dropOnText(_ providers: [NSItemProvider]) -> Bool {
        guard isOpen else { return false }
        for p in providers {
            if p.hasItemConformingToTypeIdentifier(UTType.fileURL.identifier) {
                _ = p.loadObject(ofClass: URL.self) { url, _ in
                    guard let url else { return }
                    Task { @MainActor in _ = ComposerModel.shared.insertPicture(url) }
                }
            } else if p.hasItemConformingToTypeIdentifier(UTType.image.identifier) {
                p.loadDataRepresentation(forTypeIdentifier: UTType.image.identifier) { data, _ in
                    guard let data, let url = ComposerModel.writePicture(data) else { return }
                    Task { @MainActor in _ = ComposerModel.shared.insertPicture(url) }
                }
            }
        }
        return true
    }

    /// Picture bytes to a file in this Mac's temporary folder, in their own format (no decode, no re-encode). Runs on the provider's queue.
    nonisolated static func writePicture(_ data: Data) -> URL? {
        guard data.count <= Int(ComposerRichMail.inlinePictureMaxBytes) else { return nil }
        let ext = data.starts(with: [0x89, 0x50, 0x4E, 0x47]) ? "png" : data.starts(with: [0xFF, 0xD8]) ? "jpg" : data.starts(with: [0x47, 0x49, 0x46]) ? "gif" : "tiff"
        guard let dir = AttachmentPlaces.newPastedFolder() else { return nil }   // the app's data folder: the engine reads it at send
        let url = dir.appendingPathComponent(UUID().uuidString + "." + ext)
        return (try? data.write(to: url)) != nil ? url : nil
    }
    #endif

    /// RIG ONLY (`mac-composer.dev.picture`): a small PNG written to the temporary folder goes into the text, standing for a drop on it.
    func rigPicture() -> Bool {
        guard isOpen else { return false }
        // a 1 x 1 PNG, the smallest real picture
        let png: [UInt8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
                            0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
                            0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82]
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("whoosh-rig-picture.png")
        guard (try? Data(png).write(to: url)) != nil else { return false }
        return insertPicture(url)
    }

    /// RIG ONLY (`mac-composer.dev.rich-signature`, `.plain-signature`): the From account's signature becomes a fixed one with marks, or a plain one.
    func rigSignature(rich: Bool) -> Bool {
        guard isOpen, !fromId.isEmpty else { return false }
        let text = rich ? FixtureCore.rigRichSignature : FixtureCore.rigPlainSignature
        return (try? AppModel.shared.core.run(command: .setSignature(accountId: fromId, text: text))) != nil
    }
}
