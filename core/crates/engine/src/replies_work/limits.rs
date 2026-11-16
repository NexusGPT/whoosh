//! The numbers the draft engine runs on. Each is PROPOSED for docs/CONSTANTS.md (not written there: the ledger is not this lane's) and named once,
//! here, with what it depends on (docs/lanes/reply-engine.md "Proposals").

/// Characters of prompt one call to a CLOUD provider may carry (about 12,000 tokens at 4 characters a token): well inside the smallest context window
/// of the providers table's cloud models, and short enough that a draft answers in seconds.
pub const REPLY_CLOUD_PROMPT_CHARS: usize = 48_000;
/// Characters of prompt one call to a model on this Mac (Ollama, LM Studio) may carry: ON_DEVICE_CONTEXT_TOKENS (4096) × ON_DEVICE_CHARS_PER_TOKEN_FLOOR
/// (2.5), the pessimistic reading the decision crate uses for the on-device window.
pub const REPLY_LOCAL_PROMPT_CHARS: usize = 10_240;
/// The answer's ceiling for a draft or a rewrite (AGENT_MAX_OUTPUT_TOKENS, 1500, is the Ask answer's; a reply is shorter than a cited answer).
pub const REPLY_DRAFT_OUTPUT_TOKENS: u32 = 1_000;
/// The answer's ceiling for a yes/no decision, the questions, an excerpt's judgement.
pub const REPLY_SHORT_OUTPUT_TOKENS: u32 = 200;
/// The answer's ceiling for the voice analysis (three to five sentences).
pub const REPLY_VOICE_OUTPUT_TOKENS: u32 = 400;
/// One HTTP request to the provider: a draft from a cloud model takes seconds, more than AI_HTTP_TIMEOUT_MS (10 s, the labelling flow's short answer).
pub const REPLY_HTTP_TIMEOUT_MS: u64 = 60_000;
/// The pause before the one retry of a call the provider says can pass later (429, 5xx, no answer).
pub const REPLY_RETRY_PAUSE_MS: u64 = 500;
/// While the background pauses (battery, heat, the owner acting), the waiting work reads the pause again this often (CRAWL_RECHECK_MS is 5 s; a
/// shorter wait lets a paused arrival pass go on soon after the owner stops).
pub const REPLY_PAUSE_RECHECK_MS: u64 = 1_000;
/// Reports of "Essayer sur un message" kept in memory (the newest; a try is looked at once).
pub const REPLY_TRY_REPORTS_KEPT: usize = 20;
/// The language the voice analysis writes the tone in: the app's catalogue language (the Voix pane is French; "don't mix languages").
pub const REPLY_VOICE_LANGUAGE: &str = "French";

/// How the prompt's room is shared (fractions of the model's prompt characters): retrieval's question prompt, the judge's prompt, the excerpts the
/// writer is handed, the rest of the thread, the message body, the voice examples. The order kept when room is short is the model note's
/// (docs/design/AI-REPLIES-MODEL.md section 5): the ask, the mail, the thread, instructions, voice, knowledge, examples.
pub const SHARE_QUESTION: usize = 8;
pub const SHARE_JUDGE: usize = 8;
pub const SHARE_EXCERPTS: usize = 3;
pub const SHARE_THREAD: usize = 4;
pub const SHARE_BODY: usize = 4;
pub const SHARE_EXAMPLES: usize = 8;
