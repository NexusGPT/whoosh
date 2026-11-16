//! Delta 20 (AI replies) over migration 0013: typed reads and writes for the eleven tables, ONE transaction per command. The checks are the
//! contract's pure functions (`reply_ai_logic.rs`); this file stores what they allow and says no in the contract's words (`CoreError::Invalid`
//! with the contract's code, `CoreError::NotFound`). A refused write undoes everything it did (the refusal travels out of the closure as an `Err`,
//! so the transaction is dropped, never committed half done). The state JSON is the contract's (`reply_ai_state.rs`), written by the engine.
//!
//! Not here: the chunk table and its vectors (the indexer lane), a draft's insert and its moves (the draft engine, behind the engine's `ReplyWork`),
//! the sync that fills a source's count and progress (the sources lane writes it through `reply_source_progress`).
use crate::error::StoreError;
use crate::label_meta::{decode_conditions, encode_conditions};
use crate::labels::new_id;
use crate::Store;
use contracts::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::{Arc, Mutex};

type R<T> = Result<T, CoreError>;

fn db(e: rusqlite::Error) -> CoreError { StoreError::from(e).into() }
fn from_store(e: StoreError) -> CoreError { e.into() }
fn broken(words: String) -> CoreError { CoreError::Unexpected { reason_words: words } }
fn flag(b: bool) -> i64 { i64::from(b) }
fn not_found(kind: &str, id: impl Into<String>) -> CoreError { CoreError::NotFound { kind: kind.into(), id: id.into() } }

// ---------------------------------------------------------------- row readers (shared by the reads and by the checks inside a write)

fn use_in(c: &Connection, kind: ReplyUseKind) -> R<ReplyUse> {
    c.query_row("SELECT enabled, tone_id, match_thread, prompt_id FROM reply_use WHERE kind = ?1", [reply_use_kind_key(kind)],
        |r| Ok(ReplyUse { kind, enabled: r.get::<_, i64>(0)? == 1, tone_id: r.get(1)?, match_thread: r.get::<_, i64>(2)? == 1, prompt_id: r.get(3)? })).map_err(db)
}

fn rule_in(c: &Connection, mode: &str, json: &str) -> R<RuleSet> {
    let match_mode = match_mode_from_key(mode.to_string()).ok_or_else(|| broken(format!("a stored reply rule has the match mode {mode:?}")))?;
    Ok(RuleSet { conditions: decode_conditions(c, json).map_err(from_store)?, match_mode })
}

fn filter_in(c: &Connection) -> R<ReplyFilter> {
    type Row = (i64, i64, String, String, i64, i64, i64, i64, i64, String, i64, String, String);
    let row: Row = c.query_row(
        "SELECT scope_all, entry_enabled, entry_match, entry_conditions_json, ai_enabled, ai_reads_subject, ai_reads_body, ai_reads_thread, ai_reads_attachments, \
         ai_description, exit_enabled, exit_match, exit_conditions_json FROM reply_filter WHERE id = 1",
        [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?, r.get(12)?))).map_err(db)?;
    let (scope_all, entry_enabled, entry_match, entry_json, ai_enabled, subject, body, thread, attachments, description, exit_enabled, exit_match, exit_json) = row;
    let scope = if scope_all == 1 { ReplyLabelScope::All } else {
        let mut s = c.prepare_cached("SELECT label_id FROM reply_filter_label ORDER BY label_id").map_err(db)?;
        let ids = s.query_map([], |r| r.get::<_, String>(0)).map_err(db)?.collect::<Result<Vec<_>, _>>().map_err(db)?;
        ReplyLabelScope::Labels { label_ids: ids }
    };
    Ok(ReplyFilter {
        scope, entry_enabled: entry_enabled == 1, entry_rule: rule_in(c, &entry_match, &entry_json)?,
        ai_enabled: ai_enabled == 1,
        ai_reads: AiReads { subject: subject == 1, body: body == 1, rest_of_thread: thread == 1, attachments: attachments == 1 },
        ai_description: description, exit_enabled: exit_enabled == 1, exit_rule: rule_in(c, &exit_match, &exit_json)?,
    })
}

fn on_demand_in(c: &Connection) -> R<OnDemandOptions> {
    c.query_row("SELECT instruction_before, change_after FROM reply_on_demand WHERE id = 1", [],
        |r| Ok(OnDemandOptions { instruction_before: r.get::<_, i64>(0)? == 1, change_after: r.get::<_, i64>(1)? == 1 })).map_err(db)
}

fn retrieval_in(c: &Connection) -> R<RetrievalSettings> {
    type Row = (String, u32, u32, String, i64, i64, i64, i64, u32, String, Option<String>, String);
    let row: Row = c.query_row(
        "SELECT retrieval_prompt, questions_max, excerpts_per_question, excerpt_size, attach_date, attach_sender, attach_folder, attach_subject, keep_n, clash_rule, \
         ai_filter_question, scope FROM retrieval_settings WHERE id = 1",
        [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?, r.get(11)?))).map_err(db)?;
    let (retrieval_prompt, questions_max, excerpts_per_question, size, date, sender, folder, subject, keep_n, clash, ai_filter_question, scope) = row;
    Ok(RetrievalSettings {
        retrieval_prompt, questions_max, excerpts_per_question,
        excerpt_size: excerpt_size_from_key(size.clone()).ok_or_else(|| broken(format!("a stored excerpt size {size:?}")))?,
        attached_fields: ExcerptFields { date: date == 1, sender: sender == 1, folder: folder == 1, subject: subject == 1 },
        keep_n, clash_rule: clash_rule_from_key(clash.clone()).ok_or_else(|| broken(format!("a stored clash rule {clash:?}")))?,
        ai_filter_question, scope: retrieval_scope_from_key(scope.clone()).ok_or_else(|| broken(format!("a stored retrieval scope {scope:?}")))?,
    })
}

fn voice_in(c: &Connection) -> R<ReplyVoice> {
    c.query_row("SELECT tone_text, sample_size, analysed_at_ms, same_person_examples FROM reply_voice WHERE id = 1", [],
        |r| Ok(ReplyVoice { tone_text: r.get(0)?, sample_size: r.get(1)?, analysed_at_ms: r.get(2)?, same_person_examples: r.get(3)? })).map_err(db)
}

fn tones_in(c: &Connection) -> R<Vec<ReplyTone>> {
    let mut s = c.prepare_cached("SELECT id, name, text FROM reply_tone ORDER BY sort_order, rowid").map_err(db)?;
    let rows = s.query_map([], |r| Ok(ReplyTone { id: r.get(0)?, name: r.get(1)?, text: r.get(2)? })).map_err(db)?.collect::<Result<Vec<_>, _>>().map_err(db)?;
    Ok(rows)
}

fn notes_in(c: &Connection) -> R<Vec<FixedNote>> {
    let mut s = c.prepare_cached("SELECT id, title, text FROM fixed_note ORDER BY sort_order, rowid").map_err(db)?;
    let rows = s.query_map([], |r| Ok(FixedNote { id: r.get(0)?, title: r.get(1)?, text: r.get(2)? })).map_err(db)?.collect::<Result<Vec<_>, _>>().map_err(db)?;
    Ok(rows)
}

