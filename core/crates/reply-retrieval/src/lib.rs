//! Whoosh AI replies, retrieval (docs/JOURNEYS.md J10.6 and J10.8 to J10.9; docs/design/AI-REPLIES-MODEL.md §3 and §4; the Recherche pane of
//! docs/design/boards/ai-replies-r7). One message and an optional instruction become the context a draft needs:
//!   1. questions: the retrieval prompt filled and cut to its budget, the model writes up to `questions_max` (`QueryWriter`); nothing usable
//!      from the model = one search on the message subject, named in the trace;
//!   2. a search per question in the enabled sources behind the account wall, `excerpts_per_question` each (`ExcerptIndex`); merged by chunk;
//!   3. each excerpt sized (sentence, paragraph, section), quoted history removed;
//!   4. ranked, de-duplicated, the `keep_n` best kept;
//!   5. the clash rule (keep both dated, newest, oldest);
//!   6. the AI filter, when its question is set (`ExcerptJudge`);
//!   7. a label line per excerpt from the four ticks, the whole within the writer's budget;
//!   and a trace of what each step did, whose numbers add up (`RetrievalTrace::adds_up`), with the contract's `ReplyRetrievalReport` from it.
//! Pure logic behind three ports; no store, no network, no engine. Every model call is cut to its budget, never failed.
//!
//! Third-party code: none.
pub mod domain;
pub mod ports;
pub mod usecases;

pub use domain::{
    AiFilterTrace, Budgets, ContextExcerpt, DropReason, Dropped, ExcerptQuery, FallbackWhy, FoundExcerpt, QuestionOrigin, QuestionTrace, ReplyMessage,
    RetrievalOutcome, RetrievalRequest, RetrievalTrace,
};
pub use ports::{ExcerptIndex, ExcerptJudge, QueryWriter};
pub use usecases::retrieve::Retriever;
