//! One type per docs/DATA-MAP.md record (column 1). The `records!` table at the bottom is the
//! registry `tests/records.rs` reads against DATA-MAP.md: a record added there without a type
//! here fails the test, and a type here with no row fails it too.
//!
//! Times are milliseconds since the Unix epoch (UTC); the UI formats them.
//! CREDENTIAL records hold a `KeychainRef`, never the secret: the secret cannot enter SQLite
//! or CloudKit because no type here has a field that could carry it.
use crate::notch_alerts::NotchAlerts;
use crate::{AiLabelNo, DecisionConsent, SourceChunk, OutgoingAttachment, BodyCrawlPlace, CalendarEvent, Chunk, ReplyCalendarInfo, AiReads, Block, ChainPlacement, FixedNote, FlowRejection, LabelKind, LabelScope, MatchMode, MessageFact, OnDemandOptions, ReplyDraft, ReplyFilter, ReplyPass, ReplyPrompt, ReplySource, ReplyTone, ReplyUse, ReplyVoice, RetrievalSettings, RuleAction, Suggestion};
use std::collections::HashMap;

/// A secret typed or pasted by the owner on its way to the Keychain (a provider API key). Its `Debug` prints no value,
/// so a command that carries one cannot leak it through a log line or a failed assertion; no stored record has a
/// field of this type (credential records hold a `KeychainRef`).
#[derive(uniffi::Record, Clone, PartialEq, Eq)]
pub struct SecretText { pub value: String }

impl std::fmt::Debug for SecretText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str("SecretText(hidden)") }
}

/// One line of the Decisions preview (C-J5.8): what the decision model would do with one of the recent messages.
/// `entry` is the 1-based entry of the page's list that decided it, the closing one included (None = no entry answered).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct DecisionVerdict {
    pub message_id: String,
    pub entry: Option<u32>,
    /// The tab, the yes or no, or the label path (words) the page would answer for the message; "—" when no entry answered.
    pub outcome: String,
    pub differs: bool,
    /// Who decided, in the page's words ("entry 2 · the model said Updates (0.91)"); "" from a core that does not say.
    #[uniffi(default = "")]
    pub decided_by: String,
    /// The sampled mail's sender and subject, as the preview row shows them; "" from a core that does not say.
    #[uniffi(default = "")]
    pub sender: String,
    #[uniffi(default = "")]
    pub subject: String,
}

/// Where a secret lives in the Keychain. The only shape a credential takes outside the Keychain.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct KeychainRef {
    pub service: String,
    pub account: String,
}

#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MailProvider { Google, Microsoft, Icloud, Fastmail, Imap }

#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordKind { State, File, Index, Credential }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct Account {
    pub id: String,
    pub address: String,
    pub provider: MailProvider,
    pub display_name: String,
    /// "#RRGGBB"
    pub colour: String,
    pub order: u32,
    pub paused: bool,
    /// optional grouping: Work, Personal, Clients, Side
    pub group: Option<String>,
    pub icon_id: Option<String>,
}

#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum AccountSecretKind { AppPassword, OauthToken }

#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct AccountCredential { pub account_id: String, pub kind: AccountSecretKind, pub keychain: KeychainRef }

#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq, Hash)]
pub enum AgentPermission { ReadMail, Search, Draft, Label, SendWithConfirmation }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    pub purpose: String,
    pub provider_id: String,
    pub model: String,
    pub instructions: String,
    pub allowed: Vec<AgentPermission>,
    pub built_in: bool,
}

/// One message an answer rests on, with what the citation chip shows (C-J5.4: account, sender, date) and the id the
/// reader opens (C-J5.5). The three added fields (delta 7, the agents crate's ask) carry UniFFI defaults so a Swift
/// caller written before still compiles; the core always fills them.
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct Citation {
    pub message_id: String,
    pub quote: String,
    #[uniffi(default = "")]
    pub account_id: String,
    #[uniffi(default = "")]
    pub sender_name: String,
    #[uniffi(default = 0)]
    pub received_at_ms: i64,
}

/// What an Ask turn hands back: words, and the messages they rest on. `Answer` carries at least one citation or says
/// nothing was found; `Draft` (the Writer) is marked as AI text until the owner edits or accepts it. Moved here from
/// the agents crate (delta 7) so Swift and Rust read one type.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnswerKind { Answer, Draft }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct AskAnswer { pub kind: AnswerKind, pub text: String, pub citations: Vec<Citation> }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct AgentTurn { pub from_owner: bool, pub text: String, pub citations: Vec<Citation>, pub at_ms: i64 }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct AgentConversation { pub id: String, pub agent_id: String, pub turns: Vec<AgentTurn> }