fn prompts_in(c: &Connection, prompt_id: &str) -> R<Vec<ReplyPrompt>> {
    let mut s = c.prepare_cached("SELECT prompt_id, version, text, created_at_ms, current FROM reply_prompt WHERE prompt_id = ?1 ORDER BY version").map_err(db)?;
    let rows = s.query_map([prompt_id], |r| Ok(ReplyPrompt { prompt_id: r.get(0)?, version: r.get(1)?, text: r.get(2)?, created_at_ms: r.get(3)?, current: r.get::<_, i64>(4)? == 1 }))
        .map_err(db)?.collect::<Result<Vec<_>, _>>().map_err(db)?;
    Ok(rows)
}

fn progress_of(state: &str, percent: Option<i64>, words: Option<String>) -> R<ReplySyncProgress> {
    Ok(match state {
        "not_started" => ReplySyncProgress::NotStarted,
        "syncing" => ReplySyncProgress::Syncing { percent: percent.unwrap_or(0).clamp(0, 100) as u8 },
        "up_to_date" => ReplySyncProgress::UpToDate,
        "failed" => ReplySyncProgress::Failed { reason_words: words.unwrap_or_default() },
        other => return Err(broken(format!("a stored source has the progress {other:?}"))),
    })
}

fn sources_in(c: &Connection) -> R<Vec<ReplySource>> {
    let mut s = c.prepare_cached("SELECT source_key, enabled, item_count, progress_state, progress_percent, progress_words, gone FROM reply_source ORDER BY source_key").map_err(db)?;
    let raw = s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, String>(3)?, r.get::<_, Option<i64>>(4)?, r.get::<_, Option<String>>(5)?, r.get::<_, i64>(6)?)))
        .map_err(db)?.collect::<Result<Vec<_>, _>>().map_err(db)?;
    raw.into_iter().map(|(key, enabled, count, state, percent, words, gone)| {
        let source = reply_source_from_key(key.clone()).ok_or_else(|| broken(format!("a stored source key does not name a source: {key:?}")))?;
        let row = ReplySource { source, enabled: enabled == 1, item_count: count.max(0) as u64, progress: progress_of(&state, percent, words)? };
        Ok(if gone == 1 { reply_source_with_presence(row, SourcePresence { enabled: false, owner_enabled: None, gone: true }) } else { row })   // delta 24: a gone row reads off and Gone, its last count kept
    }).collect()
}

fn step_of(word: &str) -> R<ReplyStepResult> { reply_step_result_from_key(word.to_string()).ok_or_else(|| broken(format!("a stored step result {word:?}"))) }

fn draft_in(c: &Connection, sql: &str, key: &str) -> R<Option<ReplyDraft>> {
    type Row = (String, String, String, String, String, String, Option<String>, String, u32, i64, i64);
    let row: Option<Row> = c.query_row(sql, [key], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?)))
        .optional().map_err(db)?;
    let Some((id, thread_id, message_id, text, made_by, excerpts_json, instruction, state, revision, created, updated)) = row else { return Ok(None) };
    let mut s = c.prepare("SELECT value FROM json_each(?1) ORDER BY key").map_err(db)?;
    let excerpt_ids = s.query_map([excerpts_json], |r| r.get::<_, String>(0)).map_err(db)?.collect::<Result<Vec<_>, _>>().map_err(db)?;
    Ok(Some(ReplyDraft {
        id, thread_id, message_id, text,
        made_by: reply_made_by_from_key(made_by.clone()).ok_or_else(|| broken(format!("a stored draft made by {made_by:?}")))?, excerpt_ids, instruction,
        state: reply_draft_state_from_key(state.clone()).ok_or_else(|| broken(format!("a stored draft state {state:?}")))?, revision, created_at_ms: created, updated_at_ms: updated,
    }))
}

const DRAFT_COLUMNS: &str = "SELECT id, thread_id, message_id, text, made_by, excerpt_ids_json, instruction, state, revision, created_at_ms, updated_at_ms FROM reply_draft";

fn prompt_exists(c: &Connection, prompt_id: &str) -> R<()> {
    let n: i64 = c.query_row("SELECT count(*) FROM reply_prompt WHERE prompt_id = ?1", [prompt_id], |r| r.get(0)).map_err(db)?;
    if n == 0 { Err(not_found("prompt", prompt_id)) } else { Ok(()) }
}

/// A new version of a prompt: one past the highest, now the current one (the old current is demoted first: the index allows one). Returns the number.
fn push_prompt_version(c: &Connection, prompt_id: &str, text: &str, now_ms: i64) -> R<u32> {
    let existing = prompts_in(c, prompt_id)?;
    let version = next_prompt_version(existing.iter().map(|p| p.version).collect());
    c.execute("UPDATE reply_prompt SET current = 0 WHERE prompt_id = ?1 AND current = 1", [prompt_id]).map_err(db)?;
    c.execute("INSERT INTO reply_prompt (prompt_id, version, text, created_at_ms, current) VALUES (?1, ?2, ?3, ?4, 1)", params![prompt_id, version, text, now_ms]).map_err(db)?;
    Ok(version)
}

fn next_order(c: &Connection, table: &str) -> R<i64> {
    // the table name is one of two literals of this file, never a caller's word
    c.query_row(&format!("SELECT coalesce(max(sort_order), 0) + 1 FROM {table}"), [], |r| r.get(0)).map_err(db)
}

fn json_text(v: &str) -> String {
    let mut o = String::from("\"");
    for ch in v.chars() { match ch { '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), ch if (ch as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", ch as u32)), ch => o.push(ch) } }
    o.push('"');
    o
}

fn calendars_in(c: &Connection) -> R<Vec<ReplyCalendar>> {
    let mut s = c.prepare_cached("SELECT s.source_key, s.enabled, k.name, k.account_label, k.colour FROM reply_source s \
        LEFT JOIN reply_calendar k ON s.source_key = 'calendar' || char(31) || k.calendar_id WHERE s.kind = 'calendar' ORDER BY s.source_key").map_err(db)?;
    let raw = s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?, r.get::<_, Option<String>>(4)?)))
        .map_err(db)?.collect::<Result<Vec<_>, _>>().map_err(db)?;
    raw.into_iter().map(|(key, enabled, name, label, colour)| match reply_source_from_key(key.clone()) {
        Some(ReplySourceRef::Calendar { calendar_id }) => Ok(ReplyCalendar { name: name.unwrap_or_else(|| calendar_id.clone()), calendar_id, account_label: label.unwrap_or_default(), colour: colour.unwrap_or_default(), enabled: enabled == 1 }),
        _ => Err(broken(format!("a stored calendar source key does not name a calendar: {key:?}"))),
    }).collect()
}

#[cfg(test)]
mod views_reader {
    use super::*;
    use std::time::Duration;

