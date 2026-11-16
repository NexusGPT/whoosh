//! Use case: the Preview button. Runs every switched-on page on the most recent DECISION_PREVIEW_N messages and returns,
//! per message, the verdicts and whether "Which tab" differs from where the message sits today. It holds no store
//! handle: it cannot write message_category. DECISION_PREVIEW_MS bounds it; a message not started inside the bound is
//! listed as not reached.
use crate::decide::DecideMessage;
use crate::entry::Value;
use crate::flow::decide_pipeline;
use crate::outcome::Outcome;
use crate::tuning::{decision_preview_ms, decision_preview_n};
use crate::verdict::Verdict;
use contracts::{Clock, CoreError};
use smart_inbox::{MessageFacts, TabSet};

pub struct PreviewInput {
    pub facts: MessageFacts,
    /// The tab id the message sits in today.
    pub current_tab: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreviewRow {
    pub message_id: String,
    pub verdicts: Vec<Verdict>,
    /// None when "Which tab" is off or today's tab is unknown.
    pub differs_from_today: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    pub rows: Vec<PreviewRow>,
    /// Messages the bound left unstarted, newest first.
    pub not_reached: Vec<String>,
    /// The first engine refusal seen, in the engine's words.
    pub engine_error: Option<CoreError>,
}

pub struct PreviewDecisions<'a> {
    pub decide: DecideMessage<'a>,
    pub clock: &'a dyn Clock,
    /// Resolves a "Which tab" verdict to a tab id to compare with today's.
    pub tabs: Option<&'a TabSet>,
    pub budget_ms: u64,
}

impl<'a> PreviewDecisions<'a> {
    /// The bound read from the ledger. Refused while DECISION_PREVIEW_MS holds no measured number.
    pub fn from_ledger(decide: DecideMessage<'a>, clock: &'a dyn Clock, tabs: Option<&'a TabSet>) -> Result<PreviewDecisions<'a>, CoreError> {
        let budget_ms = decision_preview_ms().ok_or_else(|| CoreError::Invalid {
            field: "DECISION_PREVIEW_MS".into(),
            reason_words: "docs/CONSTANTS.md holds no measured number for the preview bound yet".into(),
        })?;
        Ok(PreviewDecisions { decide, clock, tabs, budget_ms })
    }

    pub fn execute(&self, mut inputs: Vec<PreviewInput>) -> Preview {
        inputs.sort_by(|a, b| b.facts.received_at_ms.cmp(&a.facts.received_at_ms).then_with(|| a.facts.message_id.cmp(&b.facts.message_id)));
        inputs.truncate(decision_preview_n());
        let start = self.clock.now_ms();
        let mut rows = Vec::new();
        let mut not_reached = Vec::new();
        let mut engine_error = None;
        for input in inputs {
            if self.clock.now_ms().saturating_sub(start) >= self.budget_ms as i64 {
                not_reached.push(input.facts.message_id.clone());
                continue;
            }
            // A140: the Labels page runs the three-step pipeline (the model sees only what an AI-on label's step 1 picked)
            let d = decide_pipeline(&self.decide, &input.facts);
            if engine_error.is_none() {
                engine_error = d.engine_error.clone();
            }
            let differs = match (d.get(Outcome::WhichTab), &input.current_tab, self.tabs) {
                (Some(Verdict { value: Value::Tab(t), .. }), Some(today), Some(tabs)) => Some(&tabs.resolve(t) != today),
                _ => None,
            };
            rows.push(PreviewRow { message_id: input.facts.message_id.clone(), verdicts: d.verdicts, differs_from_today: differs });
        }
        Preview { rows, not_reached, engine_error }
    }
}
