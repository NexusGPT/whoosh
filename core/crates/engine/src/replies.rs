//! Delta 20 in the real core: the SETTINGS of AI replies. Every command that only changes a setting runs on the store (`store/src/replies.rs`, one
//! transaction each) and every state path `reply/...` answers from it through the contract's encoders (`reply_ai_state.rs`), so this core and the
//! Swift fixture core cannot spell a key differently. The commands that DO work (a try, a voice analysis, a draft on demand, a change request,
//! accept, discard) are checked here with the contract's pure rules (so their refusals are the same whatever does the work) and then handed to
//! ONE port, `ReplyWork`. Its default refuses with the not-built-yet code; the lanes reply-retrieval, reply-voice and the draft engine plug in
//! with `install_reply_work`.
//!
//! Paths: reply/use/<auto|on_demand> · reply/filter · reply/on_demand · reply/counts · reply/sources · reply/calendars/<enabled|events> · reply/progress · reply/retrieval · reply/voice ·
//! reply/prompt/<current|versions|variables|preview> · reply/notes · reply/draft/<thread id> (the live draft) · reply/try/<run id> (from the work).
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use contracts::*;
use store::Store;

use crate::rust_core::now_ms;
use crate::sign_in::codes;

const OK: CommandOutcome = CommandOutcome { undo_entry_id: None, created_id: None };
const MS_PER_DAY: i64 = 86_400_000; // a unit, not a product value

/// One piece of working AI replies work, already checked by the contract's rules (a trimmed instruction, a change request with words, an open draft).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyJob {
    /// "Essayer sur un message": retrieval and the three steps on this message; nothing is stored but the report (`created_id` = the run id).
    TryReply { message_id: String, kind: ReplyUseKind },
    /// "Analyser mes messages envoyés" over the last `sample_size` sent messages (1 to REPLY_SAMPLE_CEILING).
    RunVoiceAnalysis { sample_size: u32 },
    /// ⌘J: a draft for the thread's newest message. `instruction` is already trimmed (None = none); the old live draft must be marked Discarded in the
    /// same transaction as the insert (`reply_draft_replaced`). `created_id` = the draft's id.
    DraftOnDemand { thread_id: String, instruction: Option<String> },
    /// The field under a draft: `text` is the trimmed request, `draft` the open draft it applies to (`reply_draft_after_change`).
    RequestChange { draft: ReplyDraft, text: String },
    /// Keep: `draft` is open and not yet taken into the composer. `created_id` = the ordinary composer draft (the SaveDraft path).
    Accept { draft: ReplyDraft },
    /// Throw away: `draft` is open.
    Discard { draft: ReplyDraft },
}

/// The one port where the working commands plug in. Every method has a default that says "not built yet", so a lane implements only its own jobs.
pub trait ReplyWork: Send + Sync {
    /// Does one job; the default refuses with the contract's not-built-yet code (`engine.not_built_yet`).
    fn execute(&self, _store: &Store, _job: ReplyJob) -> Result<CommandOutcome, CoreError> { Err(not_built_yet()) }
    /// Where the voice analysis stands (`reply/voice` `analysis`); the default says nothing runs.
    fn voice_analysis(&self) -> ReplyVoiceAnalysis { ReplyVoiceAnalysis::Idle }
    /// The report of a try (`reply/try/<run id>`); the default has none.
    fn try_report(&self, _run_id: &str) -> Option<ReplyTryReport> { None }
}

/// The work nothing has plugged in yet: every default of the trait.
pub struct NotBuiltYet;
impl ReplyWork for NotBuiltYet {}

fn not_built_yet() -> CoreError { codes::refusal("Writing replies with AI is not built into the local engine yet.", codes::NOT_BUILT_YET, "") }

