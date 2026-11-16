//! The files of a folder the replies may read: every folder under it, but not hidden ones, not dependency caches, not macOS packages,
//! and never through a link; only files of a kind the app reads as text (plain text, markdown, HTML, PDF, word-processor, sheet and
//! slide files), and none over REPLY_FILE_MAX_BYTES.
use crate::constants::{REPLY_FILE_MAX_BYTES, REPLY_SKIP_DIRS, REPLY_SKIP_DIR_EXT};
use crate::ports::{FileTree, TreeEntryKind};
use attachment_index::AttachmentKind;
use contracts::CoreError;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundFile {
    /// the path inside the folder, with "/" ("Contrats/2026/acme.pdf")
    pub rel: String,
    /// the folder's path joined with `rel`
    pub path: PathBuf,
    pub size: u64,
    pub modified_ms: i64,
    pub kind: AttachmentKind,
}

/// What the walk left out, by reason (the note under a tile can say "12 fichiers trop gros").
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WalkSkips {
    pub hidden: u32,
    pub skipped_dirs: u32,
    pub links: u32,
    pub too_big: u32,
    /// images, sound, video, archives: not text
    pub not_text: u32,
    /// sub-folders that would not open (no permission)
    pub unopened_dirs: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Walk {
    /// sorted by `rel`
    pub files: Vec<FoundFile>,
    pub skips: WalkSkips,
}

fn is_text(kind: AttachmentKind) -> bool {
    matches!(kind, AttachmentKind::PlainText | AttachmentKind::Pdf | AttachmentKind::Document)
}

fn skipped_dir(name: &str) -> bool {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    REPLY_SKIP_DIRS.contains(&name) || REPLY_SKIP_DIR_EXT.contains(&ext.as_str())
}

/// Walks `root`. Err only when the folder itself will not open; a sub-folder that will not open is counted and left out.
pub fn walk_folder(tree: &dyn FileTree, root: &Path) -> Result<Walk, CoreError> {
    let mut walk = Walk::default();
    let mut stack: Vec<String> = vec![String::new()];
    let mut first = true;
    while let Some(rel_dir) = stack.pop() {
        let dir = if rel_dir.is_empty() { root.to_path_buf() } else { root.join(&rel_dir) };
        let entries = match tree.read_dir(&dir) {
            Ok(e) => e,
            Err(e) if first => return Err(e),
            Err(_) => {
                walk.skips.unopened_dirs += 1;
                continue;
            }
        };
        first = false;
        for entry in entries {
            // a name is one component: a "/" or ".." in it is not an entry of this folder
            if entry.name.is_empty() || entry.name.contains('/') || entry.name == ".." {
                continue;
            }
            if entry.name.starts_with('.') {
                walk.skips.hidden += 1;
                continue;
            }
            let rel = if rel_dir.is_empty() { entry.name.clone() } else { format!("{rel_dir}/{}", entry.name) };
            match entry.kind {
                TreeEntryKind::Link => walk.skips.links += 1,
                TreeEntryKind::Other => walk.skips.not_text += 1,
                TreeEntryKind::Dir if skipped_dir(&entry.name) => walk.skips.skipped_dirs += 1,
                TreeEntryKind::Dir => stack.push(rel),
                TreeEntryKind::File { size, modified_ms } => {
                    let kind = AttachmentKind::detect("", &entry.name);
                    if !is_text(kind) {
                        walk.skips.not_text += 1;
                    } else if size > REPLY_FILE_MAX_BYTES {
                        walk.skips.too_big += 1;
                    } else {
                        walk.files.push(FoundFile { path: root.join(&rel), rel, size, modified_ms, kind });
                    }
                }
            }
        }
    }
    walk.files.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(walk)
}
