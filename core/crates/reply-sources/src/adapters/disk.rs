//! `FileTree` over std::fs. Entries are read with `symlink_metadata`, so a link is reported as a link and never followed. A path that
//! starts with "~/" is read under `home`. A sandboxed app must hold access to the folder (the Sources pane's folder picker grants it).
use crate::ports::{FileTree, TreeEntry, TreeEntryKind};
use contracts::CoreError;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub struct DiskTree {
    pub home: Option<PathBuf>,
}

impl DiskTree {
    /// Home from $HOME.
    pub fn from_env() -> DiskTree {
        DiskTree { home: std::env::var_os("HOME").map(PathBuf::from) }
    }

    fn resolve(&self, p: &Path) -> PathBuf {
        match (p.strip_prefix("~"), &self.home) {
            (Ok(rest), Some(home)) => home.join(rest),
            _ => p.to_path_buf(),
        }
    }
}

fn io_error(path: &Path, e: std::io::Error) -> CoreError {
    CoreError::Unexpected { reason_words: format!("{}: {e}", path.display()) }
}

impl FileTree for DiskTree {
    fn read_dir(&self, dir: &Path) -> Result<Vec<TreeEntry>, CoreError> {
        let real = self.resolve(dir);
        let mut out = Vec::new();
        for entry in fs::read_dir(&real).map_err(|e| io_error(dir, e))? {
            let entry = entry.map_err(|e| io_error(dir, e))?;
            let Ok(meta) = fs::symlink_metadata(entry.path()) else { continue };
            let ft = meta.file_type();
            let kind = if ft.is_symlink() {
                TreeEntryKind::Link
            } else if ft.is_dir() {
                TreeEntryKind::Dir
            } else if ft.is_file() {
                let modified_ms = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis() as i64);
                TreeEntryKind::File { size: meta.len(), modified_ms }
            } else {
                TreeEntryKind::Other
            };
            out.push(TreeEntry { name: entry.file_name().to_string_lossy().into_owned(), kind });
        }
        Ok(out)
    }

    fn read(&self, file: &Path, max_bytes: u64) -> Result<Vec<u8>, CoreError> {
        let real = self.resolve(file);
        // the walk never hands a link, but a file may be replaced by one between the walk and the read
        if fs::symlink_metadata(&real).map_err(|e| io_error(file, e))?.file_type().is_symlink() {
            return Err(CoreError::Unexpected { reason_words: format!("{}: became a link, not followed", file.display()) });
        }
        let mut buf = Vec::new();
        fs::File::open(&real).map_err(|e| io_error(file, e))?.take(max_bytes).read_to_end(&mut buf).map_err(|e| io_error(file, e))?;
        Ok(buf)
    }
}
