//! Use case: a fact flipped on a stored message (the owner replied, a newer message arrived, a star or a read came from the server).
//! Owner, 2026-11-09 17:25: "When the fact changes (the person replies), the label leaves the message by itself (a re-run on the
//! fact, not on the model)". The decision crate's `RefileOnFact` re-asks the entry cards and then the exit filter of the label the
//! AI chose, on the new facts, and never asks the model; the owner's own filing never moves. This file reads where the message sits
//! now, hands it over, and writes the move at once: off its label, under the new one, to or from Junk, the rejection row kept.
use std::sync::Arc;

use contracts::DecisionPort;
use decision::{FlipAction, FlowPlace, RefileOnFact};
use store::{Store, INBOX_FOLDER};

use crate::flow_ctx::FlowCtx;
use crate::flow_write::{apply, flip_filing, junk_folder, Write};
use crate::imap_source::Roles;

/// A message the flip moved: from where to where, by which card.
#[derive(Debug, Clone, PartialEq)]
pub struct Moved {
    pub message_id: String,
    pub from: String,
    pub to: String,
    pub card: contracts::WhyCard,
    pub chain_id: Option<u32>,
}

/// Re-evaluates `ids` on their current facts. A message is a candidate when it sits in the INBOX, or in a Junk folder because an
/// exit filter sent it there (its rejection row says so); every other message is left alone.
pub(crate) fn run(store: &Store, port: Option<Arc<dyn DecisionPort>>, roles: &Roles, ids: &[String], now_ms: i64) -> Result<Vec<Moved>, String> {
    let stored: Vec<String> = ids.iter().filter(|id| store.message(id).ok().flatten().is_some_and(|m| !m.is_outgoing)).cloned().collect();
    if stored.is_empty() {
        return Ok(vec![]);
    }
    let ctx = FlowCtx::open(store, port.clone())?;
    let refile = RefileOnFact { settings: &ctx.settings, tabs: &[], labels: &ctx.nodes };
    let (mut writes, mut moved) = (Vec::new(), Vec::new());
    for f in &ctx.facts(&stored)? {
        let now = ctx.current(&f.message_id)?;
        let in_inbox = store.message(&f.message_id).ok().flatten().is_some_and(|m| m.folder == INBOX_FOLDER);
        if !in_inbox && now.place != FlowPlace::Junk {
            continue;
        }
        let FlipAction::Move { to, decided_by } = refile.execute(f, &now) else { continue };
        let leaves_label = matches!(&now.place, FlowPlace::Label(_)) && now.card != contracts::WhyCard::Hand;
        let unassign = match &now.place {
            FlowPlace::Label(id) if leaves_label => Some(id.clone()),
            _ => None,
        };
        writes.push(Write {
            message_id: f.message_id.clone(),
            unassign,
            back_to_inbox: now.place == FlowPlace::Junk && to.place != FlowPlace::Junk,
            file: flip_filing(&to, decided_by, port.as_ref()),
            to_junk: matches!(to.place, FlowPlace::Junk).then(|| junk_folder(store, roles, &f.account_id)).flatten(),
            rejection: to.rejection(&f.message_id, now_ms),
        });
        moved.push(Moved { message_id: f.message_id.clone(), from: now.place.key(), to: to.place.key(), card: to.card, chain_id: to.chain_id });
    }
    apply(store, writes, now_ms)?;
    Ok(moved)
}