/// The work of each core, by the path of its store (one RustCore owns one store). A later lane installs its work once, after it opens the core.
/// (A field of `RustCore` would be tidier: that is a change to the struct and its constructors, named in docs/lanes/replies-settings.md.)
fn registry() -> &'static Mutex<HashMap<PathBuf, Arc<dyn ReplyWork>>> {
    static WORK: OnceLock<Mutex<HashMap<PathBuf, Arc<dyn ReplyWork>>>> = OnceLock::new();
    WORK.get_or_init(Default::default)
}

/// Plugs the working commands of the core over the store at `store_path` in. A second install replaces the first.
pub fn install_reply_work(store_path: &std::path::Path, work: Arc<dyn ReplyWork>) {
    registry().lock().unwrap_or_else(|e| e.into_inner()).insert(store_path.to_path_buf(), work);
}

fn work_for(store: &Store) -> Arc<dyn ReplyWork> {
    registry().lock().unwrap_or_else(|e| e.into_inner()).get(store.path()).cloned().unwrap_or_else(|| Arc::new(NotBuiltYet))
}

/// Whether the command is one of delta 20's thirty (what `RustCore::run` hands here).
pub(crate) fn is_reply_command(c: &Command) -> bool {
    matches!(c, Command::SetReplyUseEnabled { .. } | Command::SetReplyUseTone { .. } | Command::SetReplyUseMatchThread { .. } | Command::SetReplyUsePrompt { .. }
        | Command::SetReplyFilterScope { .. } | Command::SetReplyFilterCard { .. } | Command::SetReplyFilterEntryRule { .. } | Command::SetReplyFilterExitRule { .. }
        | Command::SetReplyFilterAiReads { .. } | Command::SetReplyFilterAiDescription { .. } | Command::SetOnDemandOption { .. } | Command::SetRetrievalSetting { .. }
        | Command::SetReplySourceEnabled { .. } | Command::AddReplyLocalFolder { .. } | Command::RemoveReplyLocalFolder { .. } | Command::RunVoiceAnalysis { .. }
        | Command::SetVoiceText { .. } | Command::SetVoiceSamePersonExamples { .. } | Command::SaveReplyTone { .. } | Command::DeleteReplyTone { .. }
        | Command::SaveReplyPromptVersion { .. } | Command::RestoreReplyPromptVersion { .. } | Command::SaveFixedNote { .. } | Command::DeleteFixedNote { .. }
        | Command::DraftReplyOnDemand { .. } | Command::RequestReplyChange { .. } | Command::AcceptReplyDraft { .. } | Command::DiscardReplyDraft { .. }
        | Command::TryReply { .. } | Command::SyncReplyCalendars { .. })
}

fn created(id: impl Into<String>) -> CommandOutcome { CommandOutcome { undo_entry_id: None, created_id: Some(id.into()) } }

/// Whether a folder path (`/...` or `~/...`, already read by `check_local_folder_path`) is a folder on this disk.
fn folder_on_disk(path: &str) -> bool {
    match path.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME").map(|h| PathBuf::from(h).join(rest).is_dir()).unwrap_or(false),
        None => PathBuf::from(path).is_dir(),
    }
}

fn draft_by_id(store: &Store, id: &str) -> Result<ReplyDraft, CoreError> {
    store.reply_draft(id)?.ok_or_else(|| CoreError::NotFound { kind: "reply draft".into(), id: id.to_string() })
}

