import Foundation

/// Builds the attachment index the fixture core serves from a corpus's `attachment_content` map (fixtures/tests-j2.json): a downloaded
/// file is a small stub on disk, read by fixture readers that return the corpus's text; a file "not-downloaded" has no local file.
public struct FixtureAttachmentSpec: Sendable {
    public let fileName: String, message: String, kind: String, state: String, text: String
    public init(fileName: String, message: String, kind: String, state: String, text: String) {
        self.fileName = fileName; self.message = message; self.kind = kind; self.state = state; self.text = text
    }
}

public enum FixtureAttachments {
    public static func book(from specs: [FixtureAttachmentSpec]) -> AttachmentIndexBook? {
        let entries = specs.sorted { $0.fileName < $1.fileName }
        guard !entries.isEmpty else { return nil }
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent("whoosh-fixture-attachments-\(ProcessInfo.processInfo.processIdentifier)", isDirectory: true)
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        var words: [String: String] = [:]
        var items: [AttachmentIndexBook.Item] = []
        for entry in entries {
            let name = entry.fileName
            let mime = ["image": "image/jpeg", "pdf": "application/pdf", "cad": "image/vnd.dwg"][entry.kind] ?? "application/octet-stream"
            let local = folder.appendingPathComponent(name)
            if entry.state != "not-downloaded" { try? Data(name.utf8).write(to: local) }
            if !entry.text.isEmpty { words[name] = entry.text }
            items.append(.init(messageId: entry.message, attachmentId: "att-\(name)", fileName: name, mimeType: mime, localURL: entry.state == "not-downloaded" ? nil : local))
        }
        let scanner = AttachmentScanner(text: FixtureTextReader(texts: words), pictures: FixturePictureReader(words: words), speech: FixtureSpeech(),
                                        video: FixtureVideoProbe(duration: 0), frameEveryS: Double(WhooshConstants.VIDEO_FRAME_EVERY_S), framesMax: WhooshConstants.VIDEO_FRAMES_MAX)
        let book = AttachmentIndexBook(embedder: FixtureEmbedder(), scanner: scanner, scratch: folder.appendingPathComponent("scratch"))
        book.index(items)
        return book
    }
}