#[derive(uniffi::Enum, Debug, Clone, PartialEq)]
pub enum ProposalDecision { Pending, Approved { at_ms: i64 }, Rejected { at_ms: i64 } }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct AgentProposal {
    pub id: String,
    pub agent_id: String,
    pub action_words: String,
    pub message_ids: Vec<String>,
    pub label: Option<String>,
    pub decision: ProposalDecision,
    /// the draft a send proposal sends; None for a label proposal (delta 7)
    #[uniffi(default = None)]
    pub draft_id: Option<String>,
}

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct AiProvider {
    pub id: String,
    pub name: String,
    pub models: Vec<String>,
    pub connected: bool,
    pub keychain: Option<KeychainRef>,
    /// when the owner said yes to sending mail excerpts to this provider; None = never asked or said no.
    /// One consent per provider (C-J3.3, C-J5.4); on-device providers never need one (delta 7).
    #[uniffi(default = None)]
    pub mail_consent_at_ms: Option<i64>,
}

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct AttachmentFile {
    pub id: String,
    pub message_id: String,
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: u64,
    /// path inside the app container once downloaded; None until first open or drag
    pub local_path: Option<String>,
    /// delta 23: an INLINE part (a logo or picture the message body draws itself): a gallery leaves it out (`gallery_attachments`). Defaults keep Swift callers compiling.
    #[uniffi(default = false)]
    pub inline: bool,
    /// delta 23: the part's Content-ID, without the angle brackets, when it has one
    #[uniffi(default = None)]
    pub content_id: Option<String>,
}

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct Contact { pub id: String, pub name: String, pub addresses: Vec<String>, pub logo_id: Option<String>, pub mail_count_per_account: HashMap<String, u32> }

/// SPEC A153: one sender domain of the received mail: how many different sender addresses wrote from it and how many mails.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct SenderDomain { pub domain: String, pub people: u32, pub mails: u32 }

#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct DeviceLink { pub id: String, pub keychain: KeychainRef, pub expires_at_ms: i64 }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct AiMark { pub start: u32, pub end: u32, pub words: String }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct Draft {
    pub id: String,
    pub from_account_id: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub attachment_ids: Vec<String>,
    pub ai_marks: Vec<AiMark>,
    pub reply_to_thread_id: Option<String>,
    /// Bcc recipients (delta 12): they receive the message and no other recipient sees them (they go in the SMTP
    /// envelope, never in a header). Default empty keeps Swift callers written before compiling.
    #[uniffi(default = [])]
    pub bcc: Vec<String>,
}

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct MailLabel {
    pub id: String,
    /// this level's name ("Provanta")
    pub name: String,
    /// the label one level up; None at depth 1
    pub parent_id: Option<String>,
    pub colour: Option<String>,
    pub account_ids: Vec<String>,
    /// the names from the root down, joined by " > " ("Clients > Provanta"); added 2026-11-07.
    /// The defaults keep Swift callers written before nesting compiling; the core always fills both.
    #[uniffi(default = "")]
    pub path: String,
    /// 1..=LABEL_DEPTH_MAX (docs/CONSTANTS.md)
    #[uniffi(default = 1)]
    pub depth: u8,
    /// unread messages carrying THIS label, not its children (added 2026-11-07, delta 5). The sidebar rolls
    /// the children up from parent_id, to any depth; the core never stores a rolled-up figure, so a count
    /// cannot count twice. Default 0 keeps Swift callers written before delta 5 compiling.
    #[uniffi(default = 0)]
    pub unread: u32,
    /// the meaning the decision model reads (delta 9); "" = none. UniFFI cannot default an enum, so `scope` and `kind` have no
    /// default: the label constructors are the two in Sources/CoreBridge (checked 2026-11-08). The core always fills all five.
    #[uniffi(default = "")]
    pub description: String,
    /// a pinned label is a tab at the top of the list, in tree order (delta 9)
    #[uniffi(default = false)]
    pub pinned: bool,
    /// all mailboxes, or one account; the one truth (`account_ids` is the derived read)
    pub scope: LabelScope,
    /// built-in category (Rename and Delete refused) or an owner's label
    pub kind: LabelKind,
    /// position among its siblings, 0-based; the tab order follows the tree order
    #[uniffi(default = 0)]
    pub order: u32,
    /// delta 17, card 1 "Entry rules": on = a message matching one of the label's entry chains goes straight in, no AI. Default on:
    /// every label that had rules keeps them working (migration 0009).
    #[uniffi(default = true)]
    pub entry_enabled: bool,
    /// delta 17, card 2 "AI sorting": on = the decision model may file a message here from the description. Default on.
    #[uniffi(default = true)]
    pub ai_enabled: bool,
    /// delta 17, card 3 "Exit filter": on = what the AI sorted here must pass an exit chain; the rest goes to `exit_fallback`.
    /// Default off. Never on without a fallback (`check_set_card`, a trigger in migration 0009).
    #[uniffi(default = false)]
    pub exit_enabled: bool,
    /// what the AI reads of a message for this label (delta 17). At least one is on (`check_set_ai_reads`). A required field: UniFFI
    /// cannot default a record; the label constructors are the one in Sources/CoreBridge and the store's row reader.
    pub ai_reads: AiReads,
    /// where the messages the exit filter rejects go (delta 17): a label id, or "primary" (the view) or "junk" (the mailbox key).
    /// None = not chosen yet, legal only while `exit_enabled` is false.
    #[uniffi(default = None)]
    pub exit_fallback: Option<String>,
}