/// Runs one of the twenty-nine. A refusal says why in the contract's words and code; a store that fails answers `Unexpected` with its real message.
pub(crate) fn run(store: &Store, command: Command) -> Result<CommandOutcome, CoreError> {
    let now = now_ms();
    match command {
        Command::SetReplyUseEnabled { kind, enabled } => store.reply_set_use_enabled(kind, enabled, now).map(|_| OK),
        Command::SetReplyUseTone { kind, tone_id } => store.reply_set_use_tone(kind, tone_id, now).map(|_| OK),
        Command::SetReplyUseMatchThread { kind, on } => store.reply_set_use_match_thread(kind, on, now).map(|_| OK),
        Command::SetReplyUsePrompt { kind, prompt_id } => store.reply_set_use_prompt(kind, prompt_id, now).map(|_| OK),
        Command::SetReplyFilterScope { scope } => store.reply_set_filter_scope(scope, now).map(|_| OK),
        Command::SetReplyFilterCard { card, enabled } => store.reply_set_filter_card(card, enabled, now).map(|_| OK),
        Command::SetReplyFilterEntryRule { rule } => store.reply_set_filter_rule(FlowCard::Entry, rule, now).map(|_| OK),
        Command::SetReplyFilterExitRule { rule } => store.reply_set_filter_rule(FlowCard::Exit, rule, now).map(|_| OK),
        Command::SetReplyFilterAiReads { reads } => store.reply_set_filter_ai_reads(reads, now).map(|_| OK),
        Command::SetReplyFilterAiDescription { description } => store.reply_set_filter_ai_description(description, now).map(|_| OK),
        Command::SetOnDemandOption { option, enabled } => store.reply_set_on_demand(option, enabled, now).map(|_| OK),
        Command::SetRetrievalSetting { change } => store.reply_apply_retrieval_change(change, now).map(|_| OK),
        Command::SetReplySourceEnabled { source, enabled } => store.reply_set_source_enabled(source, enabled, now).map(|_| OK),
        Command::AddReplyLocalFolder { path } => {
            let path = check_local_folder_path(path)?;
            if !folder_on_disk(&path) { return Err(CoreError::Invalid { field: REPLY_BAD_FOLDER_CODE.into(), reason_words: "that folder does not exist on this Mac".into() }); }
            store.reply_add_local_folder(path, now).map(created)
        }
        // delta 21: the shell's calendars and events; the contract's `apply_calendar_sync` decides what is kept
        Command::SyncReplyCalendars { calendars, window_start_ms, window_end_ms, events } => store.reply_sync_calendars(calendars, window_start_ms, window_end_ms, events, now).map(|_| OK),
        Command::RemoveReplyLocalFolder { path } => store.reply_remove_local_folder(path).map(|_| OK),
        Command::SetVoiceText { text } => store.reply_set_voice_text(text, now).map(|_| OK),
        Command::SetVoiceSamePersonExamples { count } => store.reply_set_same_person_examples(count, now).map(|_| OK),
        Command::SaveReplyTone { tone } => store.reply_save_tone(tone).map(created),
        Command::DeleteReplyTone { tone_id } => store.reply_delete_tone(tone_id).map(|_| OK),
        Command::SaveReplyPromptVersion { prompt_id, text } => store.reply_save_prompt_version(prompt_id, text, now).map(|v| created(v.to_string())),
        Command::RestoreReplyPromptVersion { prompt_id, version } => store.reply_restore_prompt_version(prompt_id, version, now).map(|v| created(v.to_string())),
        Command::SaveFixedNote { note } => store.reply_save_fixed_note(note).map(created),
        Command::DeleteFixedNote { note_id } => store.reply_delete_fixed_note(note_id).map(|_| OK),
        // the working commands: the contract's rules first, then the one port
        Command::TryReply { message_id, kind } => {
            if store.message(&message_id).map_err(CoreError::from)?.is_none() { return Err(CoreError::NotFound { kind: "message".into(), id: message_id }); }
            work_for(store).execute(store, ReplyJob::TryReply { message_id, kind })
        }
        Command::RunVoiceAnalysis { sample_size } => {
            let sample_size = check_voice_sample_size(sample_size)?;
            work_for(store).execute(store, ReplyJob::RunVoiceAnalysis { sample_size })
        }
        Command::DraftReplyOnDemand { thread_id, instruction } => {
            let instruction = check_on_demand_instruction(store.reply_on_demand()?, instruction)?;
            work_for(store).execute(store, ReplyJob::DraftOnDemand { thread_id, instruction })
        }
        Command::RequestReplyChange { draft_id, text } => {
            let draft = draft_by_id(store, &draft_id)?;
            let text = check_change_request(store.reply_on_demand()?, draft.clone(), text)?;
            work_for(store).execute(store, ReplyJob::RequestChange { draft, text })
        }
        Command::AcceptReplyDraft { draft_id } => {
            let draft = draft_by_id(store, &draft_id)?;
            reply_draft_after_accept(draft.clone(), now)?;
            work_for(store).execute(store, ReplyJob::Accept { draft })
        }
        Command::DiscardReplyDraft { draft_id } => {
            let draft = draft_by_id(store, &draft_id)?;
            check_reply_draft_open(draft.clone())?;
            work_for(store).execute(store, ReplyJob::Discard { draft })
        }
        _ => Err(CoreError::Invalid { field: "command".into(), reason_words: "not an AI replies command".into() }),
    }
}

