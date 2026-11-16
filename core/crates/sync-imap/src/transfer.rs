//! MOVE/COPY across accounts. IMAP has no such command, so the message travels through this Mac:
//! fetch from the source, APPEND to the target, and only then (for a move) delete at the source.
//! A refusal by the target leaves the source untouched.
use contracts::CoreError;

use crate::{adapter::ImapAdapter, port::ImapMailAdapter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferMode { Copy, Move }

/// Flags that describe the message, not its place in the source folder.
fn carried(flags: &[String]) -> Vec<String> {
    flags.iter().filter(|f| !f.eq_ignore_ascii_case("\\Deleted") && !f.eq_ignore_ascii_case("\\Recent")).cloned().collect()
}

pub async fn transfer(
    src: &ImapAdapter,
    src_folder: &str,
    uids: &[u32],
    dst: &ImapAdapter,
    dst_folder: &str,
    mode: TransferMode,
) -> Result<(), CoreError> {
    // Fetch every message first: a failure here costs nothing at either end.
    let mut held = Vec::with_capacity(uids.len());
    for uid in uids {
        held.push(src.fetch_raw(src_folder, *uid).await?);
    }
    for (raw, flags, date) in &held {
        let flags = carried(flags);
        let refs: Vec<&str> = flags.iter().map(String::as_str).collect();
        dst.append(dst_folder, raw, &refs, *date).await?; // an error names the target account
    }
    if mode == TransferMode::Move {
        src.delete(src_folder, uids).await?; // an error names the source account; the copy exists
    }
    Ok(())
}