#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct LabelAssignment {
    pub label_id: String,
    pub message_id: String,
    /// The decision model's page entry that filed it ("labels/model"), when the model decided (delta 12, beside the
    /// store's decided_by; DecidedBy itself gains no arm). None for the owner, a rule, the default, or when not known.
    /// Readers that do not care ignore it.
    #[uniffi(default = None)]
    pub decided_entry_id: Option<String>,
}

#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct McpGrant { pub app_name: String, pub allowed: bool, pub granted_at_ms: Option<i64> }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct Message {
    pub id: String,
    pub thread_id: String,
    pub account_id: String,
    pub sender_name: String,
    pub from_address: String,
    pub to: Vec<String>,
    pub subject: String,
    pub preview: String,
    pub received_at_ms: i64,
    pub read: bool,
    pub archived: bool,
    pub folder: String,
    pub attachment_names: Vec<String>,
    pub login_code: Option<String>,
    pub trackers_blocked: u32,
    /// the List-Unsubscribe header's URLs (one-click https and/or mailto), in header order; empty = not a list message (delta 10)
    #[uniffi(default = [])]
    pub list_unsubscribe: Vec<String>,
    /// true when List-Unsubscribe-Post offered `List-Unsubscribe=One-Click` (RFC 8058)
    #[uniffi(default = false)]
    pub list_unsubscribe_one_click: bool,
    /// The first name to greet when From is an organisation and a person signed the mail ("Karen" for Linea Supply's
    /// "Karen Bosch, Linea Supply"); the quick reply's placeholder reads it. None when From is a person or no sign-off
    /// is found (delta 7). Default None keeps Swift callers written before compiling.
    #[uniffi(default = None)]
    pub reply_to_name: Option<String>,
    /// The Cc recipients as the message's header names them, in header order (delta 12). Empty = no Cc.
    #[uniffi(default = [])]
    pub cc: Vec<String>,
    /// The Bcc recipients (delta 12). A received message never shows its Bcc (the sender's server strips it), so this is
    /// filled only on the owner's own copies in Sent, where the provider keeps it. Empty = none known.
    #[uniffi(default = [])]
    pub bcc: Vec<String>,
    /// The owner sent it (delta 15): a reply from one of the owner's accounts, listed in its thread once it was sent. A
    /// row the inbox shows is never outgoing. The fixture core sets it when the undo window ends; the real core when the
    /// sync sees the Sent copy (core-send's door: match the copy to its thread by In-Reply-To / References).
    #[uniffi(default = false)]
    pub is_outgoing: bool,
}

/// Where a message was before a delete moved it (delta 11): its folder and its archived mark, so Z puts it back there.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct MessageOrigin { pub message_id: String, pub folder: String, pub archived: bool }

