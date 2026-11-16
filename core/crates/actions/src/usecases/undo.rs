//! Z: reverse the newest action not yet undone, then the one before it, up to UNDO_DEPTH entries.
//! The reversal is a use case like any other: it commits in the store first and writes back to the server.
use super::{archive, label, move_to_account, pin, snooze, trash};
use crate::engine::{core_first, groups_by_account, load, revert_fn, Applied, Env, Step};
use crate::port::WriteBack;
use crate::undo;
use contracts::{CommandOutcome, CoreError, UndoAction};
use store::{StoreError, WriteCtx};

pub struct Undo {
    env: Env,
}

fn nothing_to_undo() -> CoreError {
    CoreError::Invalid { field: "undo".into(), reason_words: "Nothing to undo".into() }
}

impl Undo {
    pub fn new(env: Env) -> Undo {
        Undo { env }
    }

    pub fn execute(&self) -> Result<CommandOutcome, CoreError> {
        let newest = self.env.store.write(|ctx| undo::latest(ctx))?;
        if let Some((entry, UndoAction::Send { outbox_id })) = newest {
            return self.undo_send(entry, outbox_id);
        }
        core_first(&self.env, move |ctx, now| {
            let Some((entry, action)) = undo::latest(ctx)? else { return Ok(Err(nothing_to_undo())) };
            let inverse = match &action {
                UndoAction::Archive { message_ids } => archive::apply(ctx, message_ids, false, false)?,
                UndoAction::Snooze { message_ids } => snooze::apply(ctx, message_ids, None, now, false)?,
                UndoAction::Pin { message_id, was_pinned } => pin::apply(ctx, message_id, *was_pinned, now, false)?,
                UndoAction::Label { message_ids, label_id } => label::unassign_ids(ctx, message_ids, label_id, now)?,
                UndoAction::Move { message_ids, from_account_id } => move_to_account::apply(ctx, message_ids, from_account_id, false)?,
                UndoAction::Send { .. } => Err(CoreError::Invalid { field: "undo".into(), reason_words: "a send can only be undone while its window is open".into() }),
                // a deleted batch comes back to the folder each message came from, with its archived mark (contracts-d11);
                // an entry written before origins were recorded falls back to the inbox folder
                UndoAction::DeleteMessages { message_ids, origins } => trash::restore_origins(ctx, message_ids, origins)?,
                UndoAction::MoveToLabel { label_id, labelled, archived } => match label::unassign_ids(ctx, labelled, label_id, now)? {
                    Ok(a) => match archive::apply(ctx, archived, false, false)? {
                        Ok(b) => Ok(crate::engine::Applied::merge(a, b)),
                        Err(e) => Err(e),
                    },
                    Err(e) => Err(e),
                },
                // delta 19: Z after Mute puts each message's earlier flag back; Z after Unarchive archives the messages again
                // (the flag, and the server's \Archived or Archive-folder move for a message that still has its UID)
                UndoAction::Mute { message_ids, was_muted } => super::mute::reverse(ctx, message_ids, was_muted)?,
                UndoAction::Unarchive { message_ids } => archive::apply(ctx, message_ids, true, false)?,
                // the label owners' kinds: Z restores what the removal took (contracts delta 10)
                UndoAction::Unlabel { message_ids, label_id } => label::reassign_ids(ctx, message_ids, label_id, now)?,
                UndoAction::UnlabelAll { removed } => {
                    ctx.labels_restore(removed, now)?;
                    // fix-labelstreams: each filing gets its decider back (who decided, rule, probability, flow card, page entry)
                    undo::restore_deciders(ctx, &entry)?;
                    Ok(crate::engine::Applied::nothing())
                }
                // fix-labelstreams: Z after CreateLabel, RenameLabel, MoveLabel (the engine's label_cmds.rs pushes them)
                UndoAction::CreateLabel { label_id } => create_back(ctx, label_id)?,
                UndoAction::RenameLabel { label_id, previous_name } => rename_back(ctx, label_id, previous_name, now)?,
                UndoAction::MoveLabel { label_id, previous_parent_id } => move_back(ctx, &entry, label_id, previous_parent_id)?,
                // Z after a deleted draft: the row comes back whole from the entry (usecases/delete_draft.rs)
                UndoAction::DeleteDraft { draft, .. } => {
                    if super::delete_draft::restore(ctx, &entry)? { Ok(crate::engine::Applied::nothing()) } else { Err(CoreError::NotFound { kind: "draft".into(), id: draft.id.clone() }) }
                }
                UndoAction::DeleteMessage { .. } | UndoAction::NoteAdd { .. } | UndoAction::NoteEdit { .. } | UndoAction::NoteDelete { .. } => {
                    Err(CoreError::Invalid { field: "undo".into(), reason_words: "this action is undone by the label and message owners, not by the mail actions".into() })
                }
            };
            match inverse {
                Err(e) => {
                    // The messages are gone (emptied or deleted): the entry can never apply, so it must not block the stack.
                    // A label record entry (create, rename, move) also goes when the tree no longer allows it (the old name taken, the
                    // old parent gone or full): it would otherwise sit at the top and refuse every later Z.
                    let label_record = matches!(action, UndoAction::CreateLabel { .. } | UndoAction::RenameLabel { .. } | UndoAction::MoveLabel { .. });
                    if matches!(e, CoreError::NotFound { .. }) || (label_record && matches!(e, CoreError::Invalid { .. })) {
                        undo::consume(ctx, &entry, true)?;
                    }
                    Ok(Err(e))
                }
                Ok(mut applied) => {
                    undo::consume(ctx, &entry, true)?;
                    let (inner, id) = (applied.revert.clone(), entry);
                    applied.revert = revert_fn(move |ctx, ids| {
                        inner(ctx, ids)?;
                        undo::consume(ctx, &id, false)
                    });
                    Ok(Ok(applied))
                }
            }
        })
    }

