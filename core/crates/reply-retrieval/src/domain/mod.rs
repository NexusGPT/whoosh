//! The values retrieval reads and writes. The shared records (`RetrievalSettings`, `ReplySourcesRead`, `ReplyExcerpt`, `ReplyRetrievalReport`)
//! come from the contract; what is here is what only this step needs: the message as retrieval reads it, the budgets, what the index returns,
//! and the trace of every step (richer than the contract's report; `RetrievalTrace::report` gives the contract's one).
pub mod limits;
pub mod text;

use contracts::{ReplyExcerpt, ReplySourceRef, ReplySourcesRead, RetrievalSettings};

/// The message a reply answers, as retrieval reads it. `account_id` is the account the reply goes out from (the scope "this account").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyMessage {
    pub message_id: String,
    pub account_id: String,
    pub subject: String,
    pub body: String,
}

/// The sizes, in characters, of what each model reads: the limits the caller's providers declare, passed in (this crate types none).
/// `context_chars` is the room the writer's prompt leaves for the excerpts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budgets {
    pub question_prompt_chars: usize,
    pub judge_prompt_chars: usize,
    pub context_chars: usize,
}

/// One retrieval: the message, the owner's instruction (on demand, ⌘J; None for an auto draft), the Recherche settings, the Sources pane.
/// The scope read is `settings.scope` (the contract's one truth); `sources.scope` is its mirror and is not read.
#[derive(Debug, Clone, PartialEq)]
pub struct RetrievalRequest {
    pub message: ReplyMessage,
    pub instruction: Option<String>,
    pub settings: RetrievalSettings,
    pub sources: ReplySourcesRead,
    pub budgets: Budgets,
}

/// What one search asks the index: the question, the sources it may look in, how many excerpts at most.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExcerptQuery {
    pub text: String,
    pub sources: Vec<ReplySourceRef>,
    pub limit: u32,
}

/// One excerpt the index found: the contract's `ReplyExcerpt` fields plus the score the index ranked it by (the writer never sees the score).
#[derive(Debug, Clone, PartialEq)]
pub struct FoundExcerpt {
    pub chunk_id: String,
    pub source: ReplySourceRef,
    pub date_ms: Option<i64>,
    pub sender: Option<String>,
    pub folder: Option<String>,
    pub subject: Option<String>,
    pub text: String,
    pub score: f32,
}

impl FoundExcerpt {
    /// The record a reader of a draft or a try sees (no score).
    pub fn excerpt(&self) -> ReplyExcerpt {
        ReplyExcerpt {
            chunk_id: self.chunk_id.clone(),
            source: self.source.clone(),
            date_ms: self.date_ms,
            sender: self.sender.clone(),
            folder: self.folder.clone(),
            subject: self.subject.clone(),
            text: self.text.clone(),
        }
    }
}

/// Where the questions came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestionOrigin {
    /// the model wrote them
    Model,
    /// the model failed or wrote nothing usable, so the message subject was searched
    SubjectFallback { why: FallbackWhy },
    /// no enabled source is in scope: nothing was asked, nothing searched
    NoSourceInScope,
}

/// Why the questions fell back to the subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackWhy {
    /// the question model returned an error; its words
    ModelFailed { reason_words: String },
    /// it returned no text
    ModelSaidNothing,
    /// it returned text with no line that can be a question
    ModelGarbage,
}

/// What one question found: the excerpts the index gave (at most `excerpts_per_question`), or why it found none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionTrace {
    pub question: String,
    pub found: u32,
    /// the index returned an error for this question; its words (the other questions still ran)
    pub search_failed: Option<String>,
}

/// Why an excerpt found did not reach the writer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropReason {
    /// the index returned it from a source outside the scope (counted per hit: a leak of the index, never shown to the writer)
    OutsideScope,
    /// once sized, it says what a better-ranked excerpt says (`of` = that excerpt's chunk)
    Duplicate { of: String },
    /// nothing was left once the quoted lines were removed
    EmptyAfterSizing,
    /// ranked below the `keep_n` best
    BeyondKeep,
    /// it clashes with an excerpt the clash rule kept instead (`kept` = that excerpt's chunk)
    Clash { kept: String },
    /// the AI filter answered no
    AiFilter,
    /// the writer's budget had no room left for it (the lowest-ranked go first)
    NoRoom,
}

/// One excerpt that was found and left out, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dropped {
    pub chunk_id: String,
    pub reason: DropReason,
}

/// What the AI filter did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiFilterTrace {
    /// `ai_filter_question` is None
    Off,
    /// `judged` = answered yes or no; `dropped` = answered no; `unjudged` = no clear answer or an error, kept
    On { question: String, judged: u32, dropped: u32, unjudged: u32 },
}

/// One excerpt the writer is handed: its number to cite, the label line, the excerpt (text sized), the numbers of the kept excerpts it clashes with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextExcerpt {
    pub number: u32,
    pub label: String,
    pub excerpt: ReplyExcerpt,
    pub clashes_with: Vec<u32>,
}

/// What each step did ("Essayer sur un message" shows it). The numbers add up (`adds_up`): every hit the index returned is either folded into
/// another hit of the same chunk (`merged`), or dropped with a reason, or kept; and the questions' `found` sum to `hits`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrievalTrace {
    pub origin: QuestionOrigin,
    /// the question prompt was cut to fit its budget (the message, then the instruction, lost their ends)
    pub question_prompt_cut: bool,
    pub questions: Vec<QuestionTrace>,
    /// every excerpt the index returned, all questions together
    pub hits: u32,
    /// hits that were a chunk another question had already found
    pub merged: u32,
    pub dropped: Vec<Dropped>,
    /// pairs the clash rule found, as chunk ids, older first (with KeepBothDated both stay and their labels say so)
    pub clashes: Vec<(String, String)>,
    pub ai_filter: AiFilterTrace,
    pub kept: u32,
    /// the first excerpt alone was over the writer's budget and its text was cut
    pub context_cut: bool,
}

/// The result of one retrieval: what the writer is handed (the excerpts and the one text block), and the trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetrievalOutcome {
    pub context: Vec<ContextExcerpt>,
    /// the excerpts as the writer's `{{excerpts}}` reads them: a label line then the text, a blank line between, within `Budgets::context_chars`
    pub context_text: String,
    pub trace: RetrievalTrace,
}
