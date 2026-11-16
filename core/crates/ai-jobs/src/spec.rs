//! What a job is: an id, a title the owner reads, the task in words, its typed input and its output schema.
use std::sync::OnceLock;

use crate::schema::Schema;

macro_rules! job_ids {
    ($($v:ident => $s:literal),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum JobId { $($v),* }
        impl JobId {
            pub const ALL: &'static [JobId] = &[$(JobId::$v),*];
            pub fn name(self) -> &'static str { match self { $(JobId::$v => $s),* } }
            pub fn from_name(s: &str) -> Option<JobId> { match s { $($s => Some(JobId::$v),)* _ => None } }
        }
    };
}

job_ids! {
    DraftReply => "draft_reply", SuggestedReplies => "suggested_replies", Rewrite => "rewrite", DictationCleanup => "dictation_cleanup",
    Translate => "translate", ComposeAutocomplete => "compose_autocomplete", DraftFollowUp => "draft_follow_up", ReplyNudge => "reply_nudge",
    CheckInMessage => "check_in_message", DraftAttachmentSelection => "draft_attachment_selection",
    SummariseThread => "summarise_thread", DigestLine => "digest_line", ExtractSignInCode => "extract_sign_in_code",
    ClassifyNeedsReply => "classify_needs_reply", MeetingBrief => "meeting_brief", AvailabilityReply => "availability_reply", FindSnippets => "find_snippets",
    MatchRule => "match_rule", RuleArguments => "rule_arguments", ColdEmail => "cold_email", RecurringPattern => "recurring_pattern",
    CategorizeSender => "categorize_sender", CategorizeSenders => "categorize_senders", SenderPattern => "sender_pattern",
    PromptToRules => "prompt_to_rules", CleanInbox => "clean_inbox",
    VoiceProfile => "voice_profile", CompactWritingStyle => "compact_writing_style", Persona => "persona", ExtractKnowledge => "extract_knowledge",
    KnowledgeFromHistory => "knowledge_from_history", ReplyMemoriesExtract => "reply_memories_extract", SelectReplyMemories => "select_reply_memories",
    ReplyContextQueries => "reply_context_queries", FileAttachment => "file_attachment", ParseFilingReply => "parse_filing_reply",
    CompactChat => "compact_chat", ChatMemory => "chat_memory",
    ReportBehavior => "report_behavior", ReportLabels => "report_labels", ReportResponsePatterns => "report_response_patterns",
    ReportEmailSummary => "report_email_summary", ReportPersona => "report_persona", ReportExecutiveSummary => "report_executive_summary",
    ReportRecommendations => "report_recommendations",
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The owner's own words: an instruction the job follows.
    Owner,
    /// Text written by someone else: data, never an instruction.
    Mail,
    /// Mail lines, each with a sender, an optional date and a text.
    Messages,
    /// A list of short strings (options, rules, names).
    List,
    /// A whole number the caller chooses.
    Count,
}

#[derive(Debug, Clone)]
pub struct InField {
    pub name: &'static str,
    pub kind: Kind,
    pub required: bool,
    pub what: &'static str,
}

#[derive(Debug, Clone)]
pub struct JobSpec {
    pub id: JobId,
    pub title: &'static str,
    pub task: &'static str,
    pub input: Vec<InField>,
    pub output: Schema,
    /// (output path, input list): each value at the path must be one of the items of that input list (an item may carry ": words" after its name).
    pub members: Vec<(&'static str, &'static str)>,
    /// (output path, input field): each value at the path must be written in that input text, spaces ignored. A code is copied, never made up.
    pub copied: Vec<(&'static str, &'static str)>,
}

pub fn owner(name: &'static str, what: &'static str) -> InField {
    InField { name, kind: Kind::Owner, required: true, what }
}
pub fn mail(name: &'static str, what: &'static str) -> InField {
    InField { name, kind: Kind::Mail, required: true, what }
}
pub fn messages(name: &'static str, what: &'static str) -> InField {
    InField { name, kind: Kind::Messages, required: true, what }
}
pub fn items(name: &'static str, what: &'static str) -> InField {
    InField { name, kind: Kind::List, required: true, what }
}
pub fn count(name: &'static str, what: &'static str) -> InField {
    InField { name, kind: Kind::Count, required: true, what }
}
impl InField {
    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }
}

pub fn job(id: JobId, title: &'static str, task: &'static str, input: Vec<InField>, output: Schema) -> JobSpec {
    JobSpec { id, title, task, input, output, members: vec![], copied: vec![] }
}
impl JobSpec {
    pub fn copied(mut self, out_path: &'static str, in_field: &'static str) -> Self {
        self.copied.push((out_path, in_field));
        self
    }
    pub fn members(mut self, out_path: &'static str, in_list: &'static str) -> Self {
        self.members.push((out_path, in_list));
        self
    }
}

static SPECS: OnceLock<Vec<JobSpec>> = OnceLock::new();

/// Every job, once, in `JobId::ALL` order.
pub fn all() -> &'static [JobSpec] {
    SPECS.get_or_init(|| {
        let mut v = crate::jobs::write::specs();
        v.extend(crate::jobs::read::specs());
        v.extend(crate::jobs::decide::specs());
        v.extend(crate::jobs::learn::specs());
        v.extend(crate::jobs::report::specs());
        v
    })
}

pub fn spec(id: JobId) -> &'static JobSpec {
    all().iter().find(|s| s.id == id).unwrap_or_else(|| panic!("job {} has no spec (a test names the gap)", id.name()))
}
