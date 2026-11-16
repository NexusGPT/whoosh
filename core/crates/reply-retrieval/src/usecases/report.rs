//! The report: whether the trace's numbers add up, and the contract's `ReplyRetrievalReport` (what `reply/try/<run id>` serves) built from it.
use crate::domain::{DropReason, RetrievalOutcome, RetrievalTrace};
use contracts::ReplyRetrievalReport;

impl RetrievalTrace {
    /// How many dropped excerpts carry this reason.
    pub fn dropped_for(&self, reason: impl Fn(&DropReason) -> bool) -> u32 {
        self.dropped.iter().filter(|d| reason(&d.reason)).count() as u32
    }

    /// Every hit is merged, dropped with a reason, or kept; the questions' counts sum to the hits.
    pub fn adds_up(&self) -> bool {
        let per_question: u32 = self.questions.iter().map(|q| q.found).sum();
        per_question == self.hits && self.hits == self.merged + self.dropped.len() as u32 + self.kept
    }

    /// The distinct excerpts found inside the scope (hits less the merged ones and the leaks): the "found" of the contract's report.
    pub fn found(&self) -> u32 {
        self.hits - self.merged - self.dropped_for(|r| *r == DropReason::OutsideScope)
    }
}

impl RetrievalOutcome {
    /// The contract's report: the questions, the distinct excerpts found in scope, the excerpts kept (as the writer has them), the AI filter's drops.
    pub fn report(&self) -> ReplyRetrievalReport {
        ReplyRetrievalReport {
            questions: self.trace.questions.iter().map(|q| q.question.clone()).collect(),
            found: self.trace.found(),
            kept: self.context.iter().map(|c| c.excerpt.clone()).collect(),
            dropped_by_ai_filter: self.trace.dropped_for(|r| *r == DropReason::AiFilter),
        }
    }
}
