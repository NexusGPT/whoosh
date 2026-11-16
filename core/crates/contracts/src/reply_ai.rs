//! Delta 20 (docs/JOURNEYS.md J10, docs/PRD.md A109; the approved screens docs/design/boards/ai-replies-r7/designs.js): AI replies.
//! The records, the closed vocabularies and the commands' arguments. `reply.rs` (delta 9) keeps the words and recipients of Reply all and
//! Forward; it models no record, so nothing there is extended and nothing is duplicated. What IS reused, never re-modelled:
//!   - the label flow's rule types: an entry rule and an exit rule are a `RuleSet` over the nine conditions and the eight `MessageFact`s
//!     (label_flow.rs; one chain, "Une / Toutes" is `MatchMode`), and the three cards are `FlowCard`;
//!   - `AiReads` (subject, body, attachments) one record for the label AI and the reply AI (delta 20: it gained `rest_of_thread`);
//!   - `RuleCondition`, `MessageFacts`, `CoreError::Invalid { field: <code>, reason_words }` for every refusal, as delta 17.
//! Pure logic lives in `reply_ai_logic.rs`, the state JSON in `reply_ai_state.rs`. No sentence of the owner's language is kept here: refusals are English
//! words with a code, the catalogue holds the French.
use crate::{AiReads, RuleCondition, RuleSet};

// ---------------------------------------------------------------- the two uses

/// The two ways a reply gets drafted: alone when a message arrives (Brouillon automatique), or when the owner asks in a thread (Sur demande, ⌘J).
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReplyUseKind { Auto, OnDemand }

/// What the settings every use shares say (DATA-MAP `reply-use`): the master switch, the tone, the match-thread switch, the prompt.
/// `tone_id` is `REPLY_TONE_VOICE_ID` ("Ma voix") or the id of a `ReplyTone`; `prompt_id` names a prompt of `ReplyPrompt` (the one prompt is `REPLY_PROMPT_ID`).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ReplyUse { pub kind: ReplyUseKind, pub enabled: bool, pub tone_id: String, pub match_thread: bool, pub prompt_id: String }

/// The tone of "Ma voix": the text the voice analysis wrote (`ReplyVoice.tone_text`). Any other tone id is a `ReplyTone`.
pub const REPLY_TONE_VOICE_ID: &str = "voice";
/// The id of the one prompt the replies use (the Settings pane "Le prompt").
pub const REPLY_PROMPT_ID: &str = "replies";

/// Which messages the entry card looks at: every label ("Toutes" ticked) or exactly these. UniFFI cannot default an enum; the data cannot say
/// "all" and a list at once. An EMPTY list is refused by `check_reply_scope` and, when it arises (every ticked label was deleted), admits nothing.
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum ReplyLabelScope { All, Labels { label_ids: Vec<String> } }


/// The three cards of the page "Quels messages" (DATA-MAP `reply-filter`; the auto use only; on demand the owner picks the thread, no filter applies):
/// the entry filter (labels, then the rule), the AI decision (what it reads, its description), the exit filter (the rule). A card switched OFF is
/// SKIPPED: everything passes it (the default, named in the note). The rules are `RuleSet`s; an EMPTY rule passes everything (`reply_rule_passes`), which
/// is NOT the label flow's "an empty chain matches nothing": a filter with no condition has nothing to filter by.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ReplyFilter {
    pub scope: ReplyLabelScope,
    pub entry_enabled: bool,
    pub entry_rule: RuleSet,
    pub ai_enabled: bool,
    pub ai_reads: AiReads,
    /// the question the AI answers per message, in the owner's words ("the emails that wait for a reply from me ..."); never blank while the AI card is on
    pub ai_description: String,
    pub exit_enabled: bool,
    pub exit_rule: RuleSet,
}

/// The two switches of the page "Sur demande" (DATA-MAP `reply-on-demand`).
#[derive(uniffi::Record, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OnDemandOptions { pub instruction_before: bool, pub change_after: bool }

