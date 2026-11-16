//! On-disk layout: one generation = a mapped base, an optional in-memory delta, an optional tombstone list.
use contracts::CoreError;
use std::{collections::HashSet, fs, io, path::{Path, PathBuf}};

pub fn base_path(dir: &Path, generation: u64) -> PathBuf { dir.join(format!("base-{generation}.usearch")) }
pub fn delta_path(dir: &Path, generation: u64) -> PathBuf { dir.join(format!("delta-{generation}.usearch")) }
pub fn dead_path(dir: &Path, generation: u64) -> PathBuf { dir.join(format!("dead-{generation}.bin")) }

pub fn io_err(what: &str, e: impl std::fmt::Display) -> CoreError {
    CoreError::Unexpected { reason_words: format!("the vector index could not {what}: {e}") }
}

/// The newest generation that has a base file, if any.
pub fn latest_generation(dir: &Path) -> Result<Option<u64>, CoreError> {
    let mut best: Option<u64> = None;
    for entry in fs::read_dir(dir).map_err(|e| io_err("read its folder", e))? {
        let name = entry.map_err(|e| io_err("read its folder", e))?.file_name().to_string_lossy().into_owned();
        if let Some(g) = name.strip_prefix("base-").and_then(|r| r.strip_suffix(".usearch")).and_then(|g| g.parse::<u64>().ok()) {
            best = Some(best.map_or(g, |b| b.max(g)));
        }
    }
    Ok(best)
}

/// Removes every generation file except the one in use (a compaction that died leaves them behind).
pub fn sweep_other_generations(dir: &Path, keep: u64) -> Result<(), CoreError> {
    for entry in fs::read_dir(dir).map_err(|e| io_err("read its folder", e))? {
        let path = entry.map_err(|e| io_err("read its folder", e))?.path();
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let generation = ["base-", "delta-", "dead-"].iter().find_map(|p| name.strip_prefix(p)).and_then(|r| r.split('.').next()).and_then(|g| g.parse::<u64>().ok());
        let stale_tmp = name.ends_with(".tmp");
        if stale_tmp || generation.is_some_and(|g| g != keep) {
            fs::remove_file(&path).map_err(|e| io_err("remove an old file", e))?;
        }
    }
    Ok(())
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), CoreError> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes).map_err(|e| io_err("write a file", e))?;
    fs::rename(&tmp, path).map_err(|e| io_err("replace a file", e))
}

pub fn read_dead(path: &Path) -> Result<HashSet<u64>, CoreError> {
    match fs::read(path) {
        Ok(bytes) => Ok(bytes.chunks_exact(8).map(|c| u64::from_le_bytes(c.try_into().expect("8 bytes"))).collect()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(HashSet::new()),
        Err(e) => Err(io_err("read its removed-message list", e)),
    }
}

pub fn write_dead(path: &Path, dead: &HashSet<u64>) -> Result<(), CoreError> {
    let mut sorted: Vec<u64> = dead.iter().copied().collect();
    sorted.sort_unstable();
    write_atomic(path, &sorted.iter().flat_map(|k| k.to_le_bytes()).collect::<Vec<u8>>())
}
