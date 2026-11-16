//! real-labels (main86 wave): the label record commands in the real core, which answered "The local engine does not do this yet." to
//! them. PinLabel, CreateLabel, CreateLabelFull, UpdateLabel, RenameLabel, MoveLabel, ReorderLabel, DeleteLabel, RemoveLabel and
//! RemoveAllLabels are served from the store (`store/src/label_meta.rs`, `labels.rs`, `delta10.rs`), each in ONE store transaction on
//! this Mac. The test engine (Sources/CoreBridge/FixtureLabels.swift, FixtureCore.swift `createLabel`, `renameLabel`, `moveLabel`, the
//! `removeLabel` arms) is the specification: its refusals are kept in its words. A label leaving messages (RemoveLabel, RemoveAllLabels,
//! DeleteLabel) is told to the server through the write-back ledger in the same transaction (`writeback::Ledger`, kind "label",
//! applied false), the way the actions crate tells a label put on; the worker is woken by the caller (rust_core.rs `run`).
//! Undo: RemoveLabel and RemoveAllLabels push the entry the actions crate's Z reverses (Unlabel, UnlabelAll). CreateLabel, RenameLabel and
//! MoveLabel push theirs too (fix-labelstreams: the actions crate's Z now reverses CreateLabel, RenameLabel and MoveLabel), in the same
//! transaction as the change; a rename or a move that changes nothing pushes none, as in the test engine.
//! State paths (`state`): labels/tree, labels/pinned, labels/count, labels/<id>/rerun_count, label/<id>/<description|pinned|scope|kind|
//! order|colour|path|parent|name|depth|rule_sets>, label_assignment/<message>, label_removed/<message>/<label>, the fixture core's shapes
//! (keys sorted). label/<id>/rules and message/<id>/label/<label>/decided_by stay label_flow.rs's.
use std::collections::HashMap;

use actions::{MessageRef, WriteBack, WriteBackLedger};
use contracts::*;
use store::{NewLabel, Store, StoreError, WriteCtx};

use crate::rust_core::now_ms;
use crate::writeback::Ledger;

const OK: CommandOutcome = CommandOutcome { undo_entry_id: None, created_id: None };

/// The ten commands this file serves.
pub(crate) fn is_command(c: &Command) -> bool {
    matches!(c,
        Command::PinLabel { .. } | Command::CreateLabel { .. } | Command::CreateLabelFull { .. } | Command::UpdateLabel { .. }
        | Command::RenameLabel { .. } | Command::MoveLabel { .. } | Command::ReorderLabel { .. } | Command::DeleteLabel { .. }
        | Command::RemoveLabel { .. } | Command::RemoveAllLabels { .. })
}

/// Runs one of the ten. A view used as a label is refused first (`check_view_command`, code `view_not_a_label`), as in the fixture core.
pub(crate) fn run(store: &Store, command: Command) -> Result<CommandOutcome, CoreError> {
    check_view_command(command.clone())?;
    let now = now_ms();
    match command {
        Command::PinLabel { label_id, pinned } => {
            known(store, &label_id)?;
            store.label_pin(label_id, pinned, now).map_err(from_store)?;
            Ok(OK)
        }
        Command::CreateLabel { name, parent_id } => create(store, name, parent_id, now),
        Command::CreateLabelFull { spec } => create_full(store, spec, now),
        Command::UpdateLabel { label_id, spec } => update(store, label_id, spec, now),
        Command::RenameLabel { label_id, name } => rename(store, label_id, name, now),
        Command::MoveLabel { label_id, parent_id } => move_label(store, label_id, parent_id, now),
        Command::ReorderLabel { label_id, new_parent_id, new_index } => {
            known(store, &label_id)?;
            if let Some(p) = &new_parent_id { known(store, p)?; }
            store.label_reorder(label_id, new_parent_id, new_index).map_err(from_store)?;
            Ok(OK)
        }
        Command::DeleteLabel { label_id } => delete(store, label_id, now),
        Command::RemoveLabel { message_ids, label_id } => remove(store, message_ids, label_id, now),
        Command::RemoveAllLabels { message_ids } => remove_all(store, message_ids, now),
        _ => Err(CoreError::Invalid { field: "command".into(), reason_words: "not a label command".into() }),
    }
}

/// A refusal the store or its triggers wrote in words stays those words; anything else is the store's trouble.
fn from_store(e: StoreError) -> CoreError {
    match e {
        StoreError::Invalid(words) => CoreError::Invalid { field: "label".into(), reason_words: words },
        other => CoreError::Unexpected { reason_words: format!("the label change could not be stored: {other}") },
    }
}