    /// What this proves: `reply_folder_progress` does not take the views' reader (`Store.reader`, the one connection every list, thread and count read waits
    /// on). A view mid-read is simulated by holding that lock; the call must still answer. It does not prove the scan's own duration.
    #[test]
    fn reply_progress_never_waits_for_the_reader_the_views_use() {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("whoosh-replies-reader-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Arc::new(Store::open(&dir.join("whoosh.db")).unwrap());
        let views = store.reader.lock().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let other = store.clone();
        std::thread::spawn(move || { let _ = tx.send(other.reply_folder_progress()); });
        let got = rx.recv_timeout(Duration::from_secs(5)).expect("reply_folder_progress waited for the reader the views use");
        assert!(got.unwrap().is_empty());
        drop(views);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The sync's registration of folders writes through the writer thread: with the views' reader held, it still commits.
    #[test]
    fn registering_folders_never_waits_for_the_reader_the_views_use() {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("whoosh-replies-register-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = Arc::new(Store::open(&dir.join("whoosh.db")).unwrap());
        let account = store.add_account("a@example.test", "A", contracts::MailProvider::Imap, "svc\u{1f}acct", false, contracts::AccountConfig {
            imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: contracts::ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
            smtp_security: contracts::ConnectionSecurity::Starttls, username: "a@example.test".into(), smtp_username: "a@example.test".into(),
            auth_method: contracts::AuthMethod::Password, source: contracts::DetectionSource::Manual }, 1).unwrap();
        let views = store.reader.lock().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let other = store.clone();
        std::thread::spawn(move || { let _ = tx.send(other.reply_register_folders(account, vec![("INBOX".into(), true)], 5)); });
        rx.recv_timeout(Duration::from_secs(5)).expect("registering folders waited for the reader the views use").unwrap();
        drop(views);
        assert_eq!(store.reply_sources().unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

impl Store {
    /// Runs `f` as one transaction. A refusal (`Err`) undoes every statement `f` ran.
    fn reply_write<T: Send + 'static>(&self, f: impl FnOnce(&Connection) -> R<T> + Send + 'static) -> R<T> {
        let refusal: Arc<Mutex<Option<CoreError>>> = Arc::new(Mutex::new(None));
        let slot = refusal.clone();
        let out = self.write(move |ctx| match f(ctx.tx) {
            Ok(v) => Ok(v),
            Err(e) => { *slot.lock().unwrap_or_else(|p| p.into_inner()) = Some(e); Err(StoreError::Invalid("an AI replies command was refused".into())) }
        });
        match out {
            Ok(v) => Ok(v),
            Err(e) => Err(refusal.lock().unwrap_or_else(|p| p.into_inner()).take().unwrap_or_else(|| from_store(e))),
        }
    }

    fn reply_read<T>(&self, f: impl FnOnce(&Connection) -> R<T>) -> R<T> {
        let mut refusal: Option<CoreError> = None;
        let out = self.read(|c| match f(c) { Ok(v) => Ok(Some(v)), Err(e) => { refusal = Some(e); Ok(None) } });
        match (out, refusal) { (_, Some(e)) => Err(e), (Ok(Some(v)), None) => Ok(v), (Ok(None), None) => Err(broken("a read answered nothing".into())), (Err(e), None) => Err(from_store(e)) }
    }

    // ------------------------------------------------------------ reads

    pub fn reply_use(&self, kind: ReplyUseKind) -> R<ReplyUse> { self.reply_read(|c| use_in(c, kind)) }
    pub fn reply_filter(&self) -> R<ReplyFilter> { self.reply_read(filter_in) }
    pub fn reply_on_demand(&self) -> R<OnDemandOptions> { self.reply_read(on_demand_in) }
    pub fn reply_retrieval(&self) -> R<RetrievalSettings> { self.reply_read(retrieval_in) }
    pub fn reply_voice(&self) -> R<ReplyVoice> { self.reply_read(voice_in) }
    pub fn reply_tones(&self) -> R<Vec<ReplyTone>> { self.reply_read(tones_in) }
    pub fn reply_fixed_notes(&self) -> R<Vec<FixedNote>> { self.reply_read(notes_in) }
    /// Every version of one prompt, oldest first (the encoder sorts newest first).
    pub fn reply_prompt_versions(&self, prompt_id: &str) -> R<Vec<ReplyPrompt>> { let id = prompt_id.to_string(); self.reply_read(move |c| prompts_in(c, &id)) }
    /// Every source of the three kinds, by key. The engine groups them into the cards of the Sources pane.
    pub fn reply_sources(&self) -> R<Vec<ReplySource>> { self.reply_read(sources_in) }

    /// The passes of messages received after `since_ms` (the counts' window opens there; `count_reply_steps` applies the exact bounds).
    pub fn reply_passes_since(&self, since_ms: i64) -> R<Vec<ReplyPass>> {
        self.reply_read(move |c| {
            let mut s = c.prepare_cached("SELECT message_id, received_at_ms, entry_result, ai_result, exit_result, draft_id FROM reply_pass WHERE received_at_ms > ?1 ORDER BY received_at_ms").map_err(db)?;
            let raw = s.query_map([since_ms], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, Option<String>>(5)?)))
                .map_err(db)?.collect::<Result<Vec<_>, _>>().map_err(db)?;
            raw.into_iter().map(|(message_id, received_at_ms, e, a, x, draft_id)| Ok(ReplyPass { message_id, received_at_ms, entry: step_of(&e)?, ai: step_of(&a)?, exit: step_of(&x)?, draft_id })).collect()
        })
    }

    /// The thread's live (proposed or edited) draft, if any.
    pub fn reply_draft_live(&self, thread_id: &str) -> R<Option<ReplyDraft>> {
        let id = thread_id.to_string();
        self.reply_read(move |c| draft_in(c, &format!("{DRAFT_COLUMNS} WHERE thread_id = ?1 AND state IN ('proposed','edited')"), &id))
    }

    /// One draft by id, in any state.
    pub fn reply_draft(&self, draft_id: &str) -> R<Option<ReplyDraft>> {
        let id = draft_id.to_string();
        self.reply_read(move |c| draft_in(c, &format!("{DRAFT_COLUMNS} WHERE id = ?1"), &id))
    }

    /// The newest stored message as (sender, subject, preview): what "Voir ce que le modèle reçoit" fills `{{message}}` with.
    pub fn reply_newest_message(&self) -> R<Option<(String, String, String)>> {
        self.reply_read(|c| c.query_row("SELECT CASE WHEN trim(sender_name) = '' THEN from_address ELSE sender_name END, subject, preview FROM message ORDER BY received_at_ms DESC LIMIT 1", [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional().map_err(db))
    }

    // ------------------------------------------------------------ the two uses

    pub fn reply_set_use_enabled(&self, kind: ReplyUseKind, enabled: bool, now_ms: i64) -> R<()> {
        self.reply_write(move |c| { c.execute("UPDATE reply_use SET enabled = ?2, updated_at_ms = ?3 WHERE kind = ?1", params![reply_use_kind_key(kind), flag(enabled), now_ms]).map_err(db)?; Ok(()) })
    }

    pub fn reply_set_use_match_thread(&self, kind: ReplyUseKind, on: bool, now_ms: i64) -> R<()> {
        self.reply_write(move |c| { c.execute("UPDATE reply_use SET match_thread = ?2, updated_at_ms = ?3 WHERE kind = ?1", params![reply_use_kind_key(kind), flag(on), now_ms]).map_err(db)?; Ok(()) })
    }

    /// An unknown tone is `NotFound` (`check_tone_choice`).
    pub fn reply_set_use_tone(&self, kind: ReplyUseKind, tone_id: String, now_ms: i64) -> R<()> {
        self.reply_write(move |c| {
            check_tone_choice(tone_id.clone(), tones_in(c)?)?;
            c.execute("UPDATE reply_use SET tone_id = ?2, updated_at_ms = ?3 WHERE kind = ?1", params![reply_use_kind_key(kind), tone_id, now_ms]).map_err(db)?;
            Ok(())
        })
    }

    /// An unknown prompt is `NotFound`.
    pub fn reply_set_use_prompt(&self, kind: ReplyUseKind, prompt_id: String, now_ms: i64) -> R<()> {
        self.reply_write(move |c| {
            prompt_exists(c, &prompt_id)?;
            c.execute("UPDATE reply_use SET prompt_id = ?2, updated_at_ms = ?3 WHERE kind = ?1", params![reply_use_kind_key(kind), prompt_id, now_ms]).map_err(db)?;
            Ok(())
        })
    }

    // ------------------------------------------------------------ the filter (auto use)

    /// All labels, or exactly these. An empty list, a blank id (`reply_scope_empty`) or a label that does not exist (`NotFound`) is refused.
    pub fn reply_set_filter_scope(&self, scope: ReplyLabelScope, now_ms: i64) -> R<()> {
        self.reply_write(move |c| {
            check_reply_scope(scope.clone())?;
            c.execute("DELETE FROM reply_filter_label", []).map_err(db)?;
            match &scope {
                ReplyLabelScope::All => { c.execute("UPDATE reply_filter SET scope_all = 1, updated_at_ms = ?1 WHERE id = 1", [now_ms]).map_err(db)?; }
                ReplyLabelScope::Labels { label_ids } => {
                    for id in label_ids {
                        let known: i64 = c.query_row("SELECT count(*) FROM label WHERE id = ?1", [id], |r| r.get(0)).map_err(db)?;
                        if known == 0 { return Err(not_found("label", id.clone())); }
                        c.execute("INSERT OR IGNORE INTO reply_filter_label (label_id) VALUES (?1)", [id]).map_err(db)?;
                    }
                    c.execute("UPDATE reply_filter SET scope_all = 0, updated_at_ms = ?1 WHERE id = 1", [now_ms]).map_err(db)?;
                }
            }
            Ok(())
        })
    }

    /// One of the three cards on or off. The AI card on with a blank question is refused (`reply_ai_needs_description`).
    pub fn reply_set_filter_card(&self, card: FlowCard, enabled: bool, now_ms: i64) -> R<()> {
        self.reply_write(move |c| {
            check_set_reply_card(filter_in(c)?, card, enabled)?;
            let sql = match card {
                FlowCard::Entry => "UPDATE reply_filter SET entry_enabled = ?1, updated_at_ms = ?2 WHERE id = 1",
                FlowCard::Ai => "UPDATE reply_filter SET ai_enabled = ?1, updated_at_ms = ?2 WHERE id = 1",
                FlowCard::Exit => "UPDATE reply_filter SET exit_enabled = ?1, updated_at_ms = ?2 WHERE id = 1",
            };
            c.execute(sql, params![flag(enabled), now_ms]).map_err(db)?;
            Ok(())
        })
    }

    /// Replaces the entry rule (`Entry`) or the exit rule (`Exit`) whole. An empty rule is a value (it passes everything). Refused with `bad_reply_rule`.
    pub fn reply_set_filter_rule(&self, card: FlowCard, rule: RuleSet, now_ms: i64) -> R<()> {
        self.reply_write(move |c| {
            validate_reply_rule(rule.clone())?;
            let (mode, json) = (match_mode_key(rule.match_mode), encode_conditions(&rule.conditions));
            let sql = match card {
                FlowCard::Entry => "UPDATE reply_filter SET entry_match = ?1, entry_conditions_json = ?2, updated_at_ms = ?3 WHERE id = 1",
                FlowCard::Exit => "UPDATE reply_filter SET exit_match = ?1, exit_conditions_json = ?2, updated_at_ms = ?3 WHERE id = 1",
                FlowCard::Ai => return Err(CoreError::Invalid { field: REPLY_BAD_RULE_CODE.into(), reason_words: "the AI card has no rule: it has a question".into() }),
            };
            c.execute(sql, params![mode, json, now_ms]).map_err(db)?;
            Ok(())
        })
    }

    /// The four ticks of "L'IA lit", replaced whole. All four off is refused (`reply_reads_nothing`).
    pub fn reply_set_filter_ai_reads(&self, reads: AiReads, now_ms: i64) -> R<()> {
        self.reply_write(move |c| {
            check_reply_ai_reads(reads)?;
            c.execute("UPDATE reply_filter SET ai_reads_subject = ?1, ai_reads_body = ?2, ai_reads_thread = ?3, ai_reads_attachments = ?4, updated_at_ms = ?5 WHERE id = 1",
                params![flag(reads.subject), flag(reads.body), flag(reads.rest_of_thread), flag(reads.attachments), now_ms]).map_err(db)?;
            Ok(())
        })
    }

    /// The question under the AI card, trimmed. Blank is refused while the AI card is on (`reply_ai_needs_description`).
    pub fn reply_set_filter_ai_description(&self, description: String, now_ms: i64) -> R<()> {
        self.reply_write(move |c| {
            check_reply_ai_description(filter_in(c)?.ai_enabled, description.clone())?;
            c.execute("UPDATE reply_filter SET ai_description = ?1, updated_at_ms = ?2 WHERE id = 1", params![description.trim(), now_ms]).map_err(db)?;
            Ok(())
        })
    }

    pub fn reply_set_on_demand(&self, option: OnDemandOption, enabled: bool, now_ms: i64) -> R<()> {
        self.reply_write(move |c| {
            let sql = match option {
                OnDemandOption::InstructionBefore => "UPDATE reply_on_demand SET instruction_before = ?1, updated_at_ms = ?2 WHERE id = 1",
                OnDemandOption::ChangeAfter => "UPDATE reply_on_demand SET change_after = ?1, updated_at_ms = ?2 WHERE id = 1",
            };
            c.execute(sql, params![flag(enabled), now_ms]).map_err(db)?;
            Ok(())
        })
    }

    // ------------------------------------------------------------ retrieval

    /// One retrieval setting changes alone: the row is read and written in the same transaction, through the contract's `apply_retrieval_change`
    /// (`bad_retrieval_setting`). Returns the settings after the change.
    pub fn reply_apply_retrieval_change(&self, change: RetrievalChange, now_ms: i64) -> R<RetrievalSettings> {
        self.reply_write(move |c| {
            let s = apply_retrieval_change(retrieval_in(c)?, change)?;
            c.execute(
                "UPDATE retrieval_settings SET retrieval_prompt = ?1, questions_max = ?2, excerpts_per_question = ?3, excerpt_size = ?4, attach_date = ?5, attach_sender = ?6, \
                 attach_folder = ?7, attach_subject = ?8, keep_n = ?9, clash_rule = ?10, ai_filter_question = ?11, scope = ?12, updated_at_ms = ?13 WHERE id = 1",
                params![s.retrieval_prompt, s.questions_max, s.excerpts_per_question, excerpt_size_key(s.excerpt_size), flag(s.attached_fields.date), flag(s.attached_fields.sender),
                    flag(s.attached_fields.folder), flag(s.attached_fields.subject), s.keep_n, clash_rule_key(s.clash_rule), s.ai_filter_question, retrieval_scope_key(s.scope), now_ms]).map_err(db)?;
            Ok(s)
        })
    }

    // ------------------------------------------------------------ sources

    /// A source's switch. A source that does not exist is `NotFound` (a calendar as "calendar", the rest as "source"). Turning one on starts no sync here:
    /// the sources lane reacts to the stored switch (its count and progress come back through `reply_source_progress`).
    pub fn reply_set_source_enabled(&self, source: ReplySourceRef, enabled: bool, now_ms: i64) -> R<()> {
        self.reply_write(move |c| {
            check_source_ref(source.clone())?;
            let key = reply_source_key(source.clone());
            // delta 24: the switch of a GONE folder only changes the owner's remembered choice (`source_presence_after_owner_switch`)
            let n = c.execute("UPDATE reply_source SET owner_enabled = CASE WHEN gone = 1 THEN ?2 ELSE owner_enabled END, enabled = CASE WHEN gone = 1 THEN 0 ELSE ?2 END, updated_at_ms = ?3 WHERE source_key = ?1", params![key, flag(enabled), now_ms]).map_err(db)?;
            if n == 0 {
                return Err(match source { ReplySourceRef::Calendar { calendar_id } => not_found("calendar", calendar_id), _ => not_found("source", key) });
            }
            Ok(())
        })
    }

    /// Adds a folder of this Mac, enabled (the same folder twice is one source, switched on). Returns the source key. Whether the disk holds the folder
    /// is the engine's to say; this only refuses a path that cannot be one (`bad_local_folder`).
    pub fn reply_add_local_folder(&self, path: String, now_ms: i64) -> R<String> {
        self.reply_write(move |c| {
            let key = reply_source_key(ReplySourceRef::LocalFolder { path: check_local_folder_path(path)? });
            c.execute("INSERT INTO reply_source (source_key, kind, account_id, enabled, updated_at_ms) VALUES (?1, 'local_folder', NULL, 1, ?2) \
                       ON CONFLICT(source_key) DO UPDATE SET enabled = 1, updated_at_ms = excluded.updated_at_ms", params![key, now_ms]).map_err(db)?;
            Ok(key)
        })
    }

    /// Removes a folder of this Mac from the sources. Unknown = `NotFound`. (The files on the disk are not touched; the indexed excerpts are the indexer lane's.)
    pub fn reply_remove_local_folder(&self, path: String) -> R<()> {
        self.reply_write(move |c| {
            let normal = check_local_folder_path(path.clone())?;
            let key = reply_source_key(ReplySourceRef::LocalFolder { path: normal });
            if c.execute("DELETE FROM reply_source WHERE source_key = ?1", [key]).map_err(db)? == 0 { return Err(not_found("folder", path)); }
            Ok(())
        })
    }

    /// Registers a source the owner has not switched yet (a folder of an account the sync found, a calendar the Agendas pane connected). Off by default;
    /// an existing row is left as it is. A mail folder needs its account to exist.
    pub fn reply_source_ensure(&self, source: ReplySourceRef, now_ms: i64) -> R<()> {
        self.reply_write(move |c| {
            check_source_ref(source.clone())?;
            let (kind, account) = match &source {
                ReplySourceRef::MailFolder { account_id, .. } => ("mail_folder", Some(account_id.clone())),
                ReplySourceRef::LocalFolder { .. } => ("local_folder", None),
                ReplySourceRef::Calendar { .. } => ("calendar", None),
            };
            c.execute("INSERT OR IGNORE INTO reply_source (source_key, kind, account_id, enabled, updated_at_ms) VALUES (?1, ?2, ?3, 0, ?4)", params![reply_source_key(source), kind, account, now_ms]).map_err(db)?;
            Ok(())
        })
    }

    /// The (account, folder) pairs whose first sync is complete: the sync fetched the folder's last page (`folder_sync_state.last_full_pass_ms` is set), so a
    /// folder with no message among them IS empty, not "not walked yet". Read on its own connection, never the views' reader.
    pub fn reply_folder_first_sync(&self) -> R<std::collections::HashSet<(String, String)>> {
        let c = crate::open::open_reader(&self.path).map_err(from_store)?;
        let mut s = c.prepare("SELECT account_id, folder FROM folder_sync_state WHERE last_full_pass_ms IS NOT NULL").map_err(db)?;
        let rows = s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).map_err(db)?.collect::<Result<std::collections::HashSet<_>, _>>().map_err(db)?;
        Ok(rows)
    }

    /// What the sync found in one account's LIST: every folder it syncs, with the owner's defaults for a NEW installation (Inbox and Sent on, every other
    /// folder off: `default_on`). A folder that already has a row is left exactly as it is (the owner's switch, its count and progress are never reset by a
    /// later sync or a restart). A folder that has a row but is no longer in a non-empty LIST (removed on the server) is switched OFF and its row is KEPT, with
    /// its count; an empty LIST changes nothing (a server that answered nothing is not a server that lost every folder). One transaction on the writer thread
    /// (never the views' reader). The account has to exist.
    pub fn reply_register_folders(&self, account_id: String, folders: Vec<(String, bool)>, now_ms: i64) -> R<()> {
        self.reply_write(move |c| {
            let mut listed: Vec<String> = vec![];
            for (folder, default_on) in folders {
                let source = ReplySourceRef::MailFolder { account_id: account_id.clone(), folder };
                check_source_ref(source.clone())?;
                let key = reply_source_key(source);
                c.execute("INSERT OR IGNORE INTO reply_source (source_key, kind, account_id, enabled, updated_at_ms) VALUES (?1, 'mail_folder', ?2, ?3, ?4)",
                    params![key, account_id, flag(default_on), now_ms]).map_err(db)?;
                listed.push(key);
            }
            if !listed.is_empty() {
                // delta 24: every folder of the account that the LIST no longer holds goes (`source_presence_after_list`: off, the owner's switch remembered, the count kept);
                // one that is back returns with the owner's switch. A folder that is present and was never gone is not touched.
                let mut s = c.prepare("SELECT source_key, gone FROM reply_source WHERE kind = 'mail_folder' AND account_id = ?1").map_err(db)?;
                let all = s.query_map([&account_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? == 1))).map_err(db)?.collect::<Result<Vec<_>, _>>().map_err(db)?;
                for (key, gone) in all {
                    let present = listed.contains(&key);
                    if present == gone { presence_write(c, &key, present, u64::MAX, now_ms)?; }
                }
            }
            Ok(())
        })
    }

    /// Delta 24: the owner's remembered switch of every GONE source, for `reply/sources` (`reply_sources_json_with_choices`).
    pub fn reply_owner_choices(&self) -> R<Vec<SourceChoice>> {
        self.reply_read(|c| {
            let mut s = c.prepare_cached("SELECT source_key, owner_enabled FROM reply_source WHERE gone = 1 AND owner_enabled IS NOT NULL ORDER BY source_key").map_err(db)?;
            let rows = s.query_map([], |r| Ok(SourceChoice { source_key: r.get(0)?, owner_enabled: r.get::<_, i64>(1)? == 1 })).map_err(db)?.collect::<Result<Vec<_>, _>>().map_err(db)?;
            Ok(rows)
        })
    }

    /// Delta 24: a LIST of the account's folders did (`present` true) or did not (false) hold this mail folder. The rule is the contract's `source_presence_after_list`:
    /// a missing folder is switched off with the owner's switch remembered (`owner_enabled`) and keeps its last count; one that comes back returns with that switch.
    /// False = no such source.
    pub fn reply_source_presence(&self, source: ReplySourceRef, present: bool, now_ms: i64) -> R<bool> {
        self.reply_write(move |c| presence_write(c, &reply_source_key(source), present, u64::MAX, now_ms))
    }

    /// The background sync's report on one source: its item count and where it stands. False = no such source.
    pub fn reply_source_progress(&self, source: ReplySourceRef, item_count: u64, progress: ReplySyncProgress, now_ms: i64) -> R<bool> {
        self.reply_write(move |c| {
            if progress == ReplySyncProgress::Gone { return presence_write(c, &reply_source_key(source), false, item_count, now_ms); }   // delta 24: a report of Gone IS the folder missing from a LIST
            let (state, percent, words) = match progress {
                ReplySyncProgress::Gone => unreachable!("handled above"),
                ReplySyncProgress::NotStarted => ("not_started", None, None),
                ReplySyncProgress::Syncing { percent } => ("syncing", Some(i64::from(percent.min(100))), None),
                ReplySyncProgress::UpToDate => ("up_to_date", None, None),
                ReplySyncProgress::Failed { reason_words } => ("failed", None, Some(reason_words)),
            };
            let n = c.execute("UPDATE reply_source SET item_count = ?2, progress_state = ?3, progress_percent = ?4, progress_words = ?5, updated_at_ms = ?6 WHERE source_key = ?1",
                params![reply_source_key(source), item_count as i64, state, percent, words, now_ms]).map_err(db)?;
            Ok(n > 0)
        })
    }

    // ------------------------------------------------------------ voice and tones

    pub fn reply_set_voice_text(&self, text: String, now_ms: i64) -> R<()> {
        self.reply_write(move |c| { c.execute("UPDATE reply_voice SET tone_text = ?1, updated_at_ms = ?2 WHERE id = 1", params![text, now_ms]).map_err(db)?; Ok(()) })
    }

    /// 0 to `REPLY_EXAMPLES_CEILING` earlier replies to the same person (`bad_voice_sample`).
    pub fn reply_set_same_person_examples(&self, count: u32, now_ms: i64) -> R<()> {
        self.reply_write(move |c| {
            let n = check_same_person_examples(count)?;
            c.execute("UPDATE reply_voice SET same_person_examples = ?1, updated_at_ms = ?2 WHERE id = 1", params![n, now_ms]).map_err(db)?;
            Ok(())
        })
    }

    /// What a finished voice analysis writes: how many sent messages it read, when, and (when it wrote one) the tone text. For the voice lane.
    pub fn reply_record_voice_analysis(&self, sample_size: u32, analysed_at_ms: i64, tone_text: Option<String>) -> R<()> {
        self.reply_write(move |c| {
            let n = check_voice_sample_size(sample_size)?;
            c.execute("UPDATE reply_voice SET sample_size = ?1, analysed_at_ms = ?2, tone_text = coalesce(?3, tone_text), updated_at_ms = ?2 WHERE id = 1", params![n, analysed_at_ms, tone_text]).map_err(db)?;
            Ok(())
        })
    }

    /// An empty id creates a tone (the id comes back), another id replaces it (`check_reply_tone`, `bad_reply_tone`). The name is stored trimmed.
    pub fn reply_save_tone(&self, tone: ReplyTone) -> R<String> {
        self.reply_write(move |c| {
            check_reply_tone(tone.clone(), tones_in(c)?)?;
            let id = if tone.id.is_empty() { new_id("tone") } else { tone.id.clone() };
            let order = next_order(c, "reply_tone")?;
            c.execute("INSERT INTO reply_tone (id, name, text, sort_order) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(id) DO UPDATE SET name = excluded.name, text = excluded.text",
                params![id, tone.name.trim(), tone.text, order]).map_err(db)?;
            Ok(id)
        })
    }

    /// Removes a tone; the uses that chose it fall back to the voice (the migration's trigger). Unknown = `NotFound`.
    pub fn reply_delete_tone(&self, tone_id: String) -> R<()> {
        self.reply_write(move |c| {
            if c.execute("DELETE FROM reply_tone WHERE id = ?1", [&tone_id]).map_err(db)? == 0 { return Err(not_found("tone", tone_id)); }
            Ok(())
        })
    }

    // ------------------------------------------------------------ prompt and notes

    /// A new version that becomes the current one (`check_reply_prompt_text`, `bad_reply_prompt`; an unknown prompt is `NotFound`). Returns its number.
    pub fn reply_save_prompt_version(&self, prompt_id: String, text: String, now_ms: i64) -> R<u32> {
        self.reply_write(move |c| {
            check_reply_prompt_text(text.clone())?;
            prompt_exists(c, &prompt_id)?;
            push_prompt_version(c, &prompt_id, &text, now_ms)
        })
    }

    /// An earlier version's text saved as a NEW version (history only grows). A version that does not exist is `NotFound`. Returns the new number.
    pub fn reply_restore_prompt_version(&self, prompt_id: String, version: u32, now_ms: i64) -> R<u32> {
        self.reply_write(move |c| {
            let mine = prompts_in(c, &prompt_id)?;
            check_prompt_version_exists(mine.iter().map(|p| p.version).collect(), version)?;
            let text = mine.into_iter().find(|p| p.version == version).map(|p| p.text).unwrap_or_default();
            push_prompt_version(c, &prompt_id, &text, now_ms)
        })
    }

    /// An empty id creates a note (the id comes back), another id replaces it (`check_fixed_note`, `bad_fixed_note`).
    pub fn reply_save_fixed_note(&self, note: FixedNote) -> R<String> {
        self.reply_write(move |c| {
            check_fixed_note(note.clone())?;
            let id = if note.id.is_empty() { new_id("note") } else { note.id.clone() };
            let order = next_order(c, "fixed_note")?;
            c.execute("INSERT INTO fixed_note (id, title, text, sort_order) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(id) DO UPDATE SET title = excluded.title, text = excluded.text",
                params![id, note.title.trim(), note.text, order]).map_err(db)?;
            Ok(id)
        })
    }

    pub fn reply_delete_fixed_note(&self, note_id: String) -> R<()> {
        self.reply_write(move |c| {
            if c.execute("DELETE FROM fixed_note WHERE id = ?1", [&note_id]).map_err(db)? == 0 { return Err(not_found("note", note_id)); }
            Ok(())
        })
    }

    // ------------------------------------------------------------ calendars the shell pushes (delta 21, migration 0014)

    /// Every calendar the core holds, by key, with its switch (the `Calendar` source row). A calendar with a source row and no `reply_calendar` row (registered
    /// by `reply_source_ensure` before the shell listed it) shows its id as its name.
    pub fn reply_calendars(&self) -> R<Vec<ReplyCalendar>> { self.reply_read(calendars_in) }

    /// The events the core kept, by start.
    pub fn reply_calendar_events(&self) -> R<Vec<CalendarEvent>> {
        self.reply_read(|c| {
            let mut s = c.prepare_cached("SELECT calendar_id, event_id, title, start_ms, end_ms, attendees_json, location FROM calendar_event ORDER BY start_ms, calendar_id, event_id").map_err(db)?;
            let raw = s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, String>(5)?, r.get::<_, Option<String>>(6)?)))
                .map_err(db)?.collect::<Result<Vec<_>, _>>().map_err(db)?;
            raw.into_iter().map(|(calendar_id, event_id, title, start_ms, end_ms, attendees_json, location)| {
                let mut a = c.prepare_cached("SELECT value FROM json_each(?1) ORDER BY key").map_err(db)?;
                let attendees = a.query_map([attendees_json], |r| r.get::<_, String>(0)).map_err(db)?.collect::<Result<Vec<_>, _>>().map_err(db)?;
                Ok(CalendarEvent { event_id, calendar_id, title, start_ms, end_ms, attendees, location })
            }).collect()
        })
    }

