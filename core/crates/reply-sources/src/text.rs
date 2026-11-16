//! The text and title of one file, by its kind, through the readers the attachments already use: plain text and markdown read here,
//! HTML through the mail bodies' sanitiser (crate parse), PDF through attachment-index's reader and else the app's PDFKit, and
//! word-processor, sheet and slide files through the app's document readers (`TextReader`).
use crate::constants::REPLY_FILE_MAX_BYTES;
use crate::ports::FileTree;
use crate::walk::FoundFile;
use attachment_index::{extract_pdf_text, AttachmentKind, TextReader};
use contracts::CoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileText {
    pub title: String,
    pub text: String,
}

fn ext(name: &str) -> String {
    name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
}

fn stem(rel: &str) -> String {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    match name.rsplit_once('.') {
        Some((s, _)) if !s.is_empty() => s.to_string(),
        _ => name.to_string(),
    }
}

fn markdown_title(text: &str) -> Option<String> {
    text.lines().map(str::trim).find_map(|l| l.strip_prefix("# ")).map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

fn html_title(html: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let open = lower.find("<title")?;
    let start = open + lower[open..].find('>')? + 1;
    let end = start + lower[start..].find("</title")?;
    let t = html[start..end].split_whitespace().collect::<Vec<_>>().join(" ");
    (!t.is_empty()).then_some(t)
}

fn cap(mut text: String, max_chars: Option<usize>) -> String {
    if let Some(max) = max_chars {
        if let Some((i, _)) = text.char_indices().nth(max) {
            text.truncate(i);
        }
    }
    text
}

/// The text of one found file, at most `max_chars` characters (the ledger's ATTACHMENT_TEXT_MAX_CHARS); None when it holds none.
pub fn read_file_text(tree: &dyn FileTree, reader: &dyn TextReader, file: &FoundFile, max_chars: Option<usize>) -> Result<Option<FileText>, CoreError> {
    let mut title = stem(&file.rel);
    let text = match (file.kind, ext(&file.rel).as_str()) {
        (AttachmentKind::Document, "html" | "htm") => {
            let html = String::from_utf8_lossy(&tree.read(&file.path, REPLY_FILE_MAX_BYTES)?).into_owned();
            if let Some(t) = html_title(&html) {
                title = t;
            }
            Some(parse::safe_html_to_text(&parse::sanitise_html(&html).html))
        }
        (AttachmentKind::PlainText, e) => {
            let text = String::from_utf8_lossy(&tree.read(&file.path, REPLY_FILE_MAX_BYTES)?).into_owned();
            if matches!(e, "md" | "markdown") {
                if let Some(t) = markdown_title(&text) {
                    title = t;
                }
            }
            Some(text)
        }
        (AttachmentKind::Pdf, _) => match extract_pdf_text(&tree.read(&file.path, REPLY_FILE_MAX_BYTES)?) {
            Some(t) => Some(t),
            None => reader.read_text(&file.path, AttachmentKind::Pdf)?,
        },
        (AttachmentKind::Document, _) => reader.read_text(&file.path, AttachmentKind::Document)?,
        _ => None,
    };
    Ok(text.filter(|t| !t.trim().is_empty()).map(|t| FileText { title, text: cap(t, max_chars) }))
}