/// Who placed a message in its tab. Never empty: the type has no "unknown" arm. `Apple` (lane fix-aisort round 8): Apple's on-device language
/// model judged it (the Labels AI step); stored as "apple". `Model` (lane decided-model, SPEC A133): an on-device model of the catalogue other
/// than Laya judged it, named by its catalogue id (`Sources/Features/OnDeviceAI/Core/DecisionModelCatalogue.swift`, e.g. "knowline-0.8b");
/// stored as the on-device decider 'laya' with `decided_model` = the id, served as "model:<id>".
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum DecidedBy { Rule { rule_id: String }, Laya, Jev, Owner, Default, Apple, Model { model: String } }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct MessageCategory { pub message_id: String, pub tab_id: String, pub decided_by: DecidedBy, pub probability: Option<f64> }

/// What the notch's short list shows when the pointer rests on it (C-J6.3): the latest mail, or only unread.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotchList { Latest, Unread }

/// `show` is whether the notch is on; `list_shows` (delta 7) is what its short list holds. `buttons` are the words
/// "mark-read", "archive", "reply", "delete". No Swift or Rust caller builds a NotchSetting by hand, so the new field is required.
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct NotchSetting {
    pub show: bool,
    pub buttons: Vec<String>,
    pub hidden_in_screen_share: bool,
    pub follow_focus: bool,
    pub list_shows: NotchList,
}

/// Where an arriving mail announces itself, per account (GotMail 0.7.2): the notch, system notifications, or both.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationDelivery { Notch, Notifications, Both }

/// `enabled` false = muted: nothing announces for the account. `delivery` (delta 7) says where it announces when on;
/// None means "not chosen": the default, `Both`. (UniFFI cannot default an enum, and a Swift test builds this record
/// with two arguments, so the new field is optional.)
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct NotificationSetting {
    pub account_id: String,
    pub enabled: bool,
    #[uniffi(default = None)]
    pub delivery: Option<NotificationDelivery>,
}

/// Where one outgoing message stands. A Failed entry always carries the words the owner reads.
#[derive(uniffi::Enum, Debug, Clone, PartialEq)]
pub enum OutboxState {
    Undoable { until_ms: i64 },
    Scheduled { at_ms: i64 },
    Sending,
    Sent { at_ms: i64 },
    Failed { failure: crate::ActionFailed, attempts: u32 },
    Cancelled,
    /// A scheduled entry whose time passed OUTBOX_LATE_AFTER_MS ago with no device awake to send it
    /// (added 2026-11-07, outbox crate); it still goes out when a device wakes.
    Late { scheduled_at_ms: i64 },
    /// Due, but there is no network path; it goes when the path comes back (OnlinePort). Not a failure.
    WaitingForNetwork { since_ms: i64 },
}

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct OutboxEntry { pub id: String, pub draft: Draft, pub state: OutboxState, pub created_at_ms: i64 }

#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct Pin { pub message_id: String, pub pinned_at_ms: i64 }

#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ProviderCredential { pub provider_id: String, pub keychain: KeychainRef }

#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct PushRegistration { pub account_id: String, pub routing_id: String, pub keychain: KeychainRef }

#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct SearchHistory { pub query: String, pub at_ms: i64 }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct SettingsSync {
    pub devices: Vec<String>,
    pub last_sync_ms: Option<i64>,
    pub records_carried: Vec<String>,
    /// each paired device with its role for scheduled sends (added 2026-11-07); empty until paired
    #[uniffi(default = [])]
    pub paired: Vec<PairedDevice>,
}

/// Who sends a scheduled message: the Primary at its time; a Standby only after OUTBOX_STANDBY_AFTER_MS
/// if the Primary has not (outbox crate, docs/lanes/core-outbox.md item 6).
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceRole { Primary, Standby }

#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DevicePlatform { Mac, Iphone }

#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct PairedDevice { pub device_id: String, pub name: String, pub platform: DevicePlatform, pub role: DeviceRole }

/// The default role (lead's ruling, 2026-11-07): the Mac is Primary (stable power and network), the iPhone Standby.
#[uniffi::export]
pub fn default_device_role(platform: DevicePlatform) -> DeviceRole {
    match platform {
        DevicePlatform::Mac => DeviceRole::Primary,
        DevicePlatform::Iphone => DeviceRole::Standby,
    }
}

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct ShortcutPreset { pub preset: String, pub overrides: HashMap<String, String> }

#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct ShortcutUsage { pub action: String, pub mouse_uses: u32, pub key_uses: u32 }

#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct Signature { pub address: String, pub text: String }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct SmartFolder { pub id: String, pub name: String, pub rule_words: String, pub parent_id: Option<String> }

