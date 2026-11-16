//! Nested labels (owner, 2026-11-07: "labelling and sublabelling (e.g., client>Provanta) and
//! sub-sub-labelling (up to like 3-5 levels)"). A label is named by its PATH: the names from the
//! root down, joined by " > ". Depth = the number of names, 1..=LABEL_DEPTH_MAX.
use crate::{constants::LABEL_DEPTH_MAX, CoreError};

/// How a path is written: names joined by this separator.
pub const LABEL_PATH_SEPARATOR: &str = " > ";

/// Splits a path typed by the owner ("Clients>Provanta", "Clients > Provanta") into its names.
/// Refuses an empty name and a path deeper than LABEL_DEPTH_MAX, in words.
#[uniffi::export]
pub fn parse_label_path(path: String) -> Result<Vec<String>, CoreError> {
    let names: Vec<String> = path.split('>').map(|n| n.trim().to_string()).collect();
    if names.iter().any(|n| n.is_empty()) {
        return Err(CoreError::Invalid { field: "label".into(), reason_words: format!("\"{path}\" has an empty label name") });
    }
    if names.len() as u64 > LABEL_DEPTH_MAX {
        return Err(CoreError::Invalid {
            field: "label".into(),
            reason_words: format!("a label can be at most {LABEL_DEPTH_MAX} levels deep; \"{path}\" has {}", names.len()),
        });
    }
    Ok(names)
}

/// The canonical path of a list of names.
#[uniffi::export]
pub fn label_path(names: Vec<String>) -> String {
    names.join(LABEL_PATH_SEPARATOR)
}
