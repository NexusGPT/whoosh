//! The undo stack: last in, first out, UNDO_DEPTH deep (docs/CONTROL-SOCKET.md "## Undo").
use crate::constants::undo_depth;
use crate::engine::{json_str, json_strs, new_id, optional};
use contracts::UndoAction;
use rusqlite::params;
use std::collections::HashSet;
use store::{StoreError, WriteCtx};

pub(crate) fn encode(action: &UndoAction) -> String {
    match action {
        UndoAction::Archive { message_ids } => format!("{{\"kind\":\"archive\",\"ids\":{}}}", json_strs(message_ids)),
        UndoAction::Snooze { message_ids } => format!("{{\"kind\":\"snooze\",\"ids\":{}}}", json_strs(message_ids)),
        UndoAction::Pin { message_id, was_pinned } => format!("{{\"kind\":\"pin\",\"id\":{},\"was_pinned\":{was_pinned}}}", json_str(message_id)),
        UndoAction::Label { message_ids, label_id } => format!("{{\"kind\":\"label\",\"ids\":{},\"label_id\":{}}}", json_strs(message_ids), json_str(label_id)),
        UndoAction::Move { message_ids, from_account_id } => format!("{{\"kind\":\"move\",\"ids\":{},\"from\":{}}}", json_strs(message_ids), json_str(from_account_id)),
        UndoAction::Send { outbox_id } => format!("{{\"kind\":\"send\",\"outbox_id\":{}}}", json_str(outbox_id)),
        // delta 7: label and delete undo entries are written by the label and message owners, not by this crate; the
        // arms keep the match exhaustive and the stored form readable (`latest` still refuses a kind it cannot undo)
        UndoAction::RenameLabel { label_id, previous_name } => format!("{{\"kind\":\"rename_label\",\"id\":{},\"previous_name\":{}}}", json_str(label_id), json_str(previous_name)),
        UndoAction::MoveLabel { label_id, previous_parent_id } => format!("{{\"kind\":\"move_label\",\"id\":{},\"previous_parent\":{}}}", json_str(label_id), previous_parent_id.as_deref().map(json_str).unwrap_or_else(|| "null".to_string())),
        UndoAction::CreateLabel { label_id } => format!("{{\"kind\":\"create_label\",\"id\":{}}}", json_str(label_id)),
        UndoAction::DeleteMessage { message_id } => format!("{{\"kind\":\"delete_message\",\"id\":{}}}", json_str(message_id)),
        // delta 8: note undo entries are written by the notes owner; same stance as the label arms above
        UndoAction::NoteAdd { note_id } => format!("{{\"kind\":\"note_add\",\"id\":{}}}", json_str(note_id)),
        UndoAction::NoteEdit { note_id, previous_text } => format!("{{\"kind\":\"note_edit\",\"id\":{},\"previous_text\":{}}}", json_str(note_id), json_str(previous_text)),
        UndoAction::NoteDelete { note } => format!("{{\"kind\":\"note_delete\",\"id\":{}}}", json_str(&note.id)),
        // contracts delta 10: mechanical arms; this crate does not write these kinds yet
        UndoAction::DeleteMessages { message_ids, origins } => {
            let o: Vec<String> = origins.iter().map(|o| format!("{{\"id\":{},\"folder\":{},\"archived\":{}}}", json_str(&o.message_id), json_str(&o.folder), o.archived)).collect();
            format!("{{\"kind\":\"delete_messages\",\"ids\":{},\"origins\":[{}]}}", json_strs(message_ids), o.join(","))
        }
        UndoAction::Unlabel { message_ids, label_id } => format!("{{\"kind\":\"unlabel\",\"ids\":{},\"label_id\":{}}}", json_strs(message_ids), json_str(label_id)),
        UndoAction::UnlabelAll { removed } => {
            // the whole list is kept: Z puts each assignment back (reds-labelcmds; it kept only a count, which Z could not read)
            let r: Vec<String> = removed.iter().map(|a| format!("{{\"label\":{},\"message\":{},\"decided\":{}}}", json_str(&a.label_id), json_str(&a.message_id), a.decided_entry_id.as_deref().map(json_str).unwrap_or_else(|| "null".to_string()))).collect();
            format!("{{\"kind\":\"unlabel_all\",\"count\":{},\"removed\":[{}]}}", removed.len(), r.join(","))
        }
        UndoAction::DeleteDraft { draft, .. } => format!("{{\"kind\":\"delete_draft\",\"id\":{}}}", json_str(&draft.id)), // mechanical (contracts-d11): drafts are the outbox/draft owners'
        UndoAction::Mute { message_ids, was_muted } => format!("{{\"kind\":\"mute\",\"ids\":{},\"was_muted\":{}}}", json_strs(message_ids), json_strs(was_muted)),
        UndoAction::Unarchive { message_ids } => format!("{{\"kind\":\"unarchive\",\"ids\":{}}}", json_strs(message_ids)),
        UndoAction::MoveToLabel { label_id, labelled, archived } => format!("{{\"kind\":\"move_to_label\",\"label_id\":{},\"labelled\":{},\"archived\":{}}}", json_str(label_id), json_strs(labelled), json_strs(archived)),
    }
}

