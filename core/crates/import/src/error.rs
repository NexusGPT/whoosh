//! What can stop an import, in words the first-start step shows. A permission refusal is a typed result, never a panic.
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// What the step says when macOS refuses to list ~/Library/Mail: how to grant it, and that skipping is fine.
pub const FULL_DISK_ACCESS_WORDS: &str = "Whoosh needs Full Disk Access to read Apple Mail. Open System Settings, choose Privacy & Security, then Full Disk Access, \
add Whoosh to the list and switch it on. You can also skip this step and import later from Settings, Accounts.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// macOS refused to read the folder. `words` is what the step shows (how to grant it, and the Skip).
    NeedsFullDiskAccess { path: PathBuf, words: String },
    /// The chosen file or folder is not there.
    NotFound { path: PathBuf, words: String },
    /// The whole source cannot be read (a file that is not an archive, for one). A single bad message is skipped by name instead.
    Unreadable { path: PathBuf, words: String },
    /// The local database refused the write. The words keep the real cause.
    Store { words: String },
}

impl ImportError {
    /// The sentence the step shows.
    pub fn words(&self) -> &str {
        match self {
            ImportError::NeedsFullDiskAccess { words, .. }
            | ImportError::NotFound { words, .. }
            | ImportError::Unreadable { words, .. }
            | ImportError::Store { words } => words,
        }
    }
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.words())
    }
}

impl std::error::Error for ImportError {}

/// True for the two answers macOS gives to a program that has no right to read a folder (EPERM and EACCES).
pub fn is_permission_refusal(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::PermissionDenied || e.raw_os_error() == Some(1) || e.raw_os_error() == Some(13)
}

/// An I/O error on a source path, in the typed result the step needs.
pub fn classify_io_error(path: &Path, e: &io::Error) -> ImportError {
    if is_permission_refusal(e) {
        ImportError::NeedsFullDiskAccess { path: path.to_path_buf(), words: FULL_DISK_ACCESS_WORDS.to_string() }
    } else if e.kind() == io::ErrorKind::NotFound {
        ImportError::NotFound { path: path.to_path_buf(), words: format!("{} is not there. Pick the file or folder again.", path.display()) }
    } else {
        ImportError::Unreadable { path: path.to_path_buf(), words: format!("{} could not be read: {e}", path.display()) }
    }
}