    /// What the shell pushes: the contract's `apply_calendar_sync` decides (a known calendar keeps its switch, a new one starts off, a calendar no longer listed
    /// goes with its switch and events, an event is kept only for an enabled listed calendar inside the window); this stores its answer in ONE transaction.
    /// The events inside the window are replaced by the kept ones; the events outside it are left as they were. Refused with `bad_calendar_sync`.
    pub fn reply_sync_calendars(&self, calendars: Vec<ReplyCalendarInfo>, window_start_ms: i64, window_end_ms: i64, events: Vec<CalendarEvent>, now_ms: i64) -> R<CalendarSyncResult> {
        self.reply_write(move |c| {
            let result = apply_calendar_sync(calendars_in(c)?, calendars, window_start_ms, window_end_ms, events)?;
            for id in &result.dropped_calendar_ids {
                c.execute("DELETE FROM reply_calendar WHERE calendar_id = ?1", [id]).map_err(db)?;
                c.execute("DELETE FROM reply_source WHERE source_key = ?1", [reply_source_key(ReplySourceRef::Calendar { calendar_id: id.clone() })]).map_err(db)?;
            }
            for cal in &result.calendars {
                c.execute("INSERT INTO reply_calendar (calendar_id, name, account_label, colour, updated_at_ms) VALUES (?1, ?2, ?3, ?4, ?5) \
                           ON CONFLICT(calendar_id) DO UPDATE SET name = excluded.name, account_label = excluded.account_label, colour = excluded.colour, updated_at_ms = excluded.updated_at_ms",
                    params![cal.calendar_id, cal.name, cal.account_label, cal.colour, now_ms]).map_err(db)?;
                c.execute("INSERT OR IGNORE INTO reply_source (source_key, kind, account_id, enabled, updated_at_ms) VALUES (?1, 'calendar', NULL, 0, ?2)",
                    params![reply_source_key(ReplySourceRef::Calendar { calendar_id: cal.calendar_id.clone() }), now_ms]).map_err(db)?;
            }
            c.execute("DELETE FROM calendar_event WHERE start_ms < ?2 AND end_ms >= ?1", params![window_start_ms, window_end_ms]).map_err(db)?;
            for e in &result.events {
                let attendees = format!("[{}]", e.attendees.iter().map(|a| json_text(a)).collect::<Vec<_>>().join(","));
                c.execute("INSERT OR REPLACE INTO calendar_event (calendar_id, event_id, title, start_ms, end_ms, attendees_json, location) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![e.calendar_id, e.event_id, e.title, e.start_ms, e.end_ms, attendees, e.location]).map_err(db)?;
            }
            Ok(result)
        })
    }

    // ------------------------------------------------------------ progress of the background crawl (delta 21)

    /// Per account and folder: messages stored, bodies stored, bodies whose chunks are all in the meaning index (`FolderProgress`; indexed <= bodies <= messages).
    /// Read on the BACKGROUND connection (`background_reads`, sync-full-r1): the scan over the message table never holds the reader the views wait on.
    pub fn reply_folder_progress(&self) -> R<Vec<FolderProgress>> {
        let rows = self.background_reads().map_err(from_store)?.folder_progress().map_err(from_store)?;
        for p in &rows { check_folder_progress(p.clone())?; }
        Ok(rows)
    }

    // ------------------------------------------------------------ passes (the counts' only source)

    /// The auto use's verdict on one message: a message looked at again REPLACES its row. An illegal shape is refused (`reply_pass_is_legal`).
    pub fn reply_record_pass(&self, pass: ReplyPass) -> R<()> {
        self.reply_write(move |c| {
            if !reply_pass_is_legal(pass.clone()) {
                return Err(CoreError::Invalid { field: "bad_reply_pass".into(), reason_words: "a message that a step stopped has no later step, and a draft needs all three steps through".into() });
            }
            c.execute("INSERT INTO reply_pass (message_id, received_at_ms, entry_result, ai_result, exit_result, draft_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                       ON CONFLICT(message_id) DO UPDATE SET received_at_ms = excluded.received_at_ms, entry_result = excluded.entry_result, ai_result = excluded.ai_result, \
                       exit_result = excluded.exit_result, draft_id = excluded.draft_id",
                params![pass.message_id, pass.received_at_ms, reply_step_result_key(pass.entry), reply_step_result_key(pass.ai), reply_step_result_key(pass.exit), pass.draft_id]).map_err(db)?;
            Ok(())
        })
    }
}

// ---------------------------------------------------------------- reply-engine: the drafts' moves, the auto use's pass, the model the Writer uses
// (additive: the draft engine behind the engine's `ReplyWork`, docs/lanes/reply-engine.md). Each move is ONE transaction and goes through the contract's
// rule for it (`reply_draft_replaced`, `reply_draft_after_change`, `reply_draft_after_accept`, `reply_draft_after_discard`), read against the row as it is
// INSIDE the transaction, so a draft discarded while the model was writing is never rewritten.

/// A JSON array of strings (the excerpt ids; a chunk id holds U+001F, so control characters are escaped).
fn json_strings(items: &[String]) -> String {
    let mut o = String::from("[");
    for (i, s) in items.iter().enumerate() {
        if i > 0 { o.push(','); }
        o.push('"');
        for ch in s.chars() {
            match ch {
                '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"),
                ch if (ch as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", ch as u32)),
                ch => o.push(ch),
            }
        }
        o.push('"');
    }
    o.push(']');
    o
}

fn draft_by_id_in(c: &Connection, id: &str) -> R<ReplyDraft> {
    draft_in(c, &format!("{DRAFT_COLUMNS} WHERE id = ?1"), id)?.ok_or_else(|| not_found("reply draft", id))
}

/// One pass row, inserted or replaced (the same statement as `reply_record_pass`, inside the caller's transaction).
fn put_pass_in(c: &Connection, pass: &ReplyPass) -> R<()> {
    if !reply_pass_is_legal(pass.clone()) {
        return Err(CoreError::Invalid { field: "bad_reply_pass".into(), reason_words: "a message that a step stopped has no later step, and a draft needs all three steps through".into() });
    }
    c.execute("INSERT INTO reply_pass (message_id, received_at_ms, entry_result, ai_result, exit_result, draft_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
               ON CONFLICT(message_id) DO UPDATE SET received_at_ms = excluded.received_at_ms, entry_result = excluded.entry_result, ai_result = excluded.ai_result, \
               exit_result = excluded.exit_result, draft_id = excluded.draft_id",
        params![pass.message_id, pass.received_at_ms, reply_step_result_key(pass.entry), reply_step_result_key(pass.ai), reply_step_result_key(pass.exit), pass.draft_id]).map_err(db)?;
    Ok(())
}

fn set_draft_in(c: &Connection, d: &ReplyDraft) -> R<()> {
    c.execute("UPDATE reply_draft SET text = ?2, state = ?3, revision = ?4, updated_at_ms = ?5 WHERE id = ?1",
        params![d.id, d.text, reply_draft_state_key(d.state), d.revision, d.updated_at_ms]).map_err(db)?;
    Ok(())
}

impl Store {
    /// A new draft for its thread, in ONE transaction: the thread's live draft (if any) marked Discarded first (`reply_draft_replaced`; the unique index
    /// refuses the insert otherwise), then the new row, then (the auto use) the message's pass row naming the new draft. Returns the draft discarded.
    pub fn reply_draft_put(&self, draft: ReplyDraft, pass: Option<ReplyPass>) -> R<Option<ReplyDraft>> {
        self.reply_write(move |c| {
            let live = draft_in(c, &format!("{DRAFT_COLUMNS} WHERE thread_id = ?1 AND state IN ('proposed','edited')"), &draft.thread_id)?;
            let gone = reply_draft_replaced(live, draft.created_at_ms);
            if let Some(old) = &gone { set_draft_in(c, old)?; }
            c.execute("INSERT INTO reply_draft (id, thread_id, message_id, text, made_by, excerpt_ids_json, instruction, state, revision, created_at_ms, updated_at_ms) \
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![draft.id, draft.thread_id, draft.message_id, draft.text, reply_made_by_key(draft.made_by), json_strings(&draft.excerpt_ids), draft.instruction,
                    reply_draft_state_key(draft.state), draft.revision, draft.created_at_ms, draft.updated_at_ms]).map_err(db)?;
            if let Some(p) = &pass { put_pass_in(c, p)?; }
            Ok(gone)
        })
    }

    /// The draft rewritten by a change request: the row as it is now, through `reply_draft_after_change` (a closed draft is refused, nothing written).
    pub fn reply_draft_rewrite(&self, draft_id: String, text: String, now_ms: i64) -> R<ReplyDraft> {
        self.reply_write(move |c| {
            let next = reply_draft_after_change(draft_by_id_in(c, &draft_id)?, text, now_ms)?;
            set_draft_in(c, &next)?;
            Ok(next)
        })
    }

    /// The owner kept the draft: Proposed becomes Edited (`reply_draft_after_accept`).
    pub fn reply_draft_accept(&self, draft_id: String, now_ms: i64) -> R<ReplyDraft> {
        self.reply_write(move |c| {
            let next = reply_draft_after_accept(draft_by_id_in(c, &draft_id)?, now_ms)?;
            set_draft_in(c, &next)?;
            Ok(next)
        })
    }

    /// The owner threw the draft away (`reply_draft_after_discard`).
    pub fn reply_draft_discard(&self, draft_id: String, now_ms: i64) -> R<ReplyDraft> {
        self.reply_write(move |c| {
            let next = reply_draft_after_discard(draft_by_id_in(c, &draft_id)?, now_ms)?;
            set_draft_in(c, &next)?;
            Ok(next)
        })
    }

    /// The pass row of one message, if the auto use looked at it.
    pub fn reply_pass_of(&self, message_id: &str) -> R<Option<ReplyPass>> {
        let id = message_id.to_string();
        self.reply_read(move |c| {
            let row: Option<(i64, String, String, String, Option<String>)> = c.query_row(
                "SELECT received_at_ms, entry_result, ai_result, exit_result, draft_id FROM reply_pass WHERE message_id = ?1", [&id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))).optional().map_err(db)?;
            row.map(|(at, e, a, x, d)| Ok(ReplyPass { message_id: id.clone(), received_at_ms: at, entry: step_of(&e)?, ai: step_of(&a)?, exit: step_of(&x)?, draft_id: d })).transpose()
        })
    }

}

/// Reads a source's presence columns, applies the contract's rule, writes the result. `item_count` u64::MAX = keep the stored count.
fn presence_write(c: &rusqlite::Connection, key: &str, present: bool, item_count: u64, now_ms: i64) -> R<bool> {
    let row = c.query_row("SELECT enabled, owner_enabled, gone FROM reply_source WHERE source_key = ?1", params![key],
        |r| Ok(SourcePresence { enabled: r.get::<_, i64>(0)? == 1, owner_enabled: r.get::<_, Option<i64>>(1)?.map(|v| v == 1), gone: r.get::<_, i64>(2)? == 1 })).optional().map_err(db)?;
    let Some(cur) = row else { return Ok(false); };
    let next = source_presence_after_list(cur, present);
    c.execute("UPDATE reply_source SET enabled = ?2, owner_enabled = ?3, gone = ?4, item_count = CASE WHEN ?5 < 0 THEN item_count ELSE ?5 END, updated_at_ms = ?6 WHERE source_key = ?1",
        params![key, next.enabled, next.owner_enabled, next.gone, if item_count == u64::MAX { -1 } else { item_count as i64 }, now_ms]).map_err(db)?;
    Ok(true)
}