/// Add an entry and drop the unconsumed ones beyond UNDO_DEPTH, oldest first. Returns the entry id.
pub(crate) fn push(ctx: &mut WriteCtx<'_>, action: &UndoAction, now: i64) -> Result<String, StoreError> {
    let id = new_id("undo");
    ctx.tx.execute("INSERT INTO undo_entry (id, action_json, at_ms, consumed) VALUES (?1, ?2, ?3, 0)", params![id, encode(action), now])?;
    ctx.tx.execute(
        "DELETE FROM undo_entry WHERE consumed = 0 AND id NOT IN (SELECT id FROM undo_entry WHERE consumed = 0 ORDER BY at_ms DESC, rowid DESC LIMIT ?1)",
        params![undo_depth() as i64],
    )?;
    Ok(id)
}

fn list_of(ctx: &WriteCtx<'_>, entry: &str, path: &str) -> Result<Vec<String>, StoreError> {
    let mut st = ctx.tx.prepare_cached("SELECT value FROM json_each((SELECT action_json FROM undo_entry WHERE id = ?1), ?2)")?;
    let rows = st.query_map(rusqlite::params![entry, path], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// The origins a delete recorded (contracts-d11); an entry written before them has none.
fn origins_of(ctx: &WriteCtx<'_>, entry: &str) -> Result<Vec<contracts::MessageOrigin>, StoreError> {
    let mut st = ctx.tx.prepare_cached(
        "SELECT json_extract(value,'$.id'), json_extract(value,'$.folder'), json_extract(value,'$.archived') \
         FROM json_each((SELECT action_json FROM undo_entry WHERE id = ?1), '$.origins')",
    )?;
    let rows = st.query_map([entry], |r| Ok(contracts::MessageOrigin { message_id: r.get(0)?, folder: r.get(1)?, archived: r.get::<_, i64>(2)? != 0 }))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// The assignments a RemoveAllLabels recorded (reds-labelcmds).
fn removed_of(ctx: &WriteCtx<'_>, entry: &str) -> Result<Vec<contracts::LabelAssignment>, StoreError> {
    let mut st = ctx.tx.prepare_cached(
        "SELECT json_extract(value,'$.label'), json_extract(value,'$.message'), json_extract(value,'$.decided') \
         FROM json_each((SELECT action_json FROM undo_entry WHERE id = ?1), '$.removed')",
    )?;
    let rows = st.query_map([entry], |r| Ok(contracts::LabelAssignment { label_id: r.get(0)?, message_id: r.get(1)?, decided_entry_id: r.get(2)? }))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// One text field of an entry's JSON, None when it is missing or null (fix-labelstreams: the label record kinds' extra fields).
fn text_at(ctx: &WriteCtx<'_>, entry: &str, path: &str) -> Result<Option<String>, StoreError> {
    Ok(optional(ctx.tx.query_row("SELECT json_extract(action_json, ?2) FROM undo_entry WHERE id = ?1", params![entry, path], |r| r.get::<_, Option<String>>(0)))?.flatten())
}

/// One whole-number field of an entry's JSON, None when it is missing or null.
pub(crate) fn int_at(ctx: &WriteCtx<'_>, entry: &str, path: &str) -> Result<Option<i64>, StoreError> {
    Ok(optional(ctx.tx.query_row("SELECT json_extract(action_json, ?2) FROM undo_entry WHERE id = ?1", params![entry, path], |r| r.get::<_, Option<i64>>(0)))?.flatten())
}

/// Adds a field to a stored entry, beside what the contract's record holds (`json` is valid JSON). The label kinds keep there what Z needs and
/// the record has no room for: a moved label's place among its siblings (`previous_order`), the deciders of what RemoveAllLabels took (`deciders`).
pub(crate) fn set_extra(ctx: &WriteCtx<'_>, entry: &str, key: &str, json: &str) -> Result<(), StoreError> {
    ctx.tx.execute("UPDATE undo_entry SET action_json = json_set(action_json, ?2, json(?3)) WHERE id = ?1", params![entry, format!("$.{key}"), json])?;
    Ok(())
}

/// Z after RemoveAllLabels: puts each restored filing's decider back from the entry's `deciders` (who decided, the rule, the probability, when,
/// the flow card and chain, the page entry). An entry written without them leaves the filings as `labels_restore` made them (the owner's).
pub(crate) fn restore_deciders(ctx: &WriteCtx<'_>, entry: &str) -> Result<usize, StoreError> {
    type Row = (String, String, String, Option<String>, Option<f64>, Option<i64>, Option<String>, Option<i64>, Option<String>);
    let rows: Vec<Row> = {
        let mut st = ctx.tx.prepare_cached(
            "SELECT json_extract(value,'$.label'), json_extract(value,'$.message'), json_extract(value,'$.by'), json_extract(value,'$.rule'), \
                    json_extract(value,'$.prob'), json_extract(value,'$.at'), json_extract(value,'$.card'), json_extract(value,'$.chain'), json_extract(value,'$.entry') \
             FROM json_each((SELECT action_json FROM undo_entry WHERE id = ?1), '$.deciders')",
        )?;
        let found = st.query_map([entry], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?)))?;
        found.collect::<Result<Vec<Row>, _>>()?
    };
    let mut up = ctx.tx.prepare_cached(
        "UPDATE label_assignment SET decided_by = ?3, rule_id = ?4, probability = ?5, decided_at_ms = ?6, flow_card = ?7, flow_chain_id = ?8, decided_entry_id = ?9 \
         WHERE label_id = ?1 AND message_id = ?2",
    )?;
    let mut n = 0;
    for r in &rows {
        n += up.execute(params![r.0, r.1, r.2, r.3, r.4, r.5, r.6, r.7, r.8])?;
    }
    Ok(n)
}

fn ids_of(ctx: &WriteCtx<'_>, entry: &str) -> Result<Vec<String>, StoreError> {
    let mut st = ctx.tx.prepare_cached("SELECT value FROM json_each((SELECT action_json FROM undo_entry WHERE id = ?1), '$.ids')")?;
    let rows = st.query_map([entry], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// The newest entry not yet undone.
pub(crate) fn latest(ctx: &WriteCtx<'_>) -> Result<Option<(String, UndoAction)>, StoreError> {
    let head = optional(ctx.tx.query_row(
        "SELECT id, json_extract(action_json,'$.kind'), json_extract(action_json,'$.id'), json_extract(action_json,'$.was_pinned'), \
                json_extract(action_json,'$.label_id'), json_extract(action_json,'$.from'), json_extract(action_json,'$.outbox_id') \
         FROM undo_entry WHERE consumed = 0 ORDER BY at_ms DESC, rowid DESC LIMIT 1",
        [],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<i64>>(3)?, r.get::<_, Option<String>>(4)?, r.get::<_, Option<String>>(5)?, r.get::<_, Option<String>>(6)?)),
    ))?;
    let Some((id, kind, one, was_pinned, label, from, outbox)) = head else { return Ok(None) };
    let bad = |what: &str| StoreError::Invalid(format!("undo entry {id} ({kind}) has no {what}"));
    let action = match kind.as_str() {
        "archive" => UndoAction::Archive { message_ids: ids_of(ctx, &id)? },
        "snooze" => UndoAction::Snooze { message_ids: ids_of(ctx, &id)? },
        "pin" => UndoAction::Pin { message_id: one.ok_or_else(|| bad("message id"))?, was_pinned: was_pinned.unwrap_or(0) != 0 },
        "label" => UndoAction::Label { message_ids: ids_of(ctx, &id)?, label_id: label.ok_or_else(|| bad("label id"))? },
        "move" => UndoAction::Move { message_ids: ids_of(ctx, &id)?, from_account_id: from.ok_or_else(|| bad("source account"))? },
        "send" => UndoAction::Send { outbox_id: outbox.ok_or_else(|| bad("outbox id"))? },
        "delete_messages" => UndoAction::DeleteMessages { message_ids: ids_of(ctx, &id)?, origins: origins_of(ctx, &id)? },
        "mute" => UndoAction::Mute { message_ids: ids_of(ctx, &id)?, was_muted: list_of(ctx, &id, "$.was_muted")? },
        "unarchive" => UndoAction::Unarchive { message_ids: ids_of(ctx, &id)? },
        "move_to_label" => UndoAction::MoveToLabel { label_id: label.ok_or_else(|| bad("label id"))?, labelled: list_of(ctx, &id, "$.labelled")?, archived: list_of(ctx, &id, "$.archived")? },
        "unlabel" => UndoAction::Unlabel { message_ids: ids_of(ctx, &id)?, label_id: label.ok_or_else(|| bad("label id"))? },
        "unlabel_all" => UndoAction::UnlabelAll { removed: removed_of(ctx, &id)? },
        // fix-labelstreams: the label record kinds (the engine's label_cmds.rs pushes them); encode wrote id, previous_name, previous_parent
        // a deleted draft: the entry holds the whole row (usecases/delete_draft.rs); the record here carries what a reader of the stack needs
        "delete_draft" => UndoAction::DeleteDraft {
            draft: contracts::Draft { id: one.ok_or_else(|| bad("draft id"))?, from_account_id: from.unwrap_or_default(), to: list_of(ctx, &id, "$.to")?, cc: list_of(ctx, &id, "$.cc")?,
                                      subject: text_at(ctx, &id, "$.subject")?.unwrap_or_default(), body: text_at(ctx, &id, "$.body")?.unwrap_or_default(), attachment_ids: Vec::new(),
                                      ai_marks: Vec::new(), reply_to_thread_id: text_at(ctx, &id, "$.thread")?, bcc: list_of(ctx, &id, "$.bcc")? },
            saved_at: int_at(ctx, &id, "$.saved_at")?.unwrap_or(0),
        },
        "create_label" => UndoAction::CreateLabel { label_id: one.ok_or_else(|| bad("label id"))? },
        "rename_label" => UndoAction::RenameLabel { label_id: one.ok_or_else(|| bad("label id"))?, previous_name: text_at(ctx, &id, "$.previous_name")?.ok_or_else(|| bad("previous name"))? },
        "move_label" => UndoAction::MoveLabel { label_id: one.ok_or_else(|| bad("label id"))?, previous_parent_id: text_at(ctx, &id, "$.previous_parent")? },
        other => return Err(StoreError::Invalid(format!("undo entry {id} has an unknown kind {other:?}"))),
    };
    Ok(Some((id, action)))
}

pub(crate) fn consume(ctx: &WriteCtx<'_>, entry: &str, consumed: bool) -> Result<(), StoreError> {
    ctx.tx.execute("UPDATE undo_entry SET consumed = ?1 WHERE id = ?2", params![consumed as i64, entry])?;
    Ok(())
}

/// After a refusal: keep in the entry only the messages that stayed changed; no message left, no entry.
pub(crate) fn restrict(ctx: &WriteCtx<'_>, entry: &str, action: &UndoAction, kept: &HashSet<String>) -> Result<(), StoreError> {
    let keep = |ids: &[String]| -> Vec<String> { ids.iter().filter(|i| kept.contains(*i)).cloned().collect() };
    let narrowed = match action {
        UndoAction::Archive { message_ids } => Some(UndoAction::Archive { message_ids: keep(message_ids) }),
        UndoAction::Snooze { message_ids } => Some(UndoAction::Snooze { message_ids: keep(message_ids) }),
        UndoAction::Label { message_ids, label_id } => Some(UndoAction::Label { message_ids: keep(message_ids), label_id: label_id.clone() }),
        UndoAction::Move { message_ids, from_account_id } => Some(UndoAction::Move { message_ids: keep(message_ids), from_account_id: from_account_id.clone() }),
        UndoAction::Mute { message_ids, was_muted } => Some(UndoAction::Mute { message_ids: keep(message_ids), was_muted: keep(was_muted) }),
        UndoAction::Unarchive { message_ids } => Some(UndoAction::Unarchive { message_ids: keep(message_ids) }),
        UndoAction::Pin { message_id, .. } => kept.contains(message_id).then(|| action.clone()),
        UndoAction::Send { .. } | UndoAction::RenameLabel { .. } | UndoAction::MoveLabel { .. } | UndoAction::CreateLabel { .. } | UndoAction::DeleteMessage { .. } | UndoAction::NoteAdd { .. } | UndoAction::NoteEdit { .. } | UndoAction::NoteDelete { .. } | UndoAction::Unlabel { .. } | UndoAction::UnlabelAll { .. } | UndoAction::DeleteDraft { .. } => Some(action.clone()),
        UndoAction::DeleteMessages { message_ids, origins } => Some(UndoAction::DeleteMessages {
            message_ids: keep(message_ids),
            origins: origins.iter().filter(|o| kept.contains(&o.message_id)).cloned().collect(),
        }),
        UndoAction::MoveToLabel { label_id, labelled, archived } => Some(UndoAction::MoveToLabel { label_id: label_id.clone(), labelled: keep(labelled), archived: keep(archived) }),
    };
    let empty = match &narrowed {
        None => true,
        Some(UndoAction::Archive { message_ids }) | Some(UndoAction::Snooze { message_ids }) | Some(UndoAction::Unarchive { message_ids }) | Some(UndoAction::Mute { message_ids, .. }) => message_ids.is_empty(),
        Some(UndoAction::Label { message_ids, .. }) | Some(UndoAction::Move { message_ids, .. }) => message_ids.is_empty(),
        Some(_) => false,
    };
    match narrowed {
        Some(a) if !empty => {
            ctx.tx.execute("UPDATE undo_entry SET action_json = ?1 WHERE id = ?2", params![encode(&a), entry])?;
        }
        _ => {
            ctx.tx.execute("DELETE FROM undo_entry WHERE id = ?1", params![entry])?;
        }
    }
    Ok(())
}
