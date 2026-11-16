//! Which reading an attachment gets, from its MIME type and file name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentKind {
    Pdf,
    /// text the core reads itself (txt, csv, markdown, json, log)
    PlainText,
    /// word-processor, sheet and slide files: the Swift reader answers
    Document,
    Image,
    Audio,
    Video,
    /// a kind the app cannot read (CAD, archives, installers): searched by name only
    Unreadable,
}

const UNREADABLE_EXT: &[&str] = &["dwg", "dxf", "zip", "gz", "tar", "7z", "rar", "dmg", "exe", "pkg", "iso", "psd", "ai"];
const PLAIN_EXT: &[&str] = &["txt", "text", "md", "markdown", "csv", "tsv", "log", "json", "xml", "ics", "vcf"];
const DOCUMENT_EXT: &[&str] = &["doc", "docx", "rtf", "odt", "pages", "xls", "xlsx", "numbers", "ppt", "pptx", "key", "html", "htm"];
const IMAGE_EXT: &[&str] = &["jpg", "jpeg", "png", "heic", "heif", "gif", "webp", "tif", "tiff", "bmp"];
const AUDIO_EXT: &[&str] = &["m4a", "mp3", "wav", "aac", "aiff", "caf", "flac", "ogg", "opus"];
const VIDEO_EXT: &[&str] = &["mp4", "mov", "m4v", "avi", "mkv", "webm"];

impl AttachmentKind {
    pub fn detect(mime: &str, file_name: &str) -> AttachmentKind {
        let mime = mime.trim().to_ascii_lowercase();
        let ext = file_name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        let ext = ext.as_str();
        if UNREADABLE_EXT.contains(&ext) || mime.contains("dwg") || mime.contains("zip") {
            return AttachmentKind::Unreadable;
        }
        if mime == "application/pdf" || ext == "pdf" {
            AttachmentKind::Pdf
        } else if mime.starts_with("image/") || IMAGE_EXT.contains(&ext) {
            AttachmentKind::Image
        } else if mime.starts_with("audio/") || AUDIO_EXT.contains(&ext) {
            AttachmentKind::Audio
        } else if mime.starts_with("video/") || VIDEO_EXT.contains(&ext) {
            AttachmentKind::Video
        } else if mime == "text/html" || DOCUMENT_EXT.contains(&ext) || mime.contains("officedocument") || mime == "application/msword" || mime == "application/rtf" {
            AttachmentKind::Document
        } else if mime.starts_with("text/") || PLAIN_EXT.contains(&ext) {
            AttachmentKind::PlainText
        } else {
            AttachmentKind::Unreadable
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AttachmentKind::Pdf => "pdf",
            AttachmentKind::PlainText => "text",
            AttachmentKind::Document => "document",
            AttachmentKind::Image => "image",
            AttachmentKind::Audio => "audio",
            AttachmentKind::Video => "video",
            AttachmentKind::Unreadable => "unreadable",
        }
    }
}
