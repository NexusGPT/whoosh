//! Use case: "Re-run on N filed messages" (owner, 2026-11-08 09:55: "if we change description/rules, proposing a "re-run"
//! on all previously categorised email"). Runs the Labels page ALONE on the messages handed over and returns, per message,
//! the Labels verdict now (its `decided_by` names the rule or the model that decides today) and what that means for the
//! changed label: file, unfile, keep, or pending when no rule settled it and the meaning part could not run (engine off or
//! refused): then the old filing stays and the caller marks `label/<id>/rerun/model_pending`. It holds no store handle, so
//! it writes nothing: the caller writes label_assignment and decided_by from the rows. Bounded like the preview; what the
//! bound left unstarted is listed and keeps its filing. A140: each message runs the three-step pipeline (`flow::decide_pipeline`):
//! the model is asked again here (a Re-run is one of the three times it is), but only about the messages an AI-on label's step 1
//! passes; a message no AI-on label's step 1 passes is settled by the AI-off labels' step 1, or by no label, and is never pending.
use crate::decide::DecideMessage;
use crate::entry::{LabelNode, TabOption, Value};
use crate::flow::{ai_candidates, decide_pipeline};
use crate::outcome::Outcome;
use crate::settings::DecisionSettings;
use crate::tuning::decision_rerun_ms;
use crate::verdict::{Source, Verdict};
use contracts::{Clock, CoreError, DecidedBy, DecisionPort, DecisionProviderKind, FlowRejection, WhyCard};
use smart_inbox::MessageFacts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RerunAction {
    /// Not filed under the label today; the page files it there now.
    File,
    /// Filed today; the page now files it elsewhere or nowhere.
    Unfile,
    /// Filed today and still filed.
    Keep,
    /// Neither filed today nor filed now.
    Leave,
    /// No rule settled it and the meaning part could not run: the old filing stays.
    Pending,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RerunRow {
    pub message_id: String,
    /// Filed under the label today (the facts' label_ids).
    pub filed_today: bool,
    /// The Labels verdict now, with its `decided_by`; None when the row is pending.
    pub verdict: Option<Verdict>,
    pub action: RerunAction,
}

/// The arguments of `Store::label_file_flow(label, message, by, entry, probability, card, chain, now)` (contracts delta 12 and 17)
/// for one re-run row that files a label: the label the verdict names (the changed label for File and Keep; another label when the
/// meaning or the exit filter moved the message there), who decided, the model entry's id only when the model decided, its
/// probability, and the card and chain that placed it (delta 17; `Hand` when the flow did not place it).
#[derive(Debug, Clone, PartialEq)]
pub struct FileWrite {
    pub label_id: String,
    pub message_id: String,
    pub by: DecidedBy,
    pub entry: Option<String>,
    pub probability: Option<f64>,
    pub card: WhyCard,
    pub chain_id: Option<u32>,
}

impl RerunRow {
    /// What the store writes for this row, when its verdict files a label: None for a row that files nothing (Unfile to
    /// no label, Leave, Pending). Unfiling the changed label itself is the caller's `label_unfile`.
    pub fn file_write(&self) -> Option<FileWrite> {
        let v = self.verdict.as_ref()?;
        let Value::Label(Some(label_id)) = &v.value else { return None };
        Some(FileWrite {
            label_id: label_id.clone(),
            message_id: self.message_id.clone(),
            by: v.decided_by.to_contract(),
            entry: v.decided_by.decided_entry_id(),
            probability: v.decided_by.probability(),
            card: v.flow.as_ref().map(|fl| fl.card).unwrap_or(WhyCard::Hand),
            chain_id: v.flow.as_ref().and_then(|fl| fl.chain_id),
        })
    }

    /// The `flow_rejection` row this row writes when the exit filter sent the AI's pick elsewhere (to another label, Primary or Junk);
    /// None otherwise, and the caller then clears the message's older row.
    pub fn rejection(&self, now_ms: i64) -> Option<FlowRejection> {
        self.verdict.as_ref()?.flow.as_ref()?.rejection(&self.message_id, now_ms)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rerun {
    pub label_id: String,
    pub rows: Vec<RerunRow>,
    /// The bound left these unstarted; their filing stays.
    pub not_reached: Vec<String>,
    /// At least one row is pending: the meaning part has not spoken (engine off, or its refusal in `engine_error`).
    pub model_pending: bool,
    /// The first engine refusal seen, in the engine's words. Lane fix-aisort round 11: a refusal that belongs to one mail is not here.
    pub engine_error: Option<CoreError>,
    /// Lane fix-aisort round 11: (message, reason id) of the mails the judge refused for themselves (`decide::mail_refusal`): their row is
    /// Pending (they keep their filing), and the caller sets them aside so they are not asked again.
    pub set_aside: Vec<(String, String)>,
}

pub struct RerunLabel<'a> {
    pub settings: &'a DecisionSettings,
    pub engine: Option<&'a dyn DecisionPort>,
    pub tabs: &'a [TabOption],
    pub labels: &'a [LabelNode],
    /// The label whose description or rules changed.
    pub label_id: String,
    pub clock: &'a dyn Clock,
    /// None = no bound.
    pub budget_ms: Option<u64>,
}

impl<'a> RerunLabel<'a> {
    /// The bound read from the ledger (DECISION_RERUN_MS). Refused in words while the row holds no number.
    pub fn from_ledger(
        settings: &'a DecisionSettings,
        engine: Option<&'a dyn DecisionPort>,
        tabs: &'a [TabOption],
        labels: &'a [LabelNode],
        label_id: String,
        clock: &'a dyn Clock,
    ) -> Result<RerunLabel<'a>, CoreError> {
        let budget_ms = decision_rerun_ms().ok_or_else(|| CoreError::Invalid {
            field: "DECISION_RERUN_MS".into(),
            reason_words: "docs/CONSTANTS.md holds no bound for a label re-run yet".into(),
        })?;
        Ok(RerunLabel { settings, engine, tabs, labels, label_id, clock, budget_ms: Some(budget_ms) })
    }

    pub fn execute(&self, inputs: Vec<MessageFacts>) -> Rerun {
        // The Labels page alone, switched on for the run: the owner asked for this re-run, whatever the page's switch.
        let mut only_labels = self.settings.clone();
        for p in &mut only_labels.pages {
            p.enabled = p.list.outcome == Outcome::Labels;
        }
        let asks_model = only_labels.page(Outcome::Labels).is_some_and(|p| p.list.has_model_entry());
        let engine_off = self.engine.is_none() || only_labels.engine == DecisionProviderKind::Rules;
        // A140: each message runs the three-step pipeline (`flow::decide_pipeline`): the AI is asked only about what step 1 passed
        let decide = DecideMessage { settings: &only_labels, engine: self.engine, tabs: self.tabs, labels: self.labels };

        let start = self.clock.now_ms();
        let mut out = Rerun { label_id: self.label_id.clone(), rows: vec![], not_reached: vec![], model_pending: false, engine_error: None, set_aside: vec![] };
        for f in inputs {
            if let Some(b) = self.budget_ms {
                if self.clock.now_ms().saturating_sub(start) >= b as i64 {
                    out.not_reached.push(f.message_id.clone());
                    continue;
                }
            }
            let d = decide_pipeline(&decide, &f);
            let refused = d.engine_error.as_ref().and_then(crate::decide::mail_refusal);
            if out.engine_error.is_none() && refused.is_none() {
                out.engine_error = d.engine_error.clone();
            }
            let filed_today = f.label_ids.iter().any(|l| l == &self.label_id);
            let verdict = d.get(Outcome::Labels).cloned();
            let by_default = verdict.as_ref().is_some_and(|v| v.decided_by.source == Source::Default);
            // A140: pending only when the model had something to rule on for this message (an AI-on label's step 1 passed)
            let meaning_missing = asks_model && by_default && (engine_off || d.engine_error.is_some()) && !ai_candidates(self.labels, &f).is_empty();
            let (action, verdict) = if meaning_missing {
                // a mail the judge refused for itself is set aside by the caller, not "pending for a model"
                match refused {
                    Some(reason) => out.set_aside.push((f.message_id.clone(), reason)),
                    None => out.model_pending = true,
                }
                (RerunAction::Pending, None)
            } else {
                let filed_now = verdict.as_ref().is_some_and(|v| matches!(&v.value, Value::Label(Some(id)) if id == &self.label_id));
                let action = match (filed_today, filed_now) {
                    (false, true) => RerunAction::File,
                    (true, false) => RerunAction::Unfile,
                    (true, true) => RerunAction::Keep,
                    (false, false) => RerunAction::Leave,
                };
                (action, verdict)
            };
            out.rows.push(RerunRow { message_id: f.message_id.clone(), filed_today, verdict, action });
        }
        out
    }
}
