//! lane real-notes: the real engine serves the private notes (AddNote, EditNote, DeleteNote), the outside apps' keeping (SetMcpGrant, SetMcpServer,
//! AnswerMcpApproval), the label suggestions (AcceptSuggestion, DismissSuggestion), MergeContacts and BlockDomain, over the store
//! (`store/src/notes_mcp.rs`, `contacts.rs`, `label_meta.rs`), and the state paths and reads the app uses for them. They were refused before
//! with "The local engine does not do this yet." (docs/lanes/real-notes.md has the table).
//!
//! - MCP: the engine only KEEPS the grants, the switch and the answers. It starts no server, opens no socket and dials nothing: a grant
//!   write is one row in this Mac's database.
//! - Notes here are the owner's private notes on a thread or a message (contract `Note`), not the reply writer's fixed notes (replies.rs).
//! - A note's three writes record an undo entry (`note_add`, `note_edit`, `note_delete`); the mail actions' Undo cannot reverse those kinds, so
//!   `rust_core.rs` hands Z to `undo` here when the newest entry is a note's (`newest_is_note`).
//! - Paths: note/count · note/<id>/text · note/<id>/target · mcp_server/on · mcp_grant/<app> · mcp_request/<id>/state · mcp_request/<id>/results ·
//!   mcp_request/pending_count · suggestion/count · suggestion/last · suggestion/<id>/<kind|subject|accepted|label_id|offered_at_ms|words> ·
//!   block/count · block/list · block/<address>/blocked · contact/count · contact/<id>/addresses · contacts/count. The shapes are the test
//!   engine's (Sources/CoreBridge/FixtureCore.swift `state(path:)`, FixtureLabels.swift): a string is a JSON string, a number or a bool is bare.
use std::sync::Arc;

use contracts::*;
use store::{MergeResult, Refusal, Store, StoreError};

use crate::reads::json_text;
use crate::rust_core::now_ms;

const OK: CommandOutcome = CommandOutcome { undo_entry_id: None, created_id: None };

/// Whether the command is one of the ten (what `RustCore::run` hands here).
pub(crate) fn is_command(c: &Command) -> bool {
    matches!(c,
        Command::AddNote { .. } | Command::EditNote { .. } | Command::DeleteNote { .. }
        | Command::SetMcpGrant { .. } | Command::SetMcpServer { .. } | Command::AnswerMcpApproval { .. }
        | Command::AcceptSuggestion { .. } | Command::DismissSuggestion { .. }
        | Command::MergeContacts { .. } | Command::BlockDomain { .. })
}

fn refusal(r: Refusal) -> CoreError {
    match r {
        Refusal::Invalid { field, words } => CoreError::Invalid { field, reason_words: words },
        Refusal::NotFound { kind, id } => CoreError::NotFound { kind, id },
    }
}

/// A store refusal made in words (`StoreError::Invalid`) is the owner's to read; anything else is unexpected.
fn words_or_unexpected(field: &str, e: StoreError) -> CoreError {
    match e {
        StoreError::Invalid(w) => CoreError::Invalid { field: field.into(), reason_words: w },
        other => other.into(),
    }
}

/// A domain as the test engine's `blockDomainLocked` takes it: no "@" but a leading one, no space, at least one dot; lower case.
fn clean_domain(domain: &str) -> Result<String, CoreError> {
    let d = domain.trim().trim_matches('@').to_lowercase();
    if d.is_empty() || !d.contains('.') || d.contains('@') || d.contains(char::is_whitespace) {
        return Err(CoreError::Invalid { field: "domain".into(), reason_words: "a domain looks like example.com".into() });
    }
    Ok(d)
}