/// A snooze always has a return time: there is no "snoozed until unknown".
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct Snooze { pub message_id: String, pub return_at_ms: i64 }

#[derive(uniffi::Enum, Debug, Clone, PartialEq)]
pub enum SpeechEngineKind { Local, Cloud { provider_id: String } }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct SpeechEngine { pub kind: SpeechEngineKind, pub ready: bool }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct SyncState {
    pub account_id: String,
    pub history_fetched: bool,
    pub messages_indexed: u64,
    pub last_error: Option<crate::ActionFailed>,
    pub provider_status_words: Option<String>,
    pub last_synced_ms: Option<i64>,
}

#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct TextIndex { pub documents: u64, pub built_at_ms: Option<i64> }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct MailThread { pub id: String, pub account_id: String, pub subject: String, pub participants: Vec<String>, pub message_ids: Vec<String> }

/// What Z reverses. One entry per action (C-J1.2 invariant).
#[derive(uniffi::Enum, Debug, Clone, PartialEq)]
pub enum UndoAction {
    Archive { message_ids: Vec<String> },
    Snooze { message_ids: Vec<String> },
    Pin { message_id: String, was_pinned: bool },
    Label { message_ids: Vec<String>, label_id: String },
    Move { message_ids: Vec<String>, from_account_id: String },
    Send { outbox_id: String },
    /// Z after a label rename: the label takes `previous_name` back (delta 7).
    RenameLabel { label_id: String, previous_name: String },
    /// Z after a label move: the label and its subtree go back under `previous_parent_id` (None = the top level).
    MoveLabel { label_id: String, previous_parent_id: Option<String> },
    /// Z after a label was created: the label is removed again, with any assignment of it.
    CreateLabel { label_id: String },
    /// Z after a message delete: the message comes back where it was.
    DeleteMessage { message_id: String },
    /// Z after a note was written: the note is removed again (delta 8).
    NoteAdd { note_id: String },
    /// Z after a note was edited: the note takes `previous_text` back, keeping its place and written time.
    NoteEdit { note_id: String, previous_text: String },
    /// Z after a note was deleted: the note comes back with its id, target, text and written time.
    NoteDelete { note: Note },
    /// Z after a batch delete: every message of the batch comes back (ONE entry for the whole selection, delta 10) to the
    /// folder it came from with its archived mark (`origins`, delta 11: one per message, in `message_ids` order)
    DeleteMessages { message_ids: Vec<String>, origins: Vec<MessageOrigin> },
    /// Z after RemoveLabel: the label goes back on these messages (delta 10)
    Unlabel { message_ids: Vec<String>, label_id: String },
    /// Z after RemoveAllLabels: every removed assignment comes back, with its decider (delta 10)
    UnlabelAll { removed: Vec<LabelAssignment> },
    /// Z after MoveToLabel: the label leaves `labelled` (the messages that did not carry it) and `archived` (the messages
    /// that were in the inbox) come back
    MoveToLabel { label_id: String, labelled: Vec<String>, archived: Vec<String> },
    /// Z after DeleteDraft: the draft comes back whole, with its id and its saved time (delta 11)
    DeleteDraft { draft: Draft, saved_at: i64 },
    /// Z after Mute or Unmute (delta 19): every message in `message_ids` (the named messages' whole threads) loses the muted flag, then
    /// each id in `was_muted` (the ones that were muted BEFORE the command) takes it back: each message gets its earlier flag, whichever
    /// way the command went. No entry when no message changed.
    Mute { message_ids: Vec<String>, was_muted: Vec<String> },
    /// Z after Unarchive (delta 19): these messages (only the ones the command changed) are archived again.
    Unarchive { message_ids: Vec<String> },
}

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct UndoEntry { pub id: String, pub action: UndoAction, pub at_ms: i64, pub consumed: bool }

#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct VectorIndex { pub chunks: u64, pub dimension: u32, pub built_at_ms: Option<i64> }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct VoiceProfile { pub address: String, pub summary_words: String, pub built_at_ms: i64 }

#[derive(uniffi::Enum, Debug, Clone, PartialEq)]
pub enum SplitTabRule {
    Sender { address: String },
    Domain { domain: String },
    Account { account_id: String },
    Label { label_id: String },
    DecisionCategory { category: String },
    Builtin { name: String },
}

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct SplitTab { pub id: String, pub name: String, pub order: u32, pub rule: SplitTabRule, pub enabled: bool }