fn invalid(field: &str, words: impl Into<String>) -> CoreError { CoreError::Invalid { field: field.into(), reason_words: words.into() } }
fn not_found(id: &str) -> CoreError { CoreError::NotFound { kind: "label".into(), id: id.into() } }

fn labels(store: &Store) -> Result<Vec<MailLabel>, CoreError> { store.labels_full().map_err(from_store) }
fn find(all: &[MailLabel], id: &str) -> Result<MailLabel, CoreError> { all.iter().find(|l| l.id == id).cloned().ok_or_else(|| not_found(id)) }
fn known(store: &Store, id: &str) -> Result<MailLabel, CoreError> { find(&labels(store)?, id) }

/// One name, not a path (the fixture core's `singleLabelName`).
fn one_name(text: &str) -> Result<String, CoreError> {
    let n = text.trim();
    if n.contains('>') { return Err(invalid("name", "a label name cannot hold \">\"; make the parent first")); }
    if n.is_empty() { return Err(invalid("name", "a label needs a name")); }
    Ok(n.to_string())
}

/// Whether a label other than `except` under `parent` already has this name (case ignored).
fn sibling_taken(all: &[MailLabel], parent: &Option<String>, name: &str, except: Option<&str>) -> bool {
    let n = name.to_lowercase();
    all.iter().any(|l| Some(l.id.as_str()) != except && l.parent_id == *parent && l.name.to_lowercase() == n)
}

fn depth_words(path: &str, depth: u64) -> String {
    format!("a label can be at most {} levels deep; \"{path}\" has {depth}", contracts::constants::LABEL_DEPTH_MAX)
}

fn scope_known(store: &Store, scope: &LabelScope) -> Result<(), CoreError> {
    if let LabelScope::Account { account_id } = scope {
        let accounts = store.account_summaries().map_err(from_store)?;
        if !accounts.iter().any(|a| a.id == *account_id) { return Err(CoreError::NotFound { kind: "account".into(), id: account_id.clone() }); }
    }
    Ok(())
}

fn create(store: &Store, name: String, parent_id: Option<String>, now: i64) -> Result<CommandOutcome, CoreError> {
    let n = one_name(&name)?;
    let all = labels(store)?;
    let parent = match &parent_id { Some(p) => Some(find(&all, p)?), None => None };
    let depth = parent.as_ref().map(|p| p.depth as u64).unwrap_or(0) + 1;
    if depth > contracts::constants::LABEL_DEPTH_MAX {
        let path = parent.as_ref().map(|p| format!("{} > {n}", p.path)).unwrap_or_else(|| n.clone());
        return Err(invalid("label", depth_words(&path, depth)));
    }
    if sibling_taken(&all, &parent_id, &n, None) { return Err(invalid("name", format!("\"{n}\" exists already here"))); }
    // the label and its undo entry in ONE transaction
    let (made, entry) = store
        .write(move |ctx| {
            let made = ctx.label_create(NewLabel { id: None, name: n, parent_id, colour: None, account_ids: vec![], now_ms: now })?;
            let entry = actions::usecases::undo::push_entry(ctx, &UndoAction::CreateLabel { label_id: made.id.clone() }, now)?;
            Ok((made, entry))
        })
        .map_err(from_store)?;
    Ok(CommandOutcome { undo_entry_id: Some(entry), created_id: Some(made.id) })
}

fn create_full(store: &Store, spec: LabelSpec, now: i64) -> Result<CommandOutcome, CoreError> {
    let all = labels(store)?;
    if let Some(p) = &spec.parent_id { find(&all, p)?; }
    let n = spec.name.trim().to_string();
    if !n.is_empty() && sibling_taken(&all, &spec.parent_id, &n, None) { return Err(invalid("name", format!("there is already a label named \"{n}\" there"))); }
    scope_known(store, &spec.scope)?;
    let made = store.label_create_full(spec, now).map_err(from_store)?;
    Ok(CommandOutcome { undo_entry_id: None, created_id: Some(made.id) })
}

fn update(store: &Store, label_id: String, spec: LabelSpec, now: i64) -> Result<CommandOutcome, CoreError> {
    let all = labels(store)?;
    find(&all, &label_id)?;
    if let Some(p) = &spec.parent_id { find(&all, p)?; }
    let n = spec.name.trim().to_string();
    if !n.is_empty() && sibling_taken(&all, &spec.parent_id, &n, Some(&label_id)) { return Err(invalid("name", format!("there is already a label named \"{n}\" there"))); }
    scope_known(store, &spec.scope)?;
    // the pin of a built-in label switches its split tab in the same transaction (label_meta.rs `label_tab_follow`)
    store.write(move |ctx| { ctx.label_update_full(&label_id, spec, now)?; ctx.label_tab_follow(&label_id) }).map_err(from_store)?;
    Ok(OK)
}