/// Runs one of the ten. `plain_loopback` is the core's test transport flag (the unsubscribe a suggestion may start goes through the same guarded code).
pub(crate) fn run(store: &Arc<Store>, plain_loopback: bool, command: Command) -> Result<CommandOutcome, CoreError> {
    let now = now_ms();
    match command {
        Command::AddNote { target, text } => {
            let (id, entry) = store
                .write(move |ctx| {
                    let note = match ctx.note_add(&target, &text, now)? { Ok(n) => n, Err(r) => return Ok(Err(r)) };
                    let entry = actions::usecases::undo::push_entry(ctx, &UndoAction::NoteAdd { note_id: note.id.clone() }, now)?;
                    Ok(Ok((note.id, entry)))
                })?
                .map_err(refusal)?;
            Ok(CommandOutcome { undo_entry_id: Some(entry), created_id: Some(id) })
        }
        Command::EditNote { note_id, text } => {
            let entry = store
                .write(move |ctx| {
                    let previous = match ctx.note_edit(&note_id, &text, now)? { Ok(p) => p, Err(r) => return Ok(Err(r)) };
                    Ok(Ok(actions::usecases::undo::push_entry(ctx, &UndoAction::NoteEdit { note_id, previous_text: previous }, now)?))
                })?
                .map_err(refusal)?;
            Ok(CommandOutcome { undo_entry_id: Some(entry), created_id: None })
        }
        Command::DeleteNote { note_id } => {
            let entry = store
                .write(move |ctx| {
                    let gone = match ctx.note_remove(&note_id)? { Ok(n) => n, Err(r) => return Ok(Err(r)) };
                    let entry = actions::usecases::undo::push_entry(ctx, &UndoAction::NoteDelete { note: gone.note.clone() }, now)?;
                    ctx.note_undo_record_delete(&entry, &gone)?;
                    Ok(Ok(entry))
                })?
                .map_err(refusal)?;
            Ok(CommandOutcome { undo_entry_id: Some(entry), created_id: None })
        }
        Command::SetMcpGrant { app_name, allowed } => store.mcp_grant_set(app_name, allowed, now)?.map_err(refusal).map(|_| OK),
        Command::SetMcpServer { on } => store.mcp_server_set(on, now).map(|_| OK).map_err(Into::into),
        Command::AnswerMcpApproval { request_id, allowed } => store.mcp_approval_answer(request_id, allowed, now)?.map_err(refusal).map(|_| OK),
        Command::AcceptSuggestion { suggestion_id } => {
            let Some(s) = store.suggestion_row(&suggestion_id)? else { return Err(CoreError::NotFound { kind: "suggestion".into(), id: suggestion_id }) };
            // what the contract says accepting writes; a suggestion already accepted is refused here, in its words
            let effect = suggestion_effect(s)?;
            // the part only the engine does (a mute, an unsubscribe) runs first, so a refusal leaves the suggestion on offer
            match &effect {
                SuggestionEffect::MuteThread { thread_id } => {
                    let ids = store.thread(thread_id)?.map(|t| t.message_ids).unwrap_or_default();
                    if !ids.is_empty() { crate::mute::run(store, ids, true)?; }
                }
                SuggestionEffect::Unsubscribe { subject } => {
                    let from = store.message_ids_from(subject)?;
                    if let Some(id) = from.into_iter().find(|id| crate::unsubscribe::method(store, id, plain_loopback).is_some()) {
                        crate::unsubscribe::run(store, &id, now, plain_loopback)?;
                    }
                }
                SuggestionEffect::Block { .. } | SuggestionEffect::LabelRule { .. } => {}
            }
            // the block or the label rule and the "accepted" mark are the store's one transaction
            store.suggestion_accept(suggestion_id, now).map_err(|e| words_or_unexpected("suggestion", e))?;
            Ok(OK)
        }
        Command::DismissSuggestion { suggestion_id } => {
            if store.suggestion_dismiss(suggestion_id.clone())? { Ok(OK) } else { Err(CoreError::NotFound { kind: "suggestion".into(), id: suggestion_id }) }
        }
        Command::MergeContacts { keep_id, merge_id } => match store.contacts_merge(keep_id, merge_id, now)? {
            MergeResult::Merged(_) => Ok(OK),
            MergeResult::SameCard => Err(CoreError::Invalid { field: "merge".into(), reason_words: "a card cannot be merged into itself".into() }),
            MergeResult::Unknown(id) => Err(CoreError::NotFound { kind: "contact".into(), id }),
        },
        Command::BlockDomain { domain } => {
            // the same table as a blocked sender: the junk filter reads `store.blocks()` for both (junk_filter.rs `context`), the domain form
            store.block_add(BlockTarget::Domain { domain: clean_domain(&domain)? }, now)?;
            Ok(OK)
        }
        _ => Err(CoreError::Invalid { field: "command".into(), reason_words: "not a notes, outside apps, suggestion or contact command".into() }),
    }
}

