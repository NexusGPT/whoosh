//! fix-suggest: the real engine OFFERS the follow-ups the test engine offers (Sources/CoreBridge/FixtureLabels.swift `offerAfter*Locked`;
//! PRD "Smart suggestions": junk => "Block <sender>", archive of a newsletter => "Unsubscribe", a third snooze of the same thread => "Mute
//! thread", a label applied twice to the same sender => "Always label"). Rule-based only: counts of the owner's own acts, no model.
//!
//! - Each function runs right after its act committed (`actions_bridge.rs` `Act::run`, one call line each), in ONE store transaction of
//!   its own, and never fails the act: an offer that cannot be kept is logged and the act's answer stands.
//! - One offer per kind and subject, ever (`store` `WriteCtx::suggestion_offer`): an offer still open, accepted or dismissed is never
//!   offered again. A dismissal keeps its row as a tombstone (`suggestion_dismiss`).
//! - The new offer's id is the act's `created_id` when the act created nothing (the test engine answers it for junk, archive and snooze).
//! - The counts of repeated acts (snooze, label) are read from the undo stack (`undo_entry`, written by the actions crate in the act's own
//!   transaction): an entry holds the kind, the message ids and the label. The stack keeps the newest UNDO_DEPTH entries not undone, so a
//!   repeat further back than that is not counted (docs/lanes/fix-suggest.md, finding 2).
use contracts::constants::{SUGGEST_LABEL_REPEAT, SUGGEST_SNOOZE_COUNT};
use contracts::{CommandOutcome, SuggestionKind};
use rusqlite::{params, OptionalExtension};
use store::{Store, StoreError, WriteCtx};

use crate::reads::json_text;
use crate::rust_core::now_ms;

fn ids_json(ids: &[String]) -> String { format!("[{}]", ids.iter().map(|i| json_text(i)).collect::<Vec<_>>().join(",")) }

/// The act's answer with the offer's id in `created_id` when the act created nothing; an offer that failed is logged, never raised.
fn answered(out: CommandOutcome, what: &str, offered: Result<Option<String>, StoreError>) -> CommandOutcome {
    match offered {
        Ok(Some(id)) if out.created_id.is_none() => CommandOutcome { created_id: Some(id), ..out },
        Ok(_) => out,
        Err(e) => { eprintln!("suggest: the {what} offer was not kept: {e}"); out }
    }
}

fn sender_of(ctx: &WriteCtx<'_>, message_id: &str) -> Result<Option<String>, StoreError> {
    Ok(ctx.tx.query_row("SELECT lower(trim(from_address)) FROM message WHERE id = ?1", params![message_id], |r| r.get::<_, String>(0)).optional()?)
}

/// After MarkSpam: the sender of the first message, not blocked yet, is offered "Block <sender>" (test engine `offerAfterJunkLocked`).
pub(crate) fn after_junk(store: &Store, message_ids: &[String], out: CommandOutcome) -> CommandOutcome {
    let Some(first) = message_ids.first().cloned() else { return out };
    let blocks = match store.blocks() { Ok(b) => b, Err(e) => return answered(out, "block sender", Err(e)) };
    let now = now_ms();
    let offered = store.write(move |ctx| {
        let Some(from) = sender_of(ctx, &first)? else { return Ok(None) };
        if from.is_empty() || contracts::is_blocked(blocks, from.clone()) { return Ok(None); }
        ctx.suggestion_offer(SuggestionKind::BlockSender, &from, None, now)
    });
    answered(out, "block sender", offered)
}