/// Which of the two switches a command sets.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnDemandOption { InstructionBefore, ChangeAfter }

// ---------------------------------------------------------------- the steps and their counts

/// What one step did with one message. `Skipped` = the card is switched off, so the message passed it untouched. `NotReached` = an earlier step stopped it.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReplyStepResult { Passed, Skipped, Stopped, NotReached }

/// DATA-MAP `reply-pass`: one row per message the auto use looked at (entry, AI, exit, and the draft made). The only stored fact behind the counts:
/// the figures of `ReplyStepCounts` are derived from these rows, never stored, so one arrival cannot be counted twice. The legal shapes are
/// `reply_pass_is_legal`: a step that stopped the message leaves every later step `NotReached`; a draft exists only when all three steps let it through.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ReplyPass {
    pub message_id: String,
    pub received_at_ms: i64,
    pub entry: ReplyStepResult,
    pub ai: ReplyStepResult,
    pub exit: ReplyStepResult,
    pub draft_id: Option<String>,
}

/// The figures of the auto pane over the last `days` days (the board: 128, 31, 12, 9, 9). Each is a subset of the one before it
/// (`count_reply_steps` builds them so); `drafts` = the drafts actually made, at most `after_exit`.
#[derive(uniffi::Record, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplyStepCounts { pub days: u32, pub received: u32, pub after_entry: u32, pub after_ai: u32, pub after_exit: u32, pub drafts: u32 }

/// The days the counts cover (the board: "Sur vos 7 derniers jours"). A number: PROPOSED for docs/CONSTANTS.md as REPLY_COUNTS_DAYS, not written there.
pub const REPLY_COUNTS_DAYS: u32 = 7;

// ---------------------------------------------------------------- sources

/// Where a source lives: a folder of a mail account, a folder of this Mac, a calendar. The data cannot say "a mail folder with no account".
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ReplySourceRef { MailFolder { account_id: String, folder: String }, LocalFolder { path: String }, Calendar { calendar_id: String } }

/// Where a source stands in the background sync that indexes it. `Syncing.percent` is 0..=100 (`sync_percent`).
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum ReplySyncProgress { NotStarted, Syncing { percent: u8 }, UpToDate, Failed { reason_words: String }, /** delta 24: the folder is no longer on the server (a LIST does not hold it); the row keeps its last count and is switched off */ Gone }

/// DATA-MAP `reply-source`: one source the owner can switch. `item_count` = messages (mail), files (folder of this Mac), events (calendar).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ReplySource { pub source: ReplySourceRef, pub enabled: bool, pub item_count: u64, pub progress: ReplySyncProgress }

/// One account's card on the Sources pane: its folders (the tiles) and the figures of its header ("4 dossiers sur 6 · 18 096 messages"), built by
/// `reply_account_sources` so the figures cannot disagree with the tiles.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ReplyAccountSources { pub account_id: String, pub folders: Vec<ReplySource>, pub enabled_folders: u32, pub total_folders: u32, pub enabled_items: u64 }

/// A calendar the Agendas pane connected, with its switch for the replies.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ReplyCalendar { pub calendar_id: String, pub name: String, pub account_label: String, pub colour: String, pub enabled: bool }

/// Where retrieval looks when it answers from an account: that account, or all of them (the line "Pour répondre depuis un compte, chercher dans").
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetrievalScope { ThisAccount, AllAccounts }

/// The whole Sources pane in one read (`reply/sources`): `enabled_total` counts every enabled source of the three kinds (derived, never stored).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ReplySourcesRead {
    pub accounts: Vec<ReplyAccountSources>,
    pub local_folders: Vec<ReplySource>,
    pub calendars: Vec<ReplyCalendar>,
    pub scope: RetrievalScope,
    pub enabled_total: u32,
}

// ---------------------------------------------------------------- retrieval

/// The size of one excerpt: a sentence, a paragraph, a whole section.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExcerptSize { Sentence, Paragraph, Section }

