//! The store writes of the label flow, one transaction per run: take a message off the label a flow card put it under, file it
//! under the label the flow now names (with the card, the chain and who decided), move it to or from Junk, and keep the
//! `flow_rejection` row in step. The owner's own filing is never overwritten: `label_file_flow` replaces a filing's decision, so a
//! label the owner filed by hand is skipped here, whatever a rule or the model says.
use std::sync::Arc;

use contracts::{DecidedBy, DecisionPort, FlowRejection, WhyCard};
use decision::{FlowPlace, FlowVerdict, Verdict, LABELS_MODEL_ID};
use rusqlite::{params, OptionalExtension};
use store::{Store, StoreError, WriteCtx, INBOX_FOLDER};
use sync_imap::FolderRole;

use crate::flow_ctx::ai_decider;
use crate::imap_source::Roles;
use crate::reads::folders_with_role;

/// One filing under a label: who decided, the model's entry only when the model decided, its probability, the card and the chain.
pub(crate) struct Filing {
    pub label_id: String,
    pub by: DecidedBy,
    pub entry: Option<String>,
    pub probability: Option<f64>,
    pub card: WhyCard,
    pub chain_id: Option<u32>,
}

/// What one message needs written. `unassign` is the label a flow card holds it under today; `back_to_inbox` leaves Junk; `file` is
/// the new label; `to_junk` is the account's Junk folder; `rejection` is the row to keep (None clears the message's older row).
pub(crate) struct Write {
    pub message_id: String,
    pub unassign: Option<String>,
    pub back_to_inbox: bool,
    pub file: Option<Filing>,
    pub to_junk: Option<String>,
    pub rejection: Option<FlowRejection>,
}

impl Write {
    /// Nothing to do for a fresh message that stays where it is with no rejection to clear.
    pub fn is_noop_for_arrival(&self) -> bool {
        self.unassign.is_none() && !self.back_to_inbox && self.file.is_none() && self.to_junk.is_none() && self.rejection.is_none()
    }
}

/// The account's Junk folder, as its server named it at the last LIST; None while the roles are unknown.
pub(crate) fn junk_folder(store: &Store, roles: &Roles, account_id: &str) -> Option<String> {
    folders_with_role(store, roles, &[account_id.to_string()], FolderRole::Junk).into_iter().next().map(|(_, folder)| folder)
}

/// The filing a verdict's flow names, with who decided it. The model's entry id rides only for a filing the model made (cards AI,
/// exit and fallback); an entry rule is named by its rule.
pub(crate) fn filing_of(v: &Verdict, fl: &FlowVerdict) -> Option<Filing> {
    let FlowPlace::Label(id) = &fl.place else { return None };
    Some(Filing {
        label_id: id.clone(),
        by: v.decided_by.to_contract(),
        entry: v.decided_by.decided_entry_id(),
        probability: v.decided_by.probability(),
        card: fl.card,
        chain_id: fl.chain_id,
    })
}

/// The filing a flip makes: the card and chain the flip chose, decided by whoever decides the new filing (a model filing carries the
/// model entry's id).
pub(crate) fn flip_filing(to: &FlowVerdict, decided_by: DecidedBy, port: Option<&Arc<dyn DecisionPort>>) -> Option<Filing> {
    let FlowPlace::Label(id) = &to.place else { return None };
    let by = if matches!(decided_by, DecidedBy::Default) && to.ai_pick.is_some() { ai_decider(port) } else { decided_by };
    let by_model = matches!(by, DecidedBy::Laya | DecidedBy::Jev | DecidedBy::Apple | DecidedBy::Model { .. });
    Some(Filing { label_id: id.clone(), entry: by_model.then(|| LABELS_MODEL_ID.to_string()), by, probability: None, card: to.card, chain_id: to.chain_id })
}

fn owner_filed(ctx: &WriteCtx<'_>, label_id: &str, message_id: &str) -> Result<bool, StoreError> {
    let by: Option<String> = ctx
        .tx
        .prepare_cached("SELECT decided_by FROM label_assignment WHERE label_id = ?1 AND message_id = ?2")?
        .query_row(params![label_id, message_id], |r| r.get(0))
        .optional()?;
    Ok(by.as_deref() == Some("owner"))
}

/// Writes every message's change in one transaction (the counts and the streams move by their difference: the messages are declared
/// first). An error undoes all of it.
/// Lane decided-model (SPEC A133, lead's ruling 15 Nov): every filing written here (arrivals, flips, re-runs) that names the on-device
/// decider `Laya` is named after the catalogue model the judge marker holds, when that is a model other than Laya and Apple
/// (`label_ai_failure::named_model`). The marker is read at the write; the limit this leaves is in docs/lanes/decided-model.md.
pub(crate) fn apply(store: &Store, mut writes: Vec<Write>, now_ms: i64) -> Result<(), String> {
    if writes.is_empty() {
        return Ok(());
    }
    for f in writes.iter_mut().filter_map(|w| w.file.as_mut()) {
        f.by = crate::label_ai_failure::named_model(store, f.by.clone());
    }
    store
        .write(move |ctx| {
            let ids: Vec<String> = writes.iter().map(|w| w.message_id.clone()).collect();
            ctx.will_change(&ids)?;
            for w in &writes {
                let one = std::slice::from_ref(&w.message_id);
                if let Some(old) = &w.unassign {
                    ctx.label_unassign(old, one)?;
                }
                if w.back_to_inbox {
                    ctx.set_folder(one, INBOX_FOLDER)?;
                }
                if let Some(f) = &w.file {
                    if !owner_filed(ctx, &f.label_id, &w.message_id)? {
                        ctx.label_file_flow(&f.label_id, &w.message_id, &f.by, f.entry.as_deref(), f.probability, f.card, f.chain_id, now_ms)?;
                    }
                }
                if let Some(folder) = &w.to_junk {
                    ctx.set_folder(one, folder)?;
                }
                match &w.rejection {
                    Some(r) => ctx.flow_rejection_put(r)?,
                    None => {
                        ctx.flow_rejection_clear(&w.message_id)?;
                    }
                }
            }
            Ok(())
        })
        .map_err(|e| format!("the filing could not be saved: {e}"))
}
