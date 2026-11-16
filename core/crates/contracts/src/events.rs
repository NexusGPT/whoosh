//! The streams' payloads (docs/CAPABILITY-CONTRACT.md "Core events").
use crate::{AttachmentTransfer, ReplyDraft, ReplySourceRef, ReplySyncProgress, ActionFailed, DetectionProgress, TextRange, Counts, InboxRow, MailThread, OutboxEntry, SyncState};

/// WHERE a search hit matched, so the row can draw its paperclip and the tint (delta 8, S1 board "Data that must exist").
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum MatchedIn {
    /// words of the subject; `ranges` are UTF-8 byte offsets into the subject, for the tint
    SubjectWords { ranges: Vec<TextRange> },
    /// the name of an attachment
    AttachmentName,
    /// text read out of an attachment (extracted text, OCR, scene labels, a transcript)
    AttachmentContent,
    /// no word matched: the meaning index found it
    MeaningOnly,
    /// SPEC A149: the sender holds the typed letters; UTF-8 byte offsets into the sender's display name and into the sender's
    /// address, for the bold (either may be empty)
    SenderWords { name_ranges: Vec<TextRange>, address_ranges: Vec<TextRange> },
    /// SPEC A149: the mail's text holds the typed letters; `snippet` is the passage around the first occurrence and `ranges` are
    /// UTF-8 byte offsets into `snippet`, for the bold (empty when the letters sit in the recipients only)
    TextWords { ranges: Vec<TextRange> },
    /// SPEC A150: none of the typed letters; its closeness stands clearly above this search's average (lane rank: the lift of its best
    /// piece, in deviations from the mean closeness of the search, is at or above the gate); `snippet` is the passage of that piece and
    /// `attachment_name` the file's name when the piece is an attachment's
    Related,
}

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub message_id: String,
    pub score: f64,
    pub snippet: String,
    /// where the strongest match sits (delta 8)
    pub matched_in: MatchedIn,
    /// the attachment's file name when the match is inside one (`AttachmentName` or `AttachmentContent`); None otherwise
    pub attachment_name: Option<String>,
    /// for an attachment hit whose file is not fully read (not downloaded yet, unreadable), the words the owner reads
    /// ("not downloaded yet: only its name is searched"); None otherwise (delta 10, J2.8)
    #[uniffi(default = None)]
    pub attachment_note: Option<String>,
}

#[derive(uniffi::Enum, Debug, Clone, PartialEq)]
pub enum CoreEvent {
    InboxChanged { tab_id: String, rows: Vec<InboxRow> },
    CountsChanged { counts: Counts },
    ThreadChanged { thread: MailThread },
    SyncStateChanged { state: SyncState },
    OutboxTick { entry: OutboxEntry },
    SendCompleted { outbox_id: String, sent_at_ms: i64 },
    ActionFailed { failure: ActionFailed },
    SearchResults { query: String, hits: Vec<SearchHit> },
    AgentDelta { conversation_id: String, text: String },
    ProposalReady { proposal_id: String },
    ArrivalNotice { message_id: String },
    /// One model call of an Ask run finished; `words` says what the agent did ("Searched your mail for “quote”").
    AgentStep { conversation_id: String, number: u32, words: String },
    /// Nothing was sent: the owner is asked once per provider whether mail excerpts may go to it.
    AgentConsentNeeded { conversation_id: String, provider_id: String, provider_name: String },
    /// The run hit AGENT_MAX_STEPS without an answer.
    AgentStopped { conversation_id: String, steps: u32, words: String },
    /// One detection step finished (delta 8, J4.1): `number` counts from 1; the line says which step ran and whether it answered.
    /// Rides the SyncStatus stream.
    /// An app on the MCP server asked to do something that needs the owner's yes (delta 8, group 8). Answer with `AnswerMcpApproval`.
    McpApprovalNeeded { request_id: String, app_name: String, tool_name: String, request_words: String },
    DetectionProgress { address: String, number: u32, progress: DetectionProgress },
    /// Delta 20: the AI wrote a draft for this thread that was not there before (the auto use, or ⌘J). `draft_id` reads as state `reply/draft/<thread id>`.
    ReplyDraftReady { thread_id: String, draft_id: String },
    /// Delta 20: a draft of this thread changed (rewritten, taken into the composer, sent, discarded): the new row, whole.
    ReplyDraftChanged { thread_id: String, draft: ReplyDraft },
    /// Delta 20: a source's sync moved: its item count and where it stands (Settings, Réponses, Sources).
    ReplySourceProgress { source: ReplySourceRef, item_count: u64, progress: ReplySyncProgress },
    /// Delta 22: background AI work for the owner failed (a draft that could not be written, a change, a try, a voice analysis): the screen's pending state ends here.
    /// Which of the three ids is set says what failed. `reason_words` is what the owner reads; `reply_no_model` has its own refusal at the command, this is for work
    /// that failed AFTER the command was accepted.
    /// Delta 23: a received file's transfer moved: it landed (`OnThisMac`, `AttachmentFile.local_path` is set) or failed (`Failed`, in words). Fired at the end of a
    /// `DownloadAttachment` fetch; the tile re-reads `attachment/<id>/transfer`.
    AttachmentTransferChanged { attachment_id: String, message_id: String, transfer: AttachmentTransfer },
    ReplyWorkFailed { thread_id: Option<String>, draft_id: Option<String>, run_id: Option<String>, reason_words: String },
}