/// `Apple` (lane fix-aisort round 8): Apple's on-device language model, which the shell's port reports as `answered_by` when it judged the Labels
/// question; stays on this Mac (no consent), never a choice the owner saves (the Decisions pane offers Local, Jev and Rules).
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionProviderKind { Local, Jev, Rules, Apple }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct DecisionModel {
    pub provider: DecisionProviderKind,
    /// a model id, or a model file path for a local model
    pub model: Option<String>,
    /// the Jev API key lives in the Keychain; only its reference is here
    pub keychain: Option<KeychainRef>,
    pub decisions_enabled: Vec<String>,
    pub question_words: HashMap<String, String>,
    pub last_run_ms: Option<i64>,
}

#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum AccountIconSource { Library { icon_id: String }, Uploaded { file_path: String } }

#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct AccountIcon { pub account_id: String, pub source: AccountIconSource }

#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum SyncCursor {
    Imap { uid_validity: u64, highest_mod_seq: u64 },
    GraphDelta { token: String },
    GmailHistory { history_id: String },
    Jmap { state: String },
}

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct FolderSyncState { pub account_id: String, pub folder: String, pub cursor: Option<SyncCursor>, pub last_fetched_ms: Option<i64> }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct VipList { pub addresses: Vec<String>, pub domains: Vec<String> }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct Template { pub id: String, pub name: String, pub body: String, pub from_account_id: Option<String> }

#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackingKind { Open, Click }

#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct TrackingEvent { pub message_token: String, pub kind: TrackingKind, pub at_ms: i64 }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct LayoutState { pub window_id: String, pub pane_widths: Vec<f64>, pub collapsed: Vec<bool>, pub selected_mailboxes: Vec<String> }

#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum RuleCondition {
    From { words: String }, To { words: String }, Cc { words: String }, Domain { domain: String },
    SubjectContains { words: String }, BodyContains { words: String }, ListHeader { words: String },
    Account { account_id: String }, HasAttachment,
    /// delta 17: one of the eight yes/no facts of the facts grid with the value the message must have (`wanted`). "Any" is not a
    /// condition: a fact left on "—" has no row, so an illegal "any" condition cannot be built. Pièce jointe "yes" keeps the old
    /// `HasAttachment` spelling (one reading, `condition_fact`); "no" is `Fact { HasAttachment, wanted: false }`.
    Fact { fact: MessageFact, wanted: bool },
    /// Round 18 (owner, 13 Nov 07:30: "the actual rule (e.g., Domain) and condition (is/is not) separately"): the NEGATED twin of one text or identity condition.
    /// `Not { field: Domain, value: "x" }` is "Domain is not x"; `Not { field: SubjectContains, .. }` is "Subject does not contain". It holds exactly when the
    /// positive twin does not (`RuleCondition::positive_twin`), so a blank value, which the positive form never matches, is matched by every message: a row with a
    /// blank value is refused before it is written, like the positive ones (`condition_is_blank`).
    Not { field: RuleField, value: String },
}

/// The conditions that have a negated twin: the text and identity ones (not the attachment flag and not a fact, whose own "no" is `Fact { wanted: false }`).
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleField { From, To, Cc, Domain, SubjectContains, BodyContains, ListHeader, Account }

impl RuleField {
    /// The word a stored or served `{"kind":"not","field":...}` condition names its field by: the positive kind's own word.
    pub fn key(self) -> &'static str {
        match self {
            RuleField::From => "from", RuleField::To => "to", RuleField::Cc => "cc", RuleField::Domain => "domain", RuleField::SubjectContains => "subject",
            RuleField::BodyContains => "body", RuleField::ListHeader => "list_header", RuleField::Account => "account",
        }
    }
    pub fn from_key(k: &str) -> Option<RuleField> {
        Some(match k {
            "from" => RuleField::From, "to" => RuleField::To, "cc" => RuleField::Cc, "domain" => RuleField::Domain, "subject" => RuleField::SubjectContains,
            "body" => RuleField::BodyContains, "list_header" => RuleField::ListHeader, "account" => RuleField::Account, _ => return None,
        })
    }
}