/// What clashes between two excerpts that contradict each other: keep both with their dates, keep the newest, keep the oldest.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClashRule { KeepBothDated, KeepNewest, KeepOldest }

/// What comes with each excerpt (the line "Joint à chaque extrait"). Four ticks, so a field cannot be named twice.
#[derive(uniffi::Record, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExcerptFields { pub date: bool, pub sender: bool, pub folder: bool, pub subject: bool }

/// DATA-MAP `retrieval-settings`: how the context of a reply is found (the pane "Recherche"). `ai_filter_question` None = no AI filter (an explicit
/// value: "no filter" is not "unchanged"); the scope lives in `ReplySourcesRead` too (one truth, here).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct RetrievalSettings {
    /// the prompt that writes the questions; its variables are `{{message}}` and `{{instruction}}`
    pub retrieval_prompt: String,
    pub questions_max: u32,
    pub excerpts_per_question: u32,
    pub excerpt_size: ExcerptSize,
    pub attached_fields: ExcerptFields,
    pub keep_n: u32,
    pub clash_rule: ClashRule,
    pub ai_filter_question: Option<String>,
    pub scope: RetrievalScope,
}

/// One change to one retrieval setting (the command `SetRetrievalSetting`): a setting is changed alone, so a pane that saves one control cannot
/// overwrite another that a second window changed.
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum RetrievalChange {
    Prompt { text: String },
    QuestionsMax { n: u32 },
    ExcerptsPerQuestion { n: u32 },
    ExcerptSize { size: ExcerptSize },
    AttachedFields { fields: ExcerptFields },
    KeepN { n: u32 },
    Clash { rule: ClashRule },
    AiFilterQuestion { question: Option<String> },
    Scope { scope: RetrievalScope },
}

/// One excerpt found for a reply: where it came from and what it says. Read-only; the chunk and its vector belong to the background indexer
/// (DATA-MAP "chunk", written by the indexer lane), this is what a reader of a draft or a try sees of it.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ReplyExcerpt {
    pub chunk_id: String,
    pub source: ReplySourceRef,
    pub date_ms: Option<i64>,
    pub sender: Option<String>,
    pub folder: Option<String>,
    pub subject: Option<String>,
    pub text: String,
}

// ---------------------------------------------------------------- voice and tones

/// DATA-MAP `reply-voice`: how the owner writes, as the Voix pane shows it. `tone_text` is what the analysis wrote and the owner may edit;
/// `analysed_at_ms` None = never analysed; `sample_size` = the sent messages the last analysis read; `same_person_examples` = how many earlier
/// replies to the same person join the prompt (0 = none). The older per-address `VoiceProfile` (delta 3) stays the Writer agent's summary; this is the
/// editable text the reply prompts read (open choice named in the note).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ReplyVoice { pub tone_text: String, pub sample_size: u32, pub analysed_at_ms: Option<i64>, pub same_person_examples: u32 }

/// DATA-MAP `reply-tone`: another tone beside "Ma voix" ("Formel", "Bref"). An empty `id` in `SaveReplyTone` creates one.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ReplyTone { pub id: String, pub name: String, pub text: String }

/// Where the voice analysis stands (`reply/voice/analysis`).
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum ReplyVoiceAnalysis { Idle, Running { read: u32, of: u32 }, Failed { reason_words: String } }

// ---------------------------------------------------------------- prompt and notes

/// DATA-MAP `reply-prompt`: one version of a prompt. Versions are append-only; exactly ONE per `prompt_id` is `current`. The text holds
/// `{{variable}}` placeholders (`reply_prompt_variables`) and lines starting with `//` the model never sees.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ReplyPrompt { pub prompt_id: String, pub version: u32, pub text: String, pub created_at_ms: i64, pub current: bool }

/// One variable the prompt can place (the list on the Prompt pane: message, thread, instruction, excerpts found, calendar, tone, fixed notes).
/// `id` is what the stored text holds between the braces; `name_key` and `meaning_key` are the catalogue keys of what the owner reads (the i18n lane).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ReplyPromptVariable { pub id: String, pub name_key: String, pub meaning_key: String }

