//! Delta 18 in the real core: remind me if there is no reply (docs/lanes/contracts.md "Delta 18", rule 5). The store holds the facts (the
//! time, the first reply of the thread); the contract's `reply_reminder_state` turns them into waiting, fired or answered, so the state
//! is DERIVED at every read and never kept twice. This file serves `ClearReplyReminder`, the state path `thread/<id>/reminder`, and the
//! one write at the firing (`reminder.fired_at_ms`, once, so a notification fires once). The Reminders view itself is
//! `reads::inbox` over `StoredView::Reminders`.
use contracts::{
    check_clear_reply_reminder, check_remind_if_no_reply, reply_reminder_state, reply_reminder_state_key, CommandOutcome, CoreError, ReplyReminderState,
};
use store::{ReminderFacts, Store};

use crate::reads::json_text;

const OK: CommandOutcome = CommandOutcome { undo_entry_id: None, created_id: None };

pub(crate) fn store_failed(what: &str, e: store::StoreError) -> CoreError {
    CoreError::Unexpected { reason_words: format!("{what}: {e}") }
}

/// The contract's state of a reminder's facts. A reminder still on a draft is waiting: only a sent message can go unanswered.
fn state_of(facts: Option<&ReminderFacts>, now_ms: i64) -> ReplyReminderState {
    match facts {
        None => ReplyReminderState::NoReminder,
        Some(f) if f.on_draft => ReplyReminderState::Waiting { at_ms: f.at_ms },
        Some(f) => reply_reminder_state(Some(f.at_ms), f.first_reply_ms, now_ms),
    }
}

/// `RemindIfNoReply { draft_id, at_ms }`: the reminder is stored on the draft (its column) or, once the draft was queued, on its outbox
/// entry (the envelope's `remind_if_no_reply_at_ms`). An id that is neither is refused `NotFound` first, then a time that is now or past
/// in the contract's words (`reminder_in_the_past`), the order the fixture core uses. `created_id` is the id given, as the fixture
/// returns it. A thread holds one reminder: setting one on a draft replaces the one another draft of the same reply thread held.
pub(crate) fn set(store: &Store, id: &str, at_ms: i64, now_ms: i64) -> Result<CommandOutcome, CoreError> {
    if !store.reply_reminder_target_exists(id).map_err(|e| store_failed("the draft could not be read", e))? {
        return Err(CoreError::NotFound { kind: "draft".into(), id: id.to_string() });
    }
    check_remind_if_no_reply(at_ms, now_ms)?;
    store.remind_if_no_reply(id.to_string(), at_ms, now_ms).map_err(|e| store_failed("the reminder could not be stored", e))?;
    Ok(CommandOutcome { undo_entry_id: None, created_id: Some(id.to_string()) })
}

/// The message went out: the reminder the draft (or its outbox entry) held rides the sent message from now on (a `reminder` row), where
/// a reply of its thread answers it and the Reminders view lists it once it is due with no reply. Returns the reminder's id, None when
/// the draft held none. The send path calls it when it stores the sent copy; it is idempotent (the same sent message, the same row).
pub fn follow_sent(store: &Store, draft_id: &str, sent_message_id: &str, now_ms: i64) -> Result<Option<String>, CoreError> {
    store.reply_reminder_on_sent(draft_id.to_string(), sent_message_id.to_string(), now_ms).map_err(|e| store_failed("the reminder could not follow the sent message", e))
}

/// `ClearReplyReminder { draft_id }`: the id is the one `thread/<id>/reminder` names as `draft_id` (a draft, the sent message, or the
/// thread). Nothing to clear is refused in the contract's words; a waiting, a fired and an answered reminder are all cleared, which is
/// how a row of the Reminders view is dismissed.
pub(crate) fn clear(store: &Store, id: &str, now_ms: i64) -> Result<CommandOutcome, CoreError> {
    let facts = store.reply_reminder_facts(id).map_err(|e| store_failed("the reminder could not be read", e))?;
    // a reminder an outbox entry still holds (the message waits to leave) is waiting, and is cleared from the entry
    let held = if facts.is_none() { store.outbox_reply_reminder(id).map_err(|e| store_failed("the reminder could not be read", e))? } else { None };
    let state = match held { Some(at_ms) => ReplyReminderState::Waiting { at_ms }, None => state_of(facts.as_ref(), now_ms) };
    check_clear_reply_reminder(state)?;
    if held.is_some() { store.outbox_reply_reminder_clear(id.to_string()).map_err(|e| store_failed("the reminder could not be cleared", e))?; }
    store.reply_reminder_clear(id).map_err(|e| store_failed("the reminder could not be cleared", e))?;
    Ok(OK)
}