impl RuleCondition {
    /// The positive condition a `Not` negates.
    pub fn positive_twin(field: RuleField, value: &str) -> RuleCondition {
        let w = value.to_string();
        match field {
            RuleField::From => RuleCondition::From { words: w },
            RuleField::To => RuleCondition::To { words: w },
            RuleField::Cc => RuleCondition::Cc { words: w },
            RuleField::Domain => RuleCondition::Domain { domain: w },
            RuleField::SubjectContains => RuleCondition::SubjectContains { words: w },
            RuleField::BodyContains => RuleCondition::BodyContains { words: w },
            RuleField::ListHeader => RuleCondition::ListHeader { words: w },
            RuleField::Account => RuleCondition::Account { account_id: w },
        }
    }
}

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct Rule {
    pub id: String,
    pub position: u32,
    pub conditions: Vec<RuleCondition>,
    /// the tab the rule files into when `action` is Tab; "" for the other actions
    pub tab_id: String,
    pub enabled: bool,
    /// what the rule does (delta 9); the default is the v1 meaning, a tab
    pub action: RuleAction,
    /// how the conditions combine (delta 9)
    pub match_mode: MatchMode,
    /// delta 17: which of the label's two rule cards this chain belongs to. A rule record IS one chain (conditions + match_mode);
    /// the chains of one (label, placement) are alternatives: any one that matches is enough, the conditions inside one follow its mode.
    pub placement: ChainPlacement,
    /// delta 17: the chain's number inside its (label, placement), unique there, 0 for the first (every rule written before delta 17
    /// is chain 0 of its label: migration 0009 numbers the rest by position). Names the chain in "why here".
    #[uniffi(default = 0)]
    pub chain_id: u32,
}

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct TrackerRule { pub pattern: String, pub blocked: bool, pub open_and_click_tracking_on_sent: bool }

#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct Reminder { pub id: String, pub message_id: String, pub when_ms: i64, pub note: String }

/// What a note hangs on: the whole thread (the default) or one message (the per-message setting).
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum NoteTarget { Thread { thread_id: String }, Message { message_id: String } }

/// A private note: never sent. Added 2026-11-07 with J8.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct Note { pub id: String, pub target: NoteTarget, pub text: String, pub written_at_ms: i64 }

/// The registry of record names: (DATA-MAP column 1, Rust type name, kind).
/// Each arm names its type, so a missing type is a compile error.
/// A mail action's server write-back not yet taken by the server (engine-act, migration 0011): the local change wins at once,
/// the server catches up when the worker takes the row. `kind`: set_archived · set_read · label · move · move_to_folder.
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct PendingAction { pub id: i64, pub account_id: String, pub message_id: Option<String>, pub folder: String, pub uid: Option<u32>, pub kind: String, pub payload_json: String, pub created_ms: i64, pub tries: u32, pub next_at_ms: i64, pub last_error: Option<String> }

/// The spam filter's one model per profile (junk-r2, migration 0012): the serialised token table and what it learned.
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct JunkModel { pub model_bytes: Vec<u8>, pub trained_junk: u32, pub trained_good: u32, pub updated_at_ms: i64 }

/// The spam filter's first-run training of one account (migration 0012): when, and how many of each class it learned.
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct JunkBootstrap { pub account_id: String, pub done_at_ms: i64, pub junk_learned: u32, pub good_learned: u32 }

/// One lesson the owner taught the spam filter about one message (migration 0012): junk or not_junk, learned once.
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct JunkLesson { pub message_id: String, pub label: String }

macro_rules! records {
    ($($name:literal => $ty:ty, $kind:ident;)*) => {
        pub const RECORDS: &[(&str, &str, RecordKind)] = &[$(($name, stringify!($ty), RecordKind::$kind)),*];
        #[allow(dead_code)]
        fn _every_record_is_a_type() { $(let _: Option<$ty> = None;)* }
    };
}