/// Whether Z belongs here: the newest entry of the undo stack not yet undone is a note's.
pub(crate) fn newest_is_note(store: &Store) -> bool { store.undo_newest_is_note().unwrap_or(false) }

/// Z for a note's entry: the note goes, the words come back, or the note comes back.
pub(crate) fn undo(store: &Store) -> Result<CommandOutcome, CoreError> {
    match store.write(|ctx| ctx.note_undo_latest())? {
        Some(done) => done.map(|_| OK).map_err(refusal),
        None => Err(CoreError::Invalid { field: "undo".into(), reason_words: "Nothing to undo".into() }),
    }
}

// ---------------------------------------------------------------- reads (the trait's lists)

pub(crate) fn notes(store: &Store, target: NoteTarget) -> Vec<Note> { store.notes_on(&target).unwrap_or_default() }
pub(crate) fn grants(store: &Store) -> Vec<McpGrant> { store.mcp_grants().unwrap_or_default() }
pub(crate) fn suggestions(store: &Store) -> Vec<Suggestion> { store.suggestions().unwrap_or_default() }
pub(crate) fn blocks(store: &Store) -> Vec<Block> { store.blocks().unwrap_or_default() }

// ---------------------------------------------------------------- MergeContacts in the people read

/// The people list with each merged card shown ONCE (`MergeContacts`): the persons whose addresses one card holds become one `Contact`
/// (the card's id, its name, all its addresses, the mail counts added per account). people.rs is not changed: it keeps listing every address
/// (its `people/*` paths still do); the contract's `contacts()` and `contact(address)` go through here. With no card made the list is returned as it is.
pub(crate) fn fold_merged(store: &Store, people: Vec<Contact>) -> Vec<Contact> {
    let cards = store.contact_cards().unwrap_or_default();
    if cards.is_empty() { return people; }
    let mut out: Vec<Contact> = Vec::with_capacity(people.len());
    let mut slot: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for p in people {
        let address = p.addresses.first().map(|a| a.to_lowercase()).unwrap_or_default();
        match cards.iter().find(|c| c.2.iter().any(|a| a.to_lowercase() == address)) {
            None => out.push(p),
            Some(card) => match slot.get(&card.0) {
                Some(&i) => { for (account, n) in p.mail_count_per_account { *out[i].mail_count_per_account.entry(account).or_insert(0) += n; } }
                None => {
                    slot.insert(card.0.clone(), out.len());
                    out.push(Contact { id: card.0.clone(), name: card.1.clone(), addresses: card.2.clone(), logo_id: p.logo_id, mail_count_per_account: p.mail_count_per_account });
                }
            },
        }
    }
    // the most mail first, as the people list is; a stable sort leaves every tie where it was
    out.sort_by_key(|c| std::cmp::Reverse(c.mail_count_per_account.values().sum::<u32>()));
    out
}

/// One contact by address: the merged card when a card holds the address, else the person as the people read gave it.
pub(crate) fn merged_contact(store: &Store, address: &str, person: Option<Contact>, all: &dyn Fn() -> Vec<Contact>) -> Option<Contact> {
    let a = match (address.find('<'), address.rfind('>')) { (Some(l), Some(r)) if l < r => address[l + 1..r].trim().to_lowercase(), _ => address.trim().to_lowercase() };
    let cards = store.contact_cards().unwrap_or_default();
    if !cards.iter().any(|c| c.2.iter().any(|x| x.to_lowercase() == a)) { return person; }
    fold_merged(store, all()).into_iter().find(|c| c.addresses.iter().any(|x| x.to_lowercase() == a))
}

