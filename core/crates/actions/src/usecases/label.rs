//! Label one or more messages; a name that does not exist yet is created when the caller says so.
use crate::engine::{core_first, groups_by_account, load, revert_fn, Applied, Env, MsgRow, Step};
use crate::port::WriteBack;
use contracts::{CommandOutcome, CoreError, UndoAction};
use std::collections::HashSet;
use store::{StoreError, WriteCtx};

/// The words of a refused label write, as the command's answer. Anything else stays a store error.
fn refusal(e: StoreError) -> Result<CoreError, StoreError> {
    match e {
        StoreError::Invalid(words) => Ok(CoreError::Invalid { field: "label_name".into(), reason_words: words }),
        other => Err(other),
    }
}

/// Add the label to the messages that do not carry it. `create` makes the label (and any missing level of its path)
/// when the name is new. A name with " > " in it is a path: "Clients > Provanta" is Provanta under Clients.
pub(crate) fn assign(ctx: &mut WriteCtx<'_>, rows: &[MsgRow], name: &str, create: bool, now: i64, with_undo: bool) -> Step {
    let name = name.trim();
    if name.is_empty() {
        return Ok(Err(CoreError::Invalid { field: "label_name".into(), reason_words: "a label needs a name".into() }));
    }
    let found = ctx.label_find_by_path(name)?;
    if found.is_none() && !create {
        return Ok(Err(CoreError::NotFound { kind: "label".into(), id: name.into() }));
    }
    let mut accounts: Vec<String> = Vec::new();
    for r in rows {
        if !accounts.contains(&r.account_id) {
            accounts.push(r.account_id.clone());
        }
    }
    let (label_id, label_path, made) = match found {
        Some(f) => {
            ctx.label_add_accounts(&f.id, &accounts, now)?;
            (f.id, f.path, Vec::new())
        }
        None => match ctx.label_ensure_path(name, &accounts, now) {
            Ok(e) => (e.label.id, e.label.path, e.created),
            Err(e) => return Ok(Err(refusal(e)?)),
        },
    };
    let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    let added: HashSet<String> = ctx.label_assign(&label_id, &ids, now)?.into_iter().collect();
    let fresh: Vec<MsgRow> = rows.iter().filter(|r| added.contains(&r.id)).cloned().collect();
    if fresh.is_empty() {
        return Ok(Ok(Applied::nothing()));
    }
    let ids: Vec<String> = fresh.iter().map(|r| r.id.clone()).collect();
    let set: HashSet<String> = ids.iter().cloned().collect();
    let lid = label_id.clone();
    let created_leaf = made.last().cloned();
    Ok(Ok(Applied {
        undo: with_undo.then(|| UndoAction::Label { message_ids: ids.clone(), label_id: label_id.clone() }),
        groups: groups_by_account(&fresh, |a, m| WriteBack::Label { account_id: a.into(), messages: m, label_name: label_path.clone(), applied: true }),
        revert: revert_fn(move |ctx, ids| {
            let back: Vec<String> = ids.iter().filter(|i| set.contains(*i)).cloned().collect();
            if back.is_empty() {
                return Ok(());
            }
            ctx.label_unassign(&lid, &back)?;
            ctx.label_delete_unused(&made)?;
            Ok(())
        }),
        created_id: created_leaf,
    }))
}

/// Take the label (by id) off the messages that carry it. Z on a label, unstar, unmute.
pub(crate) fn unassign(ctx: &mut WriteCtx<'_>, rows: &[MsgRow], label_id: &str, now: i64) -> Step {
    let Some(label) = ctx.label_get(label_id)? else {
        return Ok(Ok(Applied::nothing()));
    };
    let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    let removed: HashSet<String> = ctx.label_unassign(label_id, &ids)?.into_iter().collect();
    let carrying: Vec<MsgRow> = rows.iter().filter(|r| removed.contains(&r.id)).cloned().collect();
    if carrying.is_empty() {
        return Ok(Ok(Applied::nothing()));
    }
    let label_path = label.path;
    let set: HashSet<String> = removed;
    let lid = label_id.to_string();
    Ok(Ok(Applied {
        undo: None,
        groups: groups_by_account(&carrying, |a, m| WriteBack::Label { account_id: a.into(), messages: m, label_name: label_path.clone(), applied: false }),
        revert: revert_fn(move |ctx, ids| {
            let back: Vec<String> = ids.iter().filter(|i| set.contains(*i)).cloned().collect();
            if back.is_empty() {
                return Ok(());
            }
            ctx.label_assign(&lid, &back, now)?;
            Ok(())
        }),
        created_id: None,
    }))
}

/// Z on a label: the message ids come from the undo entry.
pub(crate) fn unassign_ids(ctx: &mut WriteCtx<'_>, ids: &[String], label_id: &str, now: i64) -> Step {
    let rows = match load(ctx, ids)? {
        Ok(r) => r,
        Err(e) => return Ok(Err(e)),
    };
    unassign(ctx, &rows, label_id, now)
}

/// Put a label (by id) back on messages (Z after RemoveLabel, contracts delta 10). An unknown label is NotFound.
pub(crate) fn reassign_ids(ctx: &mut WriteCtx<'_>, ids: &[String], label_id: &str, now: i64) -> Step {
    let Some(l) = ctx.label_get(label_id)? else { return Ok(Err(CoreError::NotFound { kind: "label".into(), id: label_id.into() })) };
    let rows = match load(ctx, ids)? {
        Ok(r) => r,
        Err(e) => return Ok(Err(e)),
    };
    assign(ctx, &rows, &l.path, false, now, false)
}

/// Take a label off by name; a name that does not exist is a no-op.
pub(crate) fn unassign_named(ctx: &mut WriteCtx<'_>, rows: &[MsgRow], name: &str, now: i64) -> Step {
    match ctx.label_find_by_path(name.trim())? {
        Some(f) => unassign(ctx, rows, &f.id, now),
        None => Ok(Ok(Applied::nothing())),
    }
}

pub struct Label {
    env: Env,
}

impl Label {
    pub fn new(env: Env) -> Label {
        Label { env }
    }

    pub fn execute(&self, message_ids: Vec<String>, label_name: String, create_if_missing: bool) -> Result<CommandOutcome, CoreError> {
        core_first(&self.env, move |ctx, now| {
            let rows = match load(ctx, &message_ids)? {
                Ok(r) => r,
                Err(e) => return Ok(Err(e)),
            };
            assign(ctx, &rows, &label_name, create_if_missing, now, true)
        })
    }
}