// ---------------------------------------------------------------- sources of a new installation

/// The leaf names of a Sent folder that sync-imap's role table does not know (lower case; its English names are already roles). PROPOSAL: move them into
/// `sync-imap/src/roles.rs`, which would also give the validation report and the Sent actions the same answer.
const SENT_NAMES: &[&str] = &["envoyés", "éléments envoyés", "messages envoyés", "gesendet", "gesendete objekte", "gesendete elemente", "verzonden", "verzonden items", "enviados", "elementos enviados", "inviati", "posta inviata"];

fn named_sent(name: &str) -> bool { SENT_NAMES.contains(&name.rsplit(['/', '.']).next().unwrap_or(name).to_lowercase().as_str()) }

/// The owner's defaults for the folders of an account (docs/lanes/replies-settings.md round 4): "Sources start with Envoyés and Boîte de réception of each
/// account." Called by the sync at every LIST of the account (`imap_source.rs`, the one granted line): a folder new to the store gets its row, ON when its
/// role is Inbox or Sent (read from the server's SPECIAL-USE flag, so "Éléments envoyés" and "Sent Items" are Sent), OFF for every other folder; a folder the
/// store already knows is never touched; a folder the server no longer lists is switched off and kept (`reply_register_folders`). The folders the sync skips (All Mail, Starred) are not sources. A failure is logged, never raised: the sync goes on.
pub(crate) fn register_folders(store: &Store, account_id: &str, roles: &HashMap<String, sync_imap::FolderRole>) {
    use sync_imap::FolderRole::{Inbox, Other, Sent};
    // a server that sends no SPECIAL-USE flag and names its Sent folder in the owner's language ("Gesendet"): sync-imap's name table knows the English names
    // only, so the folders it left without a role are read against `SENT_NAMES`, and only when the account has no folder with the Sent role
    let has_sent = roles.values().any(|r| *r == Sent);
    let mut folders: Vec<(String, bool)> = roles.iter().filter(|(_, role)| !crate::imap_source::skipped(**role))
        .map(|(name, role)| (name.clone(), matches!(role, Inbox | Sent) || (!has_sent && *role == Other && named_sent(name)))).collect();
    folders.sort();
    if let Err(e) = store.reply_register_folders(account_id.to_string(), folders, now_ms()) { eprintln!("replies: the folders of {account_id} were not registered as sources: {e}"); }
}

// ---------------------------------------------------------------- state