fn rename(store: &Store, label_id: String, name: String, now: i64) -> Result<CommandOutcome, CoreError> {
    let all = labels(store)?;
    let cur = find(&all, &label_id)?;
    check_label_rename(cur.clone())?;
    let n = one_name(&name)?;
    if sibling_taken(&all, &cur.parent_id, &n, Some(&label_id)) { return Err(invalid("name", format!("\"{n}\" exists already here"))); }
    if n == cur.name { return Ok(OK); }
    let previous_name = cur.name.clone();
    let entry = store
        .write(move |ctx| {
            ctx.label_rename(&label_id, &n, now)?;
            actions::usecases::undo::push_entry(ctx, &UndoAction::RenameLabel { label_id: label_id.clone(), previous_name }, now)
        })
        .map_err(from_store)?;
    Ok(CommandOutcome { undo_entry_id: Some(entry), created_id: None })
}

fn move_label(store: &Store, label_id: String, parent_id: Option<String>, now: i64) -> Result<CommandOutcome, CoreError> {
    let all = labels(store)?;
    let cur = find(&all, &label_id)?;
    if cur.kind == LabelKind::BuiltIn && parent_id.is_some() { return Err(invalid("parent", format!("{} is a built-in category: it stays at the top level", cur.name))); }
    let parent = match &parent_id { Some(p) => Some(find(&all, p)?), None => None };
    let under = format!("{} > ", cur.path);
    if let Some(p) = &parent {
        if p.id == cur.id || p.path.starts_with(&under) { return Err(invalid("parent", format!("\"{}\" cannot go inside itself", cur.name))); }
    }
    if cur.parent_id == parent_id { return Ok(OK); }
    let new_depth = parent.as_ref().map(|p| p.depth as u64).unwrap_or(0) + 1;
    let deep = all.iter().filter(|l| l.id == cur.id || l.path.starts_with(&under)).max_by_key(|l| l.depth).cloned().unwrap_or_else(|| cur.clone());
    let deepest = deep.depth as u64 - cur.depth as u64 + new_depth;
    if deepest > contracts::constants::LABEL_DEPTH_MAX {
        let rest: Vec<&str> = deep.path.split(" > ").skip((cur.depth as usize).saturating_sub(1)).collect();
        let path = match &parent { Some(p) => format!("{} > {}", p.path, rest.join(" > ")), None => rest.join(" > ") };
        return Err(invalid("label", depth_words(&path, deepest)));
    }
    if sibling_taken(&all, &parent_id, &cur.name, Some(&label_id)) { return Err(invalid("name", format!("\"{}\" exists already there", cur.name))); }
    // the end of the new parent's children, as UpdateLabel's new parent does (label_meta.rs `label_update_full`)
    // the label's place among its old siblings, for Z (the contract's MoveLabel holds the old parent only; the entry carries the place beside it)
    let previous_parent_id = cur.parent_id.clone();
    let previous_order = all.iter().filter(|l| l.parent_id == cur.parent_id).position(|l| l.id == cur.id).unwrap_or(0);
    let entry = store
        .write(move |ctx| {
            ctx.label_reorder(&label_id, parent_id, u32::MAX)?;
            actions::usecases::undo::push_entry_extra(ctx, &UndoAction::MoveLabel { label_id: label_id.clone(), previous_parent_id }, "previous_order", &previous_order.to_string(), now)
        })
        .map_err(from_store)?;
    Ok(CommandOutcome { undo_entry_id: Some(entry), created_id: None })
}

/// Every message filed under `label_id`, with its account and its place on the server.
fn carrier_refs(ctx: &WriteCtx<'_>, label_id: &str) -> Result<Vec<(String, MessageRef)>, StoreError> {
    let mut st = ctx.tx.prepare_cached(
        "SELECT m.account_id, m.id, m.thread_id, m.remote_id, m.folder FROM label_assignment la JOIN message m ON m.id = la.message_id WHERE la.label_id = ?1 ORDER BY m.id",
    )?;
    let rows = st.query_map([label_id], |r| Ok((r.get::<_, String>(0)?, MessageRef { message_id: r.get(1)?, thread_id: r.get(2)?, remote_id: r.get(3)?, folder: r.get(4)? })))?;
    let out = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(out)
}

