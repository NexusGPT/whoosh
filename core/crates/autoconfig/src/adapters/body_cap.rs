//! The size limit on a provider answer: an autoconfig or Autodiscover body is read up to AUTOCONFIG_BODY_MAX_BYTES and no further.
//! Not behind `live`: the reader takes any `Read`, so a test feeds it a local socket. (Whole-body reads were uncapped: the comparison
//! with rusted-mail, dossier-rusted-mail-gaps.md row 2.)
use std::io::Read;

use crate::consts::AUTOCONFIG_BODY_MAX_BYTES;
use crate::ports::NetError;

/// The refusal in words, for an answer larger than `limit` bytes.
pub fn too_large_words(limit: u64) -> String {
    format!("The server's answer is larger than {} KiB, so it was not read. A mail settings file is far smaller than that: use Manual setup.", limit / 1024)
}

/// Reads at most `limit` bytes. One more byte than the limit is asked of the reader (never more), so an answer of exactly `limit`
/// bytes passes and one byte over is refused; memory stays bounded by `limit + 1` whatever the server sends.
pub fn read_capped(reader: impl Read, limit: u64) -> Result<Vec<u8>, NetError> {
    let mut out = Vec::new();
    reader.take(limit.saturating_add(1)).read_to_end(&mut out).map_err(|e| NetError::Other(format!("the answer could not be read: {e}")))?;
    if out.len() as u64 > limit { Err(NetError::Other(too_large_words(limit))) } else { Ok(out) }
}

/// `read_capped` at the ledger's limit (AUTOCONFIG_BODY_MAX_BYTES), as text.
pub fn read_body(reader: impl Read) -> Result<String, NetError> {
    read_capped(reader, AUTOCONFIG_BODY_MAX_BYTES).map(|b| String::from_utf8_lossy(&b).into_owned())
}
