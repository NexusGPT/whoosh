import Foundation

public enum AttachmentFileKind: Equatable, Sendable {
    case readable(ReadableKind)
    case plainText
    /// a kind the app cannot read (CAD, archives, installers): searched by name only
    case unreadable

    /// The same rules as the Rust `AttachmentKind::detect`: unreadable extensions first, then MIME type, then extension.
    public static func detect(mime: String, fileName: String) -> AttachmentFileKind {
        let mime = mime.lowercased(), ext = (fileName as NSString).pathExtension.lowercased()
        let unreadable: Set = ["dwg", "dxf", "zip", "gz", "tar", "7z", "rar", "dmg", "exe", "pkg", "iso", "psd", "ai"]
        let documents: Set = ["doc", "docx", "rtf", "odt", "html", "htm"]
        let plain: Set = ["txt", "text", "md", "markdown", "csv", "tsv", "log", "json", "xml", "ics", "vcf"]
        if unreadable.contains(ext) || mime.contains("dwg") || mime.contains("zip") { return .unreadable }
        if mime == "application/pdf" || ext == "pdf" { return .readable(.pdf) }
        if mime.hasPrefix("image/") || ["jpg", "jpeg", "png", "heic", "heif", "gif", "webp", "tif", "tiff", "bmp"].contains(ext) { return .readable(.image) }
        if mime.hasPrefix("audio/") || ["m4a", "mp3", "wav", "aac", "aiff", "caf", "flac", "ogg", "opus"].contains(ext) { return .readable(.audio) }
        if mime.hasPrefix("video/") || ["mp4", "mov", "m4v", "avi", "mkv", "webm"].contains(ext) { return .readable(.video) }
        if documents.contains(ext) || mime.contains("officedocument") || mime == "application/msword" || mime == "application/rtf" || mime == "text/html" { return .readable(.document) }
        if mime.hasPrefix("text/") || plain.contains(ext) { return .plainText }
        return .unreadable
    }
}