fn json_string(v: &str) -> String {
    let mut o = String::from("\"");
    for c in v.chars() {
        match c {
            '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), '\n' => o.push_str("\\n"), '\r' => o.push_str("\\r"), '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// The Sources pane: the mail folders grouped into one card per account, the folders of this Mac, the calendars the shell listed (with their real name,
/// colour and account label, delta 21; a calendar registered before the shell listed it shows its id).
fn sources_json(store: &Store) -> Result<String, CoreError> {
    let rows = live_mail_sources(store, store.reply_sources()?)?;
    let mut accounts: Vec<String> = vec![];
    let mut local = vec![];
    for s in &rows {
        match &s.source {
            ReplySourceRef::MailFolder { account_id, .. } => if !accounts.contains(account_id) { accounts.push(account_id.clone()) },
            ReplySourceRef::LocalFolder { .. } => local.push(s.clone()),
            ReplySourceRef::Calendar { .. } => {}
        }
    }
    let cards = accounts.into_iter().map(|a| reply_account_sources(a, rows.clone())).collect();
    Ok(reply_sources_json_with_choices(reply_sources_read(cards, local, store.reply_calendars()?, store.reply_retrieval()?.scope), store.reply_owner_choices()?))   // delta 24: owner_enabled of a gone folder
}

/// What a mail folder's tile says of its sync, derived, never a stale stored word: a switched-off folder is "not started" and stays so; an enabled folder
/// whose first sync is complete and holds no message is UP TO DATE (nothing to index: the contract's `folder_sync_progress` reads 0 messages as "syncing 0 %",
/// which would last for ever); every other enabled folder is "syncing" with the share of its messages indexed, from the moment it is switched on (its walk is
/// queued then: no "not started" for an enabled folder). The contract's function still gives the figure and the UpToDate of a folder with messages.
pub(crate) fn mail_folder_progress(p: &FolderProgress, enabled: bool, first_sync_done: bool) -> ReplySyncProgress {
    if !enabled { return ReplySyncProgress::NotStarted; }
    if p.messages == 0 && first_sync_done { return ReplySyncProgress::UpToDate; }
    folder_sync_progress(p.clone(), true)
}

/// The tiles of the mail folders with their live PROGRESS (from the stored messages and the sync's own record); a sync that FAILED keeps its words. The item
/// count stays the stored one (the crawl's report writes it; a test that holds the background keeps what it seeded). Folders of this Mac and calendars are left as stored.
fn live_mail_sources(store: &Store, rows: Vec<ReplySource>) -> Result<Vec<ReplySource>, CoreError> {
    let figures = store.reply_folder_progress()?;
    let synced = store.reply_folder_first_sync()?;
    Ok(rows.into_iter().map(|s| {
        let ReplySourceRef::MailFolder { account_id, folder } = &s.source else { return s };
        let p = figures.iter().find(|p| &p.account_id == account_id && &p.folder == folder).cloned()
            .unwrap_or(FolderProgress { account_id: account_id.clone(), folder: folder.clone(), messages: 0, bodies: 0, indexed: 0 });
        let progress = match (&s.progress, s.enabled) {
            (ReplySyncProgress::Failed { .. }, true) => s.progress.clone(),
            _ => mail_folder_progress(&p, s.enabled, synced.contains(&(account_id.clone(), folder.clone()))),
        };
        ReplySource { progress, ..s }
    }).collect())
}

/// What the model would receive for the newest message: the variables replaced, the note lines dropped. A variable with nothing to say (the instruction,
/// the excerpts retrieval finds at draft time, the calendar) is replaced by nothing.
fn preview(store: &Store) -> Result<Option<String>, CoreError> {
    let Some(current) = store.reply_prompt_versions(REPLY_PROMPT_ID)?.into_iter().find(|p| p.current) else { return Ok(None) };
    let message = store.reply_newest_message()?;
    let tone_id = store.reply_use(ReplyUseKind::Auto)?.tone_id;
    let tone = if tone_id == REPLY_TONE_VOICE_ID { store.reply_voice()?.tone_text } else { store.reply_tones()?.into_iter().find(|t| t.id == tone_id).map(|t| t.text).unwrap_or_default() };
    let notes = store.reply_fixed_notes()?.iter().map(|n| format!("{} : {}", n.title, n.text)).collect::<Vec<_>>().join("\n");
    let value = |id: &str, v: String| PromptValue { id: id.to_string(), value: v };
    let values = vec![
        value("message", message.as_ref().map(|(from, subject, preview)| format!("De {from} : {subject}\n{preview}")).unwrap_or_default()),
        value("thread", message.map(|(_, _, preview)| preview).unwrap_or_default()),
        value("instruction", String::new()), value("excerpts", String::new()), value("calendar", String::new()), value("tone", tone), value("notes", notes),
    ];
    Ok(Some(json_string(&render_reply_prompt(current.text, values))))
}

fn answer(store: &Store, p: &[&str]) -> Result<Option<String>, CoreError> {
    Ok(match p {
        ["reply", "use", k] => match reply_use_kind_from_key(k.to_string()) { Some(kind) => Some(reply_use_json(store.reply_use(kind)?)), None => None },
        ["reply", "filter"] => Some(reply_filter_json(store.reply_filter()?)),
        ["reply", "on_demand"] => Some(on_demand_json(store.reply_on_demand()?)),
        ["reply", "counts"] => {
            let now = now_ms();
            let days = reply_counts_days();
            Some(reply_counts_json(count_reply_steps(store.reply_passes_since(now - i64::from(days) * MS_PER_DAY)?, now, days)))
        }
        ["reply", "sources"] => Some(sources_json(store)?),
        // delta 21: what the shell reads before it sends events, the events kept, and the crawl's progress per folder
        ["reply", "calendars", "enabled"] => Some(enabled_calendar_ids_json(store.reply_calendars()?)),
        ["reply", "calendars", "events"] => Some(calendar_events_json(store.reply_calendar_events()?)),
        ["reply", "progress"] => Some(folder_progress_json(store.reply_folder_progress()?)),
        ["reply", "retrieval"] => Some(retrieval_json(store.reply_retrieval()?)),
        ["reply", "voice"] => Some(reply_voice_json(store.reply_voice()?, work_for(store).voice_analysis(), store.reply_tones()?)),
        ["reply", "prompt", "current"] => store.reply_prompt_versions(REPLY_PROMPT_ID)?.into_iter().find(|v| v.current).map(reply_prompt_json),
        ["reply", "prompt", "versions"] => Some(reply_prompt_versions_json(store.reply_prompt_versions(REPLY_PROMPT_ID)?)),
        ["reply", "prompt", "variables"] => Some(reply_prompt_variables_json(reply_prompt_variables())),
        ["reply", "prompt", "preview"] => preview(store)?,
        ["reply", "notes"] => Some(fixed_notes_json(store.reply_fixed_notes()?)),
        ["reply", "draft", thread] => store.reply_draft_live(thread)?.map(reply_draft_json),
        ["reply", "try", run] => work_for(store).try_report(run).map(reply_try_json),
        _ => None,
    })
}

/// The answer of one `reply/...` path; None = not one of them, or nothing there (no live draft, no such run). A store that fails is logged with its
/// real message and answers None: a read never waits on, or throws into, the screen.
pub(crate) fn state(store: &Store, path: &str) -> Option<String> {
    let p: Vec<&str> = path.split('/').collect();
    if p.first() != Some(&"reply") { return None; }
    match answer(store, &p) {
        Ok(v) => v,
        Err(e) => { eprintln!("replies: the read {path} failed: {e}"); None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sync must never depend on reply sources: a registration that fails (here: an account the store does not hold) is logged and returns.
    #[test]
    fn a_registration_that_fails_never_fails_the_listing() {
        let dir = std::env::temp_dir().join(format!("whoosh-replies-register-fail-{}-{}", std::process::id(), now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(&dir.join("whoosh.db")).unwrap();
        let roles = HashMap::from([("INBOX".to_string(), sync_imap::FolderRole::Inbox)]);
        register_folders(&store, "no-such-account", &roles);
        assert!(store.reply_sources().unwrap().is_empty(), "nothing was stored for an account that does not exist");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