/// `thread/<id>/reminder`: `{at_ms, draft_id, state}` (keys sorted, as the fixture core writes them); `<id>` is a thread or one of its
/// messages. `draft_id` is what `ClearReplyReminder` takes; both are null when no reminder is set.
pub(crate) fn state(store: &Store, id: &str, now_ms: i64) -> Option<String> {
    let facts = store.reply_reminder_facts(id).ok()?;
    if facts.is_none() {
        // set on an outbox entry that has not left yet: waiting, keyed by the id it was set with
        if let Some(at) = store.outbox_reply_reminder(id).ok().flatten() {
            return Some(format!(r#"{{"at_ms":{at},"draft_id":{},"state":"waiting"}}"#, json_text(id)));
        }
    }
    let state = state_of(facts.as_ref(), now_ms);
    let (at, key) = match (&state, &facts) {
        (ReplyReminderState::NoReminder, _) | (_, None) => ("null".to_string(), "null".to_string()),
        (_, Some(f)) => (f.at_ms.to_string(), json_text(&f.key)),
    };
    Some(format!(r#"{{"at_ms":{at},"draft_id":{key},"state":{}}}"#, json_text(&reply_reminder_state_key(state))))
}

/// The write at the firing: every reminder whose time came with no reply by then and that is not yet marked gets its `fired_at_ms`.
/// Returns the sent messages it marked: the notification goes out once for each. A reminder a reply answered is never marked.
pub(crate) fn fire_due(store: &Store, now_ms: i64) -> Vec<String> {
    store.reply_reminders_mark_fired(now_ms).unwrap_or_else(|e| {
        eprintln!("reminders: the due reminders could not be marked fired: {e}");
        Vec::new()
    })
}

/// gap-remind: the inbox rows of a thread whose reminder FIRED carry `reminder_fired = true` (the list draws the reminder chip). The set is
/// the store's projection of the same rule the Reminders view lists by, read once per list whatever its size; a store that cannot answer
/// marks nothing (a list without the chip, never an empty list).
pub(crate) fn mark_rows(store: &Store, rows: &mut [contracts::InboxRow], now_ms: i64) {
    if rows.is_empty() { return; }
    match store.reminder_fired_threads(now_ms) {
        Ok(threads) if !threads.is_empty() => {
            for r in rows.iter_mut() { r.reminder_fired = threads.contains(&r.thread_id); }
        }
        Ok(_) => {}
        Err(e) => eprintln!("reminders: the fired reminders could not be read for the list: {e}"),
    }
}

/// gap-remind: the firing, and the return. `fire_due` marks each due reminder once; then the mail the owner had put away in each of those
/// threads (archived, or sitting in an Archive folder) comes back to the inbox through `Unarchive` (the server's copy moves too; no undo entry: the owner did not ask for it), so the
/// thread "returns to the inbox marked with the reminder". Returns the sent messages marked, as `fire_due` does (the notification goes
/// out once). A thread still in the inbox is only marked (by `mark_rows`); a muted or snoozed one keeps its own place.
pub(crate) fn fire_due_and_return(store: &Store, roles: &crate::imap_source::Roles, now_ms: i64) -> Vec<String> {
    let fired = fire_due(store, now_ms);
    if fired.is_empty() { return fired; }
    let accounts: Vec<String> = store.account_summaries().unwrap_or_default().into_iter().map(|a| a.id).collect();
    let archive = crate::reads::folders_with_role(store, roles, &accounts, sync_imap::FolderRole::Archive);
    let away: Vec<String> = match store.reminder_return_candidates(&fired) {
        Ok(list) => list
            .into_iter()
            .filter(|m| (m.archived && m.folder == store::INBOX_FOLDER) || archive.iter().any(|(a, f)| *a == m.account_id && *f == m.folder))
            .map(|m| m.message_id)
            .collect(),
        Err(e) => { eprintln!("reminders: the threads of the fired reminders could not be read: {e}"); Vec::new() }
    };
    if !away.is_empty() {
        if let Err(e) = crate::unarchive::run_recording(store, roles, away, false) { eprintln!("reminders: the fired threads could not return to the inbox: {e:?}"); }
    }
    fired
}