/// One stored message with its account and its place on the server; None when no message has this id.
fn message_ref(ctx: &WriteCtx<'_>, id: &str) -> Result<Option<(String, MessageRef)>, StoreError> {
    let mut st = ctx.tx.prepare_cached("SELECT account_id, id, thread_id, remote_id, folder FROM message WHERE id = ?1")?;
    let rows = st.query_map([id], |r| Ok((r.get::<_, String>(0)?, MessageRef { message_id: r.get(1)?, thread_id: r.get(2)?, remote_id: r.get(3)?, folder: r.get(4)? })))?;
    let mut found = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(found.pop())
}

/// The ids once each, in the order given.
fn once_each(ids: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    ids.into_iter().filter(|id| seen.insert(id.clone())).collect()
}

/// What the server must hear: one ledger request per (account, label path), "label" rows with `applied`. A message with no server
/// copy (no UID) gets no row (`writeback::pending_rows`).
fn queue(ctx: &mut WriteCtx<'_>, items: Vec<(String, String, MessageRef)>, applied: bool, now: i64) -> Result<(), StoreError> {
    let mut groups: Vec<((String, String), Vec<MessageRef>)> = Vec::new();
    for (account, path, r) in items {
        match groups.iter_mut().find(|(k, _)| k.0 == account && k.1 == path) {
            Some((_, v)) => v.push(r),
            None => groups.push(((account, path), vec![r])),
        }
    }
    for ((account_id, label_name), messages) in groups {
        Ledger.execute(ctx, &WriteBack::Label { account_id, messages, label_name, applied }, now)?;
    }
    Ok(())
}

/// DeleteLabel: an owner's label with the labels under it (built-ins refused in words, `check_label_delete`). The messages stay and lose
/// the labels (the assignments go by cascade, the rules by the store's trigger); the server hears each removal. No undo entry (delta 9).
fn delete(store: &Store, label_id: String, now: i64) -> Result<CommandOutcome, CoreError> {
    let all = labels(store)?;
    let cur = find(&all, &label_id)?;
    check_label_delete(cur.clone())?;
    let under = format!("{} > ", cur.path);
    let gone: Vec<(String, String)> = all.iter().filter(|l| l.id == cur.id || l.path.starts_with(&under)).map(|l| (l.id.clone(), l.path.clone())).collect();
    store
        .write(move |ctx| {
            let mut back = Vec::new();
            for (id, path) in &gone {
                for (account, r) in carrier_refs(ctx, id)? {
                    back.push((account, path.clone(), r));
                }
            }
            ctx.label_delete_tree(&label_id)?;
            queue(ctx, back, false, now)
        })
        .map_err(from_store)?;
    Ok(OK)
}

/// RemoveLabel: the label leaves the messages that carry it, whoever filed it, and the removal is recorded on their threads (a rule or
/// the model never files it back). One undo entry (`Unlabel`) with the ids that lost it; none when none did. An unknown label or message
/// is a no-op, as in the fixture core.
fn remove(store: &Store, message_ids: Vec<String>, label_id: String, now: i64) -> Result<CommandOutcome, CoreError> {
    let entry = store
        .write(move |ctx| {
            let Some(label) = ctx.label_get(&label_id)? else { return Ok(None) };
            let mut known_ids = Vec::new();
            let mut refs = Vec::new();
            for id in once_each(message_ids) {
                if let Some(found) = message_ref(ctx, &id)? {
                    known_ids.push(id);
                    refs.push(found);
                }
            }
            let carrying = ctx.label_carriers(&label_id, &known_ids, true)?;
            if carrying.is_empty() { return Ok(None); }
            let taken = ctx.label_remove_by_user(&label_id, &carrying)?;
            if taken.is_empty() { return Ok(None); }
            let back = refs.into_iter().filter(|(_, r)| taken.contains(&r.message_id)).map(|(a, r)| (a, label.path.clone(), r)).collect();
            queue(ctx, back, false, now)?;
            Ok(Some(actions::usecases::undo::push_entry(ctx, &UndoAction::Unlabel { message_ids: taken, label_id: label_id.clone() }, now)?))
        })
        .map_err(from_store)?;
    Ok(CommandOutcome { undo_entry_id: entry, created_id: None })
}