// ---------------------------------------------------------------- state paths

fn kind_word(k: SuggestionKind) -> &'static str {
    match k {
        SuggestionKind::BlockSender => "blockSender",
        SuggestionKind::AlwaysJunkDomain => "alwaysJunkDomain",
        SuggestionKind::Unsubscribe => "unsubscribe",
        SuggestionKind::MuteThread => "muteThread",
        SuggestionKind::AlwaysLabel => "alwaysLabel",
    }
}

fn json_list(items: &[String]) -> String { format!("[{}]", items.iter().map(|i| json_text(i)).collect::<Vec<_>>().join(",")) }

/// The answer of one of this lane's paths; None = not one of them or nothing stored there (the caller's next reader answers).
/// `people` is the number of persons `contacts()` lists (`contacts/count`).
pub(crate) fn state(store: &Store, path: &str, people: &dyn Fn() -> usize) -> Option<String> {
    let p: Vec<&str> = path.split('/').collect();
    match p.as_slice() {
        ["note", "count"] => store.note_count().ok().map(|n| n.to_string()),
        ["note", id, "text"] => store.note_get(id).ok().flatten().map(|n| json_text(&n.note.text)),
        ["note", id, "target"] => store.note_get(id).ok().flatten().map(|n| match n.note.target {
            NoteTarget::Thread { thread_id } => json_text(&format!("thread:{thread_id}")),
            NoteTarget::Message { message_id } => json_text(&format!("message:{message_id}")),
        }),
        ["mcp_server", "on"] => Some(store.mcp_server_on().unwrap_or(false).to_string()),
        ["mcp_grant", app] => store.mcp_grant_of(app).ok().flatten().map(|b| b.to_string()),
        ["mcp_request", "pending_count"] => store.mcp_approvals_pending().ok().map(|n| n.to_string()),
        ["mcp_request", id, "state"] => store.mcp_approval(id).ok().flatten().map(|r| json_text(&r.state)),
        // the engine runs no outside app's search, so an approved request holds no results
        ["mcp_request", id, "results"] => store.mcp_approval(id).ok().flatten().map(|_| "[]".to_string()),
        ["suggestion", "count"] => store.suggestions().ok().map(|v| v.len().to_string()),
        ["suggestion", "last"] => store.suggestion_last_id().ok().flatten().map(|id| json_text(&id)),
        ["suggestion", id, field] => {
            let s = store.suggestion_row(id).ok().flatten()?;
            match *field {
                "kind" => Some(json_text(kind_word(s.kind))),
                "subject" => Some(json_text(&s.subject)),
                "accepted" => Some(s.accepted.to_string()),
                "label_id" => s.label_id.as_deref().map(json_text),
                "offered_at_ms" => Some(s.offered_at_ms.to_string()),
                "words" => Some(json_text(&suggestion_words(s.kind, s.subject.clone()))),
                _ => None,
            }
        }
        ["block", "count"] => store.blocks().ok().map(|v| v.len().to_string()),
        ["block", "list"] => store.blocks().ok().map(|v| {
            // oldest first, as the test engine lists them
            let words: Vec<String> = v.into_iter().rev().map(|b| match b.target { BlockTarget::Sender { address } => format!("sender:{address}"), BlockTarget::Domain { domain } => format!("domain:{domain}") }).collect();
            json_list(&words)
        }),
        ["block", address, "blocked"] => store.blocks().ok().map(|v| is_blocked(v, address.to_string()).to_string()),
        ["contact", "count"] => store.contact_cards().ok().map(|v| v.len().to_string()),
        ["contact", id, "addresses"] => store.contact_cards().ok()?.into_iter().find(|c| c.0 == *id).map(|c| json_list(&c.2)),
        ["contacts", "count"] => Some(people().to_string()),
        _ => None,
    }
}
