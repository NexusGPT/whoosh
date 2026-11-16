//! body-r1: the core's own log file, `<data_dir>/core.log`, beside the `eprintln!` lines the installed app never keeps. One line
//! per failure the owner may ask about later (a body the server did not send, a body that could not be kept), with its time.
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Past this size the file keeps its newer half (cut at a line end). Proposed for docs/CONSTANTS.md in docs/lanes/body-r1.md.
pub const CORE_LOG_MAX_BYTES: u64 = 1 << 20;

/// One writer at a time, so two failures never interleave inside a line or race the truncation.
static WRITING: Mutex<()> = Mutex::new(());

/// The log file of a store opened at `<data_dir>/whoosh.sqlite`.
pub fn path_beside(store_path: &Path) -> PathBuf { store_path.parent().unwrap_or_else(|| Path::new(".")).join("core.log") }

/// Appends `line` with the time (epoch milliseconds) to the log at `path`, after truncating the file to its newer half when it
/// is past `CORE_LOG_MAX_BYTES`. A log that cannot be written says so on stderr and costs nothing else.
pub fn log_line(path: &Path, line: &str) {
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    if let Err(e) = write(path, line) { eprintln!("core.log {}: not written: {e}", path.display()); }
}

fn write(path: &Path, line: &str) -> std::io::Result<()> {
    if std::fs::metadata(path).map(|m| m.len() > CORE_LOG_MAX_BYTES).unwrap_or(false) {
        let bytes = std::fs::read(path)?;
        let half = bytes.len() / 2;
        let start = bytes[half..].iter().position(|&b| b == b'\n').map(|p| half + p + 1).unwrap_or(bytes.len());
        std::fs::write(path, &bytes[start..])?;
    }
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{} {}", crate::rust_core::now_ms(), line.replace('\n', " "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_append_and_a_full_log_keeps_its_newer_half() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("core.log");
        log_line(&p, "first");
        log_line(&p, "second\nline");
        let text = std::fs::read_to_string(&p).unwrap();
        assert_eq!(text.lines().count(), 2, "{text:?}");
        assert!(text.lines().nth(1).unwrap().ends_with("second line"));
        let filler = "x".repeat(1000);
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        for i in 0..1100 { writeln!(f, "{i} {filler}").unwrap(); }
        drop(f);
        assert!(std::fs::metadata(&p).unwrap().len() > CORE_LOG_MAX_BYTES);
        log_line(&p, "after the cut");
        let text = std::fs::read_to_string(&p).unwrap();
        assert!((text.len() as u64) < CORE_LOG_MAX_BYTES * 3 / 4, "{}", text.len());
        assert!(!text.contains("first") && text.ends_with("after the cut\n"), "the newer half and the new line stay");
        assert!(text.lines().all(|l| l.ends_with("after the cut") || l.ends_with(&filler)), "cut at a line end");
    }
}