records! {
    "account" => Account, State;
    "account-credential" => AccountCredential, Credential;
    "agent-config" => AgentConfig, State;
    "agent-conversation" => AgentConversation, State;
    "agent-proposal" => AgentProposal, State;
    "ai-provider" => AiProvider, State;
    "attachment-file" => AttachmentFile, File;
    "contact" => Contact, State;
    "device-link" => DeviceLink, Credential;
    "draft" => Draft, State;
    "label" => MailLabel, State;
    "label-assignment" => LabelAssignment, State;
    "mcp-grant" => McpGrant, State;
    "message" => Message, State;
    "message-category" => MessageCategory, State;
    "notch-alerts" => NotchAlerts, State;
    "notch-setting" => NotchSetting, State;
    "notification-setting" => NotificationSetting, State;
    "outbox-entry" => OutboxEntry, State;
    "pin" => Pin, State;
    "provider-credential" => ProviderCredential, Credential;
    "push-registration" => PushRegistration, Credential;
    "search-history" => SearchHistory, State;
    "settings-sync" => SettingsSync, State;
    "shortcut-preset" => ShortcutPreset, State;
    "shortcut-usage" => ShortcutUsage, State;
    "signature" => Signature, State;
    "smart-folder" => SmartFolder, State;
    "snooze" => Snooze, State;
    "speech-engine" => SpeechEngine, State;
    "sync-state" => SyncState, State;
    "text-index" => TextIndex, Index;
    "thread" => MailThread, State;
    "undo-entry" => UndoEntry, State;
    "vector-index" => VectorIndex, Index;
    "voice-profile" => VoiceProfile, State;
    "split-tab" => SplitTab, State;
    "decision-model" => DecisionModel, State;
    "account-icon" => AccountIcon, State;
    "folder-sync-state" => FolderSyncState, State;
    "vip-list" => VipList, State;
    "template" => Template, State;
    "tracking-event" => TrackingEvent, State;
    "layout-state" => LayoutState, State;
    "rule" => Rule, State;
    "tracker-rule" => TrackerRule, State;
    "reminder" => Reminder, State;
    "note" => Note, State;
    "detection-cache" => DetectionCache, State;
    "message-location" => MessageLocation, State;
    "attachment-content" => AttachmentContent, Index;
    "suggestion" => Suggestion, State;
    "block" => Block, State;
    "flow-rejection" => FlowRejection, State;
    "ai-label-no" => AiLabelNo, State;
    "pending-action" => PendingAction, State;
    "junk-model" => JunkModel, State;
    "junk-bootstrap" => JunkBootstrap, State;
    "junk-lesson" => JunkLesson, State;
    "reply-use" => ReplyUse, State;
    "reply-filter" => ReplyFilter, State;
    "reply-on-demand" => OnDemandOptions, State;
    "reply-pass" => ReplyPass, State;
    "reply-source" => ReplySource, State;
    "retrieval-settings" => RetrievalSettings, State;
    "reply-voice" => ReplyVoice, State;
    "reply-tone" => ReplyTone, State;
    "reply-prompt" => ReplyPrompt, State;
    "fixed-note" => FixedNote, State;
    "reply-draft" => ReplyDraft, State;
    "reply-calendar" => ReplyCalendarInfo, State;
    "calendar-event" => CalendarEvent, State;
    "chunk" => Chunk, State;
    "body-crawl-place" => BodyCrawlPlace, State;
    "outgoing-attachment" => OutgoingAttachment, State;
    "source-chunk" => SourceChunk, State;
    "decision-consent" => DecisionConsent, State;
}

/// The record registry as UniFFI exports it, for the Swift contract-types check.
#[uniffi::export]
pub fn record_names() -> Vec<String> {
    RECORDS.iter().map(|(n, _, _)| (*n).to_string()).collect()
}

/// The last provider detection of a domain (DATA-MAP `detection-cache`, migration 0003): the configuration found, or the refusal in words.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct DetectionCache {
    pub domain: String,
    pub config: Option<crate::AccountConfig>,
    pub refused_words: Option<String>,
    pub detected_at_ms: i64,
}

/// One copy of a Gmail message in one label-folder (DATA-MAP `message-location`, migration 0003).
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct MessageLocation { pub account_id: String, pub folder: String, pub uid_validity: i64, pub uid: i64, pub message_id: String }

/// What was read out of one downloaded attachment on this device (DATA-MAP "One search", `attachment-content`; delta 8, group 3).
/// The text joins the word index, the labels and OCR text the same; `embedded` says its image, frame or audio vector is in the
/// vector index (the vector itself is not stored here). Not in the `records!` registry until the data map names its table.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct AttachmentContent {
    pub attachment_id: String,
    /// extracted text of a PDF or document
    pub text: Option<String>,
    /// OCR text of an image or a video frame
    pub ocr_text: Option<String>,
    /// scene labels of an image, or the sampled frames of a video
    pub labels: Vec<String>,
    /// speech in audio or video
    pub transcript: Option<String>,
    pub embedded: bool,
    /// the key the vector index files this attachment's vector under, beneath its message ("<message_id>/<attachment_id>");
    /// None = not embedded yet (delta 9). The vector crate consumes it.
    #[uniffi(default = None)]
    pub vector_key: Option<String>,
    pub updated_at_ms: i64,
}