    fn undo_send(&self, entry: String, outbox_id: String) -> Result<CommandOutcome, CoreError> {
        let Some(sends) = &self.env.sends else {
            return Err(CoreError::Invalid { field: "undo".into(), reason_words: "the outbox is not connected to undo".into() });
        };
        sends.execute(&outbox_id).map_err(|r| self.env.failure(&r))?;
        self.env.store.write(move |ctx| undo::consume(ctx, &entry, true))?;
        Ok(CommandOutcome { undo_entry_id: None, created_id: None })
    }
}

fn label_missing(id: &str) -> CoreError {
    CoreError::NotFound { kind: "label".into(), id: id.into() }
}

/// A refusal the store wrote in words stays those words; anything else is the store's trouble.
fn in_words(e: StoreError) -> Result<CoreError, StoreError> {
    match e {
        StoreError::Invalid(words) => Ok(CoreError::Invalid { field: "label".into(), reason_words: words }),
        other => Err(other),
    }
}

/// Z after CreateLabel: the label goes with its sub-labels and every filing under them (contracts UndoAction::CreateLabel, as the test engine's
/// `reverse`). A message that still carried one of them is told to the server, as DeleteLabel tells it.
fn create_back(ctx: &mut WriteCtx<'_>, label_id: &str) -> Step {
    if ctx.label_get(label_id)?.is_none() {
        return Ok(Err(label_missing(label_id)));
    }
    let subtree: Vec<(String, String)> = {
        let mut st = ctx.tx.prepare_cached(
            "SELECT id, path FROM label WHERE id = ?1 OR (path >= (SELECT path FROM label WHERE id = ?1) || ' > ' AND path < (SELECT path FROM label WHERE id = ?1) || ' ?') ORDER BY depth DESC",
        )?;
        let rows = st.query_map([label_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    let mut groups = Vec::new();
    for (id, path) in &subtree {
        let ids: Vec<String> = {
            let mut st = ctx.tx.prepare_cached("SELECT message_id FROM label_assignment WHERE label_id = ?1 ORDER BY message_id")?;
            let rows = st.query_map([id], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        if ids.is_empty() {
            continue;
        }
        let rows = match load(ctx, &ids)? {
            Ok(r) => r,
            Err(e) => return Ok(Err(e)),
        };
        let name = path.clone();
        groups.extend(groups_by_account(&rows, |a, m| WriteBack::Label { account_id: a.into(), messages: m, label_name: name.clone(), applied: false }));
    }
    match ctx.label_delete_tree(label_id) {
        Ok(_) => Ok(Ok(Applied { groups, ..Applied::nothing() })),
        Err(e) => Ok(Err(in_words(e)?)),
    }
}

/// Z after RenameLabel: the label takes its earlier name back (the paths below follow, in the store).
fn rename_back(ctx: &mut WriteCtx<'_>, label_id: &str, previous_name: &str, now: i64) -> Step {
    if ctx.label_get(label_id)?.is_none() {
        return Ok(Err(label_missing(label_id)));
    }
    // the database does not refuse a second sibling of the same name (the engine checks it before it renames): so does Z
    let taken: bool = ctx.tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM label o WHERE o.id <> ?1 AND o.parent_id IS (SELECT parent_id FROM label WHERE id = ?1) AND lower(o.name) = lower(?2))",
        rusqlite::params![label_id, previous_name],
        |r| r.get(0),
    )?;
    if taken {
        return Ok(Err(CoreError::Invalid { field: "name".into(), reason_words: format!("\"{previous_name}\" exists already here") }));
    }
    match ctx.label_rename(label_id, previous_name, now) {
        Ok(_) => Ok(Ok(Applied::nothing())),
        Err(e) => Ok(Err(in_words(e)?)),
    }
}

/// Z after MoveLabel: the label goes back under its earlier parent (None = the top level), at the place it had among that parent's children
/// (the entry's `previous_order`; an entry without it puts the label last, as the test engine's `moveLabel` does).
fn move_back(ctx: &mut WriteCtx<'_>, entry: &str, label_id: &str, previous_parent_id: &Option<String>) -> Step {
    if ctx.label_get(label_id)?.is_none() {
        return Ok(Err(label_missing(label_id)));
    }
    if let Some(p) = previous_parent_id {
        if ctx.label_get(p)?.is_none() {
            return Ok(Err(label_missing(p)));
        }
    }
    let place = undo::int_at(ctx, entry, "$.previous_order")?.map(|n| n.clamp(0, u32::MAX as i64) as u32).unwrap_or(u32::MAX);
    match ctx.label_reorder(label_id, previous_parent_id.clone(), place) {
        Ok(_) => Ok(Ok(Applied::nothing())),
        Err(e) => Ok(Err(in_words(e)?)),
    }
}

/// Records one undo entry from outside this crate's use cases (delta 19): the real core's `Mute` and `Unarchive` (engine/src/mute.rs,
/// unarchive.rs) change the store themselves, in their own transaction, and push their entry in it. Returns the entry id.
pub fn push_entry(ctx: &mut store::WriteCtx<'_>, action: &UndoAction, now_ms: i64) -> Result<String, store::StoreError> {
    undo::push(ctx, action, now_ms)
}

/// `push_entry` with one extra field beside the contract's record (fix-labelstreams, label kinds only): `key` and `json` (valid JSON) are stored
/// in the same transaction, for what Z needs and `UndoAction` has no room for (a moved label's place, the deciders of removed filings).
pub fn push_entry_extra(ctx: &mut store::WriteCtx<'_>, action: &UndoAction, key: &str, json: &str, now_ms: i64) -> Result<String, store::StoreError> {
    let id = undo::push(ctx, action, now_ms)?;
    undo::set_extra(ctx, &id, key, json)?;
    Ok(id)
}
