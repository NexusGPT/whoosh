//! The whole retrieval, in the order of the Recherche pane: questions → a search per question → merge → size → rank, de-duplicate, keep the
//! best → the clash rule → the AI filter → labels and the writer's budget. Orchestration only: every step is a function of its own file, every
//! outside answer comes through a port. Never fails: a step that cannot run is named in the trace and the rest go on. Deterministic: the same
//! answers from the ports give the same outcome.
use crate::domain::text::words;
use crate::domain::{QuestionOrigin, RetrievalOutcome, RetrievalRequest, RetrievalTrace};
use crate::ports::{ExcerptIndex, ExcerptJudge, QueryWriter};
use crate::usecases::ai_filter::ai_filter;
use crate::usecases::assemble::assemble;
use crate::usecases::clash::apply_clash_rule;
use crate::usecases::gather::gather;
use crate::usecases::keep_best::{dedupe, keep_first, rank};
use crate::usecases::search_scope::sources_in_scope;
use crate::usecases::size_excerpt::size_text;
use crate::usecases::write_questions::{clamp_count, message_value, write_questions, Questions};
use crate::domain::{DropReason, Dropped};
use contracts::{REPLY_EXCERPTS_CEILING, REPLY_KEEP_CEILING};

/// The three ports one retrieval runs on.
pub struct Retriever<'a> {
    pub writer: &'a dyn QueryWriter,
    pub index: &'a dyn ExcerptIndex,
    pub judge: &'a dyn ExcerptJudge,
}

impl Retriever<'_> {
    pub fn run(&self, req: &RetrievalRequest) -> RetrievalOutcome {
        let s = &req.settings;
        let scope = sources_in_scope(&req.sources, s.scope, &req.message.account_id);
        let instruction = req.instruction.as_deref().map(str::trim).filter(|t| !t.is_empty());
        let q = if scope.is_empty() {
            Questions { origin: QuestionOrigin::NoSourceInScope, questions: vec![], prompt_cut: false }
        } else {
            write_questions(self.writer, s, &req.message, instruction, req.budgets.question_prompt_chars)
        };

        let g = gather(self.index, &q.questions, &scope, clamp_count(s.excerpts_per_question, REPLY_EXCERPTS_CEILING));
        let mut dropped = g.dropped;
        let mut sized = Vec::new();
        for mut cand in g.candidates {
            let wanted: Vec<String> = cand.found_by.iter().flat_map(|&i| words(&q.questions[i])).collect();
            cand.found.text = size_text(&cand.found.text, s.excerpt_size, &wanted);
            if cand.found.text.is_empty() {
                dropped.push(Dropped { chunk_id: cand.found.chunk_id, reason: DropReason::EmptyAfterSizing });
            } else {
                sized.push(cand);
            }
        }
        let unique = dedupe(rank(sized), &mut dropped);
        let best = keep_first(unique, clamp_count(s.keep_n, REPLY_KEEP_CEILING), &mut dropped);
        let (after_clash, clashes) = apply_clash_rule(best, s.clash_rule, &mut dropped);
        let message = message_value(&req.message);
        let (filtered, ai) = ai_filter(self.judge, s.ai_filter_question.as_deref(), after_clash, &message, req.budgets.judge_prompt_chars, &mut dropped);
        let (context, context_text, context_cut) = assemble(filtered, &clashes, s.attached_fields, s.clash_rule, req.budgets.context_chars, &mut dropped);

        let trace = RetrievalTrace {
            origin: q.origin,
            question_prompt_cut: q.prompt_cut,
            questions: g.per_question,
            hits: g.hits,
            merged: g.merged,
            dropped,
            clashes,
            ai_filter: ai,
            kept: context.len() as u32,
            context_cut,
        };
        RetrievalOutcome { context, context_text, trace }
    }
}
