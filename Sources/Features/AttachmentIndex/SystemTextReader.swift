import Foundation
import PDFKit
#if canImport(AppKit)
import AppKit
#elseif canImport(UIKit)
import UIKit
#endif

/// PDFKit reads a PDF's text layer; the system's document readers read rtf, doc, docx, odt and html. Spreadsheets and slides
/// are not read here (nil: searched by name). A scanned PDF has no text layer and gives nil as well.
public struct SystemTextReader: AttachmentTextReading {
    public init() {}

    public func readText(at url: URL, kind: ReadableKind) throws -> String? {
        switch kind {
        case .pdf:
            guard let document = PDFDocument(url: url) else { throw AttachmentReadError.unreadable("this PDF could not be opened") }
            if document.isLocked { throw AttachmentReadError.unreadable("this PDF is protected by a password") }
            return Self.trimmed(document.string)
        case .document:
            do {
                let attributed = try NSAttributedString(url: url, options: [:], documentAttributes: nil)
                return Self.trimmed(attributed.string)
            } catch {
                throw AttachmentReadError.unreadable("this document could not be read: \(error.localizedDescription)")
            }
        case .image, .audio, .video:
            return nil
        }
    }

    private static func trimmed(_ s: String?) -> String? {
        guard let s = s?.trimmingCharacters(in: .whitespacesAndNewlines), !s.isEmpty else { return nil }
        return s
    }
}