/// DATA-MAP `fixed-note`: a note always joined to the prompt ("Notes fixes"). An empty `id` in `SaveFixedNote` creates one.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct FixedNote { pub id: String, pub title: String, pub text: String }

// ---------------------------------------------------------------- drafts

/// Who started the draft.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyMadeBy { Auto, OnDemand }

/// Where a draft stands. `Proposed` = made, not touched; `Edited` = the owner took it into the composer (`AcceptReplyDraft`); `Sent` and `Discarded` are
/// SEALED: nothing changes a draft in either (`check_reply_draft_open`, the store's trigger).
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReplyDraftState { Proposed, Edited, Sent, Discarded }

/// DATA-MAP `reply-draft`: the AI's draft of a reply, kept apart from the owner's own drafts until accepted. One LIVE (proposed or edited) draft per thread:
/// a new one replaces the old (the store's unique index). `excerpt_ids` = the chunks it rests on (`ReplyExcerpt.chunk_id`); `revision` counts the
/// changes asked after the first text (0 = as first written); `instruction` None = none was given.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ReplyDraft {
    pub id: String,
    pub thread_id: String,
    pub message_id: String,
    pub text: String,
    pub made_by: ReplyMadeBy,
    pub excerpt_ids: Vec<String>,
    pub instruction: Option<String>,
    pub state: ReplyDraftState,
    pub revision: u32,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

// ---------------------------------------------------------------- try on a message

/// What one step reported in a try: the verdict, the conditions that held (a rule step), and the model's one line (the AI step), in the model's words.
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct ReplyStepReport { pub result: ReplyStepResult, pub matched: Vec<RuleCondition>, pub model_note: Option<String> }

/// What retrieval did in a try: the questions it wrote, how many excerpts it found, which it kept, how many the AI filter dropped.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ReplyRetrievalReport { pub questions: Vec<String>, pub found: u32, pub kept: Vec<ReplyExcerpt>, pub dropped_by_ai_filter: u32 }

/// The answer of `TryReply` (state `reply/try/<run id>`): retrieval and the three steps, and the text a draft would have, when every step let the message
/// through. A try STORES NO `ReplyDraft` and changes no message: the owner looks, nothing is made.
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct ReplyTryReport {
    pub run_id: String,
    pub message_id: String,
    pub kind: ReplyUseKind,
    pub retrieval: ReplyRetrievalReport,
    pub entry: ReplyStepReport,
    pub ai: ReplyStepReport,
    pub exit: ReplyStepReport,
    pub draft_text: Option<String>,
}

// ---------------------------------------------------------------- refusal codes (the `field` of `CoreError::Invalid`)

pub const REPLY_SCOPE_EMPTY_CODE: &str = "reply_scope_empty";
pub const REPLY_BAD_RULE_CODE: &str = "bad_reply_rule";
pub const REPLY_READS_NOTHING_CODE: &str = "reply_reads_nothing";
pub const REPLY_AI_NEEDS_DESCRIPTION_CODE: &str = "reply_ai_needs_description";
pub const REPLY_BAD_TONE_CODE: &str = "bad_reply_tone";
pub const REPLY_BAD_PROMPT_CODE: &str = "bad_reply_prompt";
pub const REPLY_BAD_NOTE_CODE: &str = "bad_fixed_note";
pub const REPLY_BAD_SETTING_CODE: &str = "bad_retrieval_setting";
pub const REPLY_BAD_FOLDER_CODE: &str = "bad_local_folder";
pub const REPLY_BAD_SAMPLE_CODE: &str = "bad_voice_sample";
pub const REPLY_DRAFT_CLOSED_CODE: &str = "reply_draft_closed";
pub const REPLY_INSTRUCTION_OFF_CODE: &str = "reply_instruction_off";
pub const REPLY_CHANGE_OFF_CODE: &str = "reply_change_off";
pub const REPLY_BAD_TEXT_CODE: &str = "bad_reply_text";
