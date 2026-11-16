//! The numbers and the one fixed text this crate needs. Each is PROPOSED for docs/CONSTANTS.md (docs/lanes/reply-retrieval.md "Proposals"), not
//! written there. The budgets of the model calls are NOT here: the caller passes the limits its providers declare (`Budgets`).

/// A line the question model writes that is longer than this is the model rambling, not a search: it is not taken as a question.
/// Depends on: the questions the retrieval prompt asks for are "short" (the seed: "write up to 3 short questions").
pub const QUESTION_CHARS_MAX: usize = 200;

/// What the AI filter's model reads per excerpt. `{{question}}` = the owner's filter question (in the owner's words), `{{excerpt}}`, `{{message}}`.
/// English like the contract's other seeds; the i18n lane seeds the owner's language. Priority when the budget is short: question, excerpt, message.
pub const JUDGE_TEMPLATE: &str = "{{question}}\nAnswer yes or no.\n\nExcerpt:\n{{excerpt}}\n\nMessage:\n{{message}}";
