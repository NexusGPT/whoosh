//! The real Whoosh core. Slice 1 (2026-11-09): add an account for real (provider detection over real DNS and HTTPS, a real
//! IMAP and SMTP sign-in, the account in the store and its password in the Keychain). Slice 2: each password account syncs
//! by IMAP into the store (first screen, then history, then IDLE with CONDSTORE/QRESYNC) and the mail reads answer from
//! the store. Every other method answers the honest empty read or refuses in words; the list is docs/lanes/contracts.md
//! "## Real core, slice 2: what still refuses".
uniffi::setup_scaffolding!("engine");

mod consts;
pub mod imap_source;
pub mod map;
pub mod reads;
mod actions_bridge;
mod add_oauth;
// ai-settings-core: provider keys, agents, the speech engine, and `model_for` (the model a job uses)
pub mod ai_settings;
mod decision_list;
mod decision_teaching;   // fix-teachcore: the teaching block's state paths and RefitDecisionModel (hid-ai F5)
pub mod decision_preview;
mod decision_model;
// body-r1: bodies downloaded in the background (the reader never waits on the server) and the core's own log file
pub mod bodies;
mod log;
mod check_flow;
mod check_import;
mod compose_cmds;
mod draft_files;
mod message_state;
/// engine-act-r6 round 6: "HH:mm" of a time in this Mac's time, as `snooze/<id>/return_local_time` answers it (tests read it to compare).
pub use message_state::local_time as message_state_local_time;
mod recent_search;
mod file_arrivals;
mod filing_worker;
mod flow_ctx;
mod flow_write;
mod import_run;
// junk-r2: the local spam filter on live arrivals, the first-run training and the owner's lessons
pub mod junk_filter;
mod label_flow;
mod label_ai_failure;
mod check_laya; // fix-aisort round 5: the hidden check laya-sanity.py drives the shell's Laya with synthetic mails (rig only) // fix-aisort: a failed or unreadable judgement of the label AI is said once, kept waiting and asked again
mod mute;
// lane real-notes: notes, outside apps' grants, suggestions, MergeContacts, BlockDomain (docs/lanes/real-notes.md)
mod notes_cmds;
pub mod outbox_tick;
mod refile_flipped;
mod reminders;
// delta 20: the settings of AI replies and the one port (ReplyWork) the working commands plug into
pub mod replies;
// reply-engine: the working commands of AI replies (drafts, the auto pass, the try, the voice analysis, the folders of this Mac) behind ReplyWork
pub mod replies_work;
// rig-core: the hidden test copy's switches (providers on loopback, the keyring file, the new mutant names)
pub mod rig;
mod rust_core;
// fix-realcore round 10: the hidden --slice-labeltab's rig-only switch (no tab rows on the asked open, before the backfill)
mod check_tabs;
mod settings_cmds;
// fix-aicore: the reader's thread summary, made by the writing model in the background and kept on this Mac (`thread/<id>/summary`)
pub mod thread_summary;
// fix-suggest: the follow-ups offered after junk, archive, snooze and label (docs/lanes/fix-suggest.md)
mod suggest_offers;
pub mod sign_in;
pub mod sync;
mod unarchive;
mod unsubscribe;
mod vip_mark;   // vip-mark: MarkVip, the sender of a message joins or leaves the VIP list
pub mod writeback;

pub use rust_core::{Detect, Keyring, RustCore, TestTransport, CHECK_DOMAIN};
pub use sign_in::{OAuthMutant, SignInParts};
pub use sync::SyncPhase;
// delta 17: what one run of the label flow reports (`RustCore::file_stored_messages`, `RustCore::refile_flipped`)
pub use file_arrivals::{AiStep, Filed};
pub use refile_flipped::Moved;
// delta 18: the test seam of the import run
pub use import_run::ImportPace;
// engine-actions: the send path hands a draft's reply reminder to the message it sent
pub use reminders::follow_sent;
// engine-act: the wall clock the actions and the outbox run on
pub use actions_bridge::SystemClock;

use std::sync::Arc;
use contracts::{CoreError, WhooshCore};

/// Opens the real core over the app's data folder. `rig` true = the hidden test copy: its own Keychain namespace, so a
/// test run never reads or writes the owner's credentials. Refused in words when the store cannot open.
#[uniffi::export]
pub fn open_rust_core(data_dir: String, rig: bool) -> Result<Arc<dyn WhooshCore>, CoreError> {
    let core = RustCore::open_production(&data_dir, rig)?;
    Ok(Arc::new(core))
}

/// As `open_rust_core`, handing the core the shell's on-device decision model (Laya) at launch. The port is lazy on the shell's side: nothing loads
/// until the label AI asks its first question, and the shell unloads it when idle. `None` is `open_rust_core`.
#[uniffi::export]
pub fn open_rust_core_with_decision(data_dir: String, rig: bool, decision: Option<Arc<dyn contracts::DecisionPort>>) -> Result<Arc<dyn WhooshCore>, CoreError> {
    let core = RustCore::open_production(&data_dir, rig)?;
    core.set_laya_port(decision);
    core.start_laya_check(rig);
    Ok(Arc::new(core))
}
