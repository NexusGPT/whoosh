//! fix-realcore round 10 (owner, 12 Nov 23:44: "when quitting, it doesn't seem to keep the mails"; the hidden `--slice-labeltab`): the rig-only switch
//! that gives a hidden copy the owner's main106 state: filings under a built-in label with NO `message_category` row for them (an earlier build wrote
//! only `label_assignment`). `WHOOSH_CORE_CHECK_DROP_TABS=<n>` (rig copies only): on the n-th open of this data folder, before the store's seed and its
//! `backfill_builtin_tabs` run, the tab rows of every built-in label are deleted, so the check reads whether the backfill puts the filed mail back on
//! its tab. The opens are counted in `<data dir>/check-opens` (a number, written by this file only). What was dropped is served at
//! `rig/builtin_tabs_dropped` (a count; null when nothing was dropped). Nothing happens on the owner's copy.
use std::path::Path;
use std::sync::atomic::{AtomicI64, Ordering};

use store::Store;

const VARIABLE: &str = "WHOOSH_CORE_CHECK_DROP_TABS";
const OPENS_FILE: &str = "check-opens";
/// The state path the check reads.
pub const STATE_PATH: &str = "rig/builtin_tabs_dropped";

/// -1: nothing dropped in this process.
static DROPPED: AtomicI64 = AtomicI64::new(-1);

/// Counts this open of `data_dir` and, on the asked one, deletes the built-in labels' tab rows. Rig only; any failure is a log line on stderr.
pub(crate) fn on_open(rig: bool, data_dir: &Path, store: &Store) {
    if !rig { return; }
    let Some(want) = std::env::var(VARIABLE).ok().and_then(|v| v.trim().parse::<u64>().ok()) else { return };
    let file = data_dir.join(OPENS_FILE);
    let opens = std::fs::read_to_string(&file).ok().and_then(|t| t.trim().parse::<u64>().ok()).unwrap_or(0) + 1;
    if let Err(e) = std::fs::write(&file, opens.to_string()) { eprintln!("check_tabs: {OPENS_FILE} was not written: {e}"); }
    if opens != want { return; }
    let dropped = store.write(|ctx| {
        Ok(ctx.tx.execute("DELETE FROM message_category WHERE tab_id IN (SELECT id FROM label WHERE kind = 'builtin')", [])
            .map_err(|e| store::StoreError::Invalid(e.to_string()))?)
    });
    match dropped {
        Ok(n) => { DROPPED.store(n as i64, Ordering::SeqCst); eprintln!("check_tabs: open {opens}: {n} built-in tab rows dropped before the backfill"); }
        Err(e) => eprintln!("check_tabs: the tab rows were not dropped: {e}"),
    }
}

/// `rig/builtin_tabs_dropped`: the rows dropped at this process's open, or null.
pub(crate) fn state(path: &str) -> Option<String> {
    (path == STATE_PATH).then(|| match DROPPED.load(Ordering::SeqCst) { n if n >= 0 => n.to_string(), _ => "null".to_string() })
}