/// RemoveAllLabels: every label leaves these messages; one undo entry (`UnlabelAll`) holding what went; none when nothing did.
fn remove_all(store: &Store, message_ids: Vec<String>, now: i64) -> Result<CommandOutcome, CoreError> {
    let entry = store
        .write(move |ctx| {
            let mut known_ids = Vec::new();
            let mut refs: HashMap<String, (String, MessageRef)> = HashMap::new();
            for id in once_each(message_ids) {
                if let Some(found) = message_ref(ctx, &id)? {
                    refs.insert(id.clone(), found);
                    known_ids.push(id);
                }
            }
            // who decided each filing is read before the rows go; Z puts it back (actions undo.rs `restore_deciders`)
            let deciders = ctx.labels_deciders(&known_ids)?;
            let removed = ctx.labels_remove_all(&known_ids)?;
            if removed.is_empty() { return Ok(None); }
            let mut back = Vec::new();
            for a in &removed {
                let (Some((account, r)), Some(l)) = (refs.get(&a.message_id).cloned(), ctx.label_get(&a.label_id)?) else { continue };
                back.push((account, l.path, r));
            }
            queue(ctx, back, false, now)?;
            Ok(Some(actions::usecases::undo::push_entry_extra(ctx, &UndoAction::UnlabelAll { removed }, "deciders", &deciders, now)?))
        })
        .map_err(from_store)?;
    Ok(CommandOutcome { undo_entry_id: entry, created_id: None })
}

// ---------------------------------------------------------------- state paths

fn s(v: &str) -> String {
    let mut o = String::from("\"");
    for c in v.chars() {
        match c { '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)), c => o.push(c) }
    }
    o.push('"');
    o
}
fn kind_words(k: LabelKind) -> &'static str { if k == LabelKind::BuiltIn { "built_in" } else { "owner" } }
fn scope_words(sc: &LabelScope) -> String { match sc { LabelScope::Account { account_id } => format!("account:{account_id}"), LabelScope::All => "all".into() } }

/// `labels/tree`: the fixture core's `treeJSON`, keys sorted, children in sidebar order; `unread` is the unread inbox mail filed there.
fn tree_json(all: &[MailLabel], parent: Option<&str>, unread: &HashMap<String, u32>) -> String {
    let items: Vec<String> = all
        .iter()
        .filter(|l| l.parent_id.as_deref() == parent)
        .map(|l| {
            format!(
                "{{\"children\":{},\"depth\":{},\"id\":{},\"kind\":{},\"name\":{},\"order\":{},\"path\":{},\"pinned\":{},\"scope\":{},\"unread\":{}}}",
                tree_json(all, Some(l.id.as_str()), unread), l.depth, s(&l.id), s(kind_words(l.kind)), s(&l.name), l.order, s(&l.path), l.pinned,
                s(&scope_words(&l.scope)), unread.get(&l.id).copied().unwrap_or(0)
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

/// The label paths the test engine serves and no other real-core reader did; None = not one of them.
pub(crate) fn state(store: &Store, path: &str) -> Option<String> {
    let p: Vec<&str> = path.split('/').collect();
    let all = || store.labels_full().ok().map(labels_without_views);
    match p.as_slice() {
        ["labels", "tree"] => {
            let ls = all()?;
            Some(tree_json(&ls, None, &store.label_unread_counts().ok()?))
        }
        ["labels", "pinned"] => {
            let unread = store.label_unread_counts().ok()?;
            let items: Vec<String> = pinned_in_tree_order(all()?)
                .iter()
                .map(|l| format!("{{\"id\":{},\"name\":{},\"unread\":{}}}", s(&l.id), s(&l.name), unread.get(&l.id).copied().unwrap_or(0)))
                .collect();
            Some(format!("[{}]", items.join(",")))
        }
        ["labels", "count"] => Some(all()?.len().to_string()),
        ["labels", id, "rerun_count"] => {
            all()?.iter().any(|l| l.id == *id).then_some(())?;
            store.label_filed_count(id).ok().map(|n| n.to_string())
        }
        ["label", id, field] => {
            let l = all()?.into_iter().find(|l| l.id == *id)?;
            match *field {
                "description" => Some(s(&l.description)),
                "pinned" => Some(l.pinned.to_string()),
                "scope" => Some(s(&scope_words(&l.scope))),
                "kind" => Some(s(kind_words(l.kind))),
                "order" => Some(l.order.to_string()),
                "colour" => l.colour.as_deref().map(s),
                "path" => Some(s(&l.path)),
                "parent" => l.parent_id.as_deref().map(s),
                "name" => Some(s(&l.name)),
                "depth" => Some(l.depth.to_string()),
                "rule_sets" => store.label_chains(id, ChainPlacement::Entry).ok().map(|c| c.len().to_string()),
                _ => None,
            }
        }
        ["label_assignment", id] => store.label_names_of(id).ok().map(|names| format!("[{}]", names.iter().map(|n| s(n)).collect::<Vec<_>>().join(","))),
        ["label_removed", message, label] => store.label_removed(label, message).ok().map(|b| b.to_string()),
        _ => None,
    }
}