/// After Archive: the first archived message that carries a way out of its list (List-Unsubscribe) and whose list was not left offers
/// "Unsubscribe from <sender>" (test engine `offerAfterArchiveLocked`).
pub(crate) fn after_archive(store: &Store, message_ids: &[String], out: CommandOutcome) -> CommandOutcome {
    let ids = ids_json(message_ids);
    let candidates = store.write(move |ctx| {
        let mut st = ctx.tx.prepare_cached(
            "SELECT m.id, lower(trim(m.from_address)) FROM json_each(?1) j JOIN message m ON m.id = j.value \
             WHERE m.list_unsubscribe_json IS NOT NULL ORDER BY j.key")?;
        let rows = st.query_map(params![ids], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    });
    let candidates = match candidates { Ok(c) => c, Err(e) => return answered(out, "unsubscribe", Err(e)) };
    let mut sender = None;
    for (id, from) in candidates {
        let way_out = store.list_unsubscribe(&id).map(|(urls, one_click)| contracts::unsubscribe_method_from(urls, one_click).is_some());
        let left = store.list_left_at(&id).map(|at| at.is_some());
        match (way_out, left) {
            (Ok(true), Ok(false)) if !from.is_empty() => { sender = Some(from); break; }
            (Err(e), _) | (_, Err(e)) => return answered(out, "unsubscribe", Err(e)),
            _ => {}
        }
    }
    let Some(from) = sender else { return out };
    let now = now_ms();
    let offered = store.write(move |ctx| ctx.suggestion_offer(SuggestionKind::Unsubscribe, &from, None, now));
    answered(out, "unsubscribe", offered)
}

/// After Snooze: each thread of the snoozed messages, snoozed for the SUGGEST_SNOOZE_COUNT-th time or more and not muted, offers "Mute this
/// thread" (test engine `offerAfterSnoozeLocked`). A snooze is counted by its undo entry (one per act that changed a return time).
pub(crate) fn after_snooze(store: &Store, message_ids: &[String], out: CommandOutcome) -> CommandOutcome {
    let ids = ids_json(message_ids);
    let now = now_ms();
    let offered = store.write(move |ctx| {
        let threads: Vec<String> = {
            let mut st = ctx.tx.prepare_cached("SELECT DISTINCT m.thread_id FROM json_each(?1) j JOIN message m ON m.id = j.value ORDER BY m.thread_id")?;
            let rows = st.query_map(params![ids], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        let mut offered = None;
        for thread in threads {
            let snoozes: i64 = ctx.tx.query_row(
                "SELECT count(*) FROM undo_entry u WHERE json_extract(u.action_json, '$.kind') = 'snooze' \
                 AND EXISTS (SELECT 1 FROM json_each(u.action_json, '$.ids') j JOIN message m ON m.id = j.value WHERE m.thread_id = ?1)",
                params![thread], |r| r.get(0))?;
            if (snoozes.max(0) as u64) < SUGGEST_SNOOZE_COUNT { continue; }
            let muted: bool = ctx.tx.query_row("SELECT EXISTS (SELECT 1 FROM message WHERE thread_id = ?1 AND muted != 0)", params![thread], |r| r.get(0))?;
            if muted { continue; }
            if let Some(id) = ctx.suggestion_offer(SuggestionKind::MuteThread, &thread, None, now)? { offered = Some(id); }
        }
        Ok(offered)
    });
    answered(out, "mute thread", offered)
}

/// After Label: each sender of the messages the act labelled, given the same label by SUGGEST_LABEL_REPEAT acts or more, offers "Always
/// label <sender>" (test engine `offerAfterLabelLocked`). The act's own undo entry names the label and the messages it changed; an act that
/// changed nothing has no entry and offers nothing. The act's `created_id` (a label it made) is kept as it is.
pub(crate) fn after_label(store: &Store, out: CommandOutcome) -> CommandOutcome {
    let Some(entry) = out.undo_entry_id.clone() else { return out };
    let now = now_ms();
    let offered = store.write(move |ctx| {
        let label: Option<String> = ctx.tx.query_row(
            "SELECT json_extract(action_json, '$.label_id') FROM undo_entry WHERE id = ?1 AND json_extract(action_json, '$.kind') = 'label'",
            params![entry], |r| r.get::<_, Option<String>>(0)).optional()?.flatten();
        let Some(label) = label else { return Ok(None) };
        let senders: Vec<String> = {
            let mut st = ctx.tx.prepare_cached(
                "SELECT DISTINCT lower(trim(m.from_address)) FROM undo_entry u, json_each(u.action_json, '$.ids') j JOIN message m ON m.id = j.value \
                 WHERE u.id = ?1 ORDER BY 1")?;
            let rows = st.query_map(params![entry], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        let mut offered = None;
        for sender in senders.into_iter().filter(|s| !s.is_empty()) {
            let acts: i64 = ctx.tx.query_row(
                "SELECT count(*) FROM undo_entry u WHERE json_extract(u.action_json, '$.kind') = 'label' AND json_extract(u.action_json, '$.label_id') = ?1 \
                 AND EXISTS (SELECT 1 FROM json_each(u.action_json, '$.ids') j JOIN message m ON m.id = j.value WHERE lower(trim(m.from_address)) = ?2)",
                params![label, sender], |r| r.get(0))?;
            if (acts.max(0) as u64) < SUGGEST_LABEL_REPEAT { continue; }
            if let Some(id) = ctx.suggestion_offer(SuggestionKind::AlwaysLabel, &sender, Some(&label), now)? { offered = Some(id); }
        }
        Ok(offered)
    });
    // a label act answers the label it created, never the offer (the test engine answers the label id there)
    if let Err(e) = offered { eprintln!("suggest: the always label offer was not kept: {e}"); }
    out
}
