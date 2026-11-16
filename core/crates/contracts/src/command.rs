//! The command API: everything the UI asks the core to do, as one closed enum, and the core
//! interface both implementations (the Rust core and the Swift FixtureCore) provide.
use std::sync::Arc;

use crate::{DecisionProviderKind, AttachmentSource, CalendarEvent, ReplyCalendarInfo, FixedNote, OnDemandOption, ReplyLabelScope, ReplySourceRef, ReplyTone, ReplyUseKind, RetrievalChange, ImportInto, AiReads, ChainPlacement, FlowCard, OAuthProvider, OAuthSignInState, AttachmentFile, InboxFilter, UnsubscribeMethod, Block, Mailbox, RuleSet, RuleSetCount, LabelSpec, Suggestion, Snooze, AccountConfig, DecidedBy, DetectionResult, ValidationReport, AccountIconSource, ActionFailed, AgentConfig, AgentProposal, AiProvider, AskAnswer, Contact, SenderDomain, DecisionModel, DecisionVerdict, McpGrant, Note, NoteTarget, SecretText, ShortcutPreset, Signature, SpeechEngine, SplitTab, SplitTabRule, SpeechEngineKind, CoreError, Counts, CoreEvent, MailLabel, MailProvider, MailThread, Message, NotchAlerts, NotchKind, NotchSetting, NotificationSetting, OutboxEntry, SmartFolder, SyncState};

/// "tomorrow morning" and its siblings are words; the core turns them into a return time
/// with the SNOOZE_* constants of docs/CONSTANTS.md.
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum SnoozeWhen { LaterToday, TomorrowMorning, NextWeek, At { at_ms: i64 } }

/// What kind of entry a page's list holds (the preview reads the page's own list: `Command::PreviewDecisionList`).
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewEntryKind { Rule, Model, Otherwise }

/// One entry of a decision page as the page holds it right now, ticked or not. `match_key` is the rule's key as the page names it ("vip", "list", "me", "code",
/// "reply", "from:<words>", "subject:<words>"; empty for a model or closing entry); `outcome` is the page's word for the answer (a tab name, "yes", "notify", a label
/// path with " › " between its names, "none"); `question` is the model entry's sentence as edited ("" for the others); `trust` is the model entry's level, 0 to 100.
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct PreviewEntry { pub kind: PreviewEntryKind, pub enabled: bool, pub match_key: String, pub outcome: String, pub question: String, pub trust: Option<u32> }

#[derive(uniffi::Enum, Debug, Clone, PartialEq)]
pub enum Command {
    Archive { message_ids: Vec<String> },
    Snooze { message_ids: Vec<String>, when: SnoozeWhen },
    Pin { message_id: String, pinned: bool },
    // `label_name` is a name or a path ("Clients > Provanta"). A single name that matches exactly one
    // existing label anywhere in the tree means that label; otherwise the path is read from the root.
    // With `create_if_missing`, every missing level is created; a path deeper than LABEL_DEPTH_MAX is
    // refused in words (`parse_label_path`).
    Label { message_ids: Vec<String>, label_name: String, create_if_missing: bool },
    MoveToAccount { message_ids: Vec<String>, account_id: String },
    Undo,
    MarkRead { message_ids: Vec<String>, read: bool },
    OpenThread { thread_id: String },
    QuickSend { thread_id: String, body: String },
    // Sends a saved draft as it stands (delta 12 serves it in the fixture core): its account, To, Cc, Bcc, subject and body
    // go out through the outbox with the undo window, exactly as QuickSend; `created_id` is the outbox entry and Z cancels
    // it. The draft leaves the draft list when it is queued and comes back when Z cancels. An unknown draft is NotFound;
    // a draft with no recipient and no reply thread is refused in words. This is the door for an EXPANDED quick reply
    // (edited To, Cc, Bcc, subject): UpdateDraft saves the fields, Send sends them.
    Send { draft_id: String },
    CancelSend { outbox_id: String },
    Schedule { draft_id: String, at_ms: i64 },
    Search { query: String },
    SaveSmartFolder { name: String, rule_words: String },
    AddAccount { address: String, provider: MailProvider },
    Ask { agent_id: Option<String>, question: String },
    ApproveProposal { proposal_id: String, approve: bool },
    RenameSplitTab { tab_id: String, name: String },
    // Renames one label (added 2026-11-07, delta 6). `name` is ONE name, not a path; the paths of the label's
    // whole subtree follow. A name already used by a sibling, an empty name and a name holding ">" are refused in words.
    RenameLabel { label_id: String, name: String },
    // Moves a label, with everything under it, to a new parent; `parent_id` None = to the top level.
    // Refused in words when the label would go inside itself or its own subtree, when the deepest label of the
    // moved subtree would pass LABEL_DEPTH_MAX, or when the new parent already holds a label of that name.
    MoveLabel { label_id: String, parent_id: Option<String> },
    // Creates one label under `parent_id` (None = top level). Depth rule: the new label is at level
    // parent.depth + 1, and a level above LABEL_DEPTH_MAX (docs/CONSTANTS.md: 5) is refused in words
    // ("a label can be at most 5 levels deep; ... has 6"). `created_id` is the new label's id.
    CreateLabel { name: String, parent_id: Option<String> },
    // Deletes one message (the notch's Delete button). It leaves every list and count; there is no undo entry,
    // because a delete on the server is not undone from here.
    DeleteMessage { message_id: String },
    // Two contact cards become one: `keep_id` takes every address of `merge_id`, and `merge_id` no longer exists.
    MergeContacts { keep_id: String, merge_id: String },
    // The owner's "Allow" on the consent question: mail excerpts may go to this provider (once per provider, C-J5.4).
    // An off-device provider with no recorded yes makes `Ask` stop with `AgentConsentNeeded` before any mail is read;
    // ask again after this command (delta 7).
    GiveAgentConsent { provider_id: String },
    // Saves the notch settings whole. A button word outside mark-read, archive, reply, delete is refused in words (delta 7; reply added 2026-11-09 for the notch Reply button).
    SetNotchSetting { setting: NotchSetting },
    // Saves the notch's alert settings whole: one style per kind of mail, how long an alert stays, sound, the pictures switch
    // (owner's picks of 14 Nov 2026; contracts/src/notch_alerts.rs). No undo entry.
    SetNotchAlerts { alerts: NotchAlerts },
    // Saves one account's notification setting (on or off, and where it announces). An unknown account is refused.
    SetNotificationSetting { setting: NotificationSetting },
    // Mutes (`enabled` false) or unmutes one account, leaving where it announces as it was.
    SetNotification { account_id: String, enabled: bool },
    // Picks the account's icon from the library, or uploads an image (refused in words over ACCOUNT_ICON_MAX_BYTES).
    SetAccountIcon { account_id: String, source: AccountIconSource },
    // Sets the account's colour, "#RRGGBB"; anything else is refused in words.
    SetAccountColour { account_id: String, colour: String },
    // Saves a provider's API key after checking it with the provider; the key goes to the Keychain and is never read
    // back or echoed (state shows only `provider_credential/<id>/stored`). A refused key is named in words (delta 7). Answers AT ONCE (delta 22), in TWO STEPS (delta 24,
    // ai_provider_state.rs): step 1 publishes `ai_provider/<id>/state` = "checking" (a key check exists) or "not checked" (the key is kept, nothing can verify it: Jev);
    // step 2 lands "connected" (+ the models at `/models`) or "refused" (+ the provider's words at `/refusal`); a refused key stores nothing. Spellings: `ai_provider_state_key`.
    SaveProviderKey { provider_id: String, key: SecretText },
    // Forgets a provider's key (delta 22, ai-settings-core): the Keychain item and the provider's connected state go; an agent on it then reads "not connected". A
    // provider with no key is not an error; an unknown provider is NotFound. No undo entry.
    RemoveProviderKey { provider_id: String },
    // Saves an agent: an empty `id` creates one (`created_id`); a saved agent offers only the actions in `allowed`.
    SaveAgentConfig { config: AgentConfig },
    // Saves the decision model: its provider, the decisions it takes, its question words. Jev needs its key saved first.
    SaveDecisionModel { model: DecisionModel },
    // Picks the speech engine; the engine reports ready, or fails in words (a download that stopped).
    ChooseSpeechEngine { kind: SpeechEngineKind },
    // Runs the decision model on the DECISION_PREVIEW_N most recent messages and keeps the verdicts; changes no message.
    PreviewDecisions,
    // Runs ONE page's list as the page holds it (every entry, ticked or not, with an edited question) on the DECISION_PREVIEW_N most recent messages of the inbox
    // and keeps the verdict rows for `decision_preview()` (each row carries who decided in words, the sender and the subject of the mail, and the tab or label it
    // would land in). Changes no message and writes nothing: the lists saved in the store are not read, the one given is. `PreviewDecisions` above stays as it was.
    PreviewDecisionList { page_id: String, entries: Vec<PreviewEntry> },
    // Applies the kept preview: the rows it would move are placed by the model. Refused when there is no preview.
    ApplyDecisionPreview,
    // Writes the decision list of one page as the owner edited it, for the rules that file under a node of the label tree: `node_ids[i]` is the label whose
    // entry card gets `rules[i]` (one chain), so a rule of the owner may file under ANY node, also one whose AI card is off (the model is offered only the
    // labels whose AI card is on). Both lists have the same length; only the Labels page ("labels") files under nodes (another page is refused in words);
    // an unknown node is NotFound and nothing is written; a rule the node already holds is not written twice. The mail the rules catch is filed under
    // each node at once (its re-run, on the filing worker: progress on `label/<id>/rerun/*`). No undo entry. Read back by `label_rerun_count`.
    SaveDecisionList { page_id: String, node_ids: Vec<String>, rules: Vec<RuleSet> },
    // Allows or revokes one app on the MCP server; an app that is not allowed gets nothing.
    SetMcpGrant { app_name: String, allowed: bool },
    // Starts or stops the MCP server.
    SetMcpServer { on: bool },
    // Picks the shortcut preset ("Superhuman" or "GotEmail"); an unknown preset is refused in words (delta 7, Welcome).
    SetShortcutPreset { preset: String },
    // Puts the accounts in the owner's order: `ids` must hold every account exactly once, else it is refused.
    ReorderAccounts { ids: Vec<String> },
    // Adds a split tab after the last one; `created_id` is its id. A name that is empty or taken (any case) and a rule
    // that names nothing real (an unknown account or label, an address without "@", a domain with one) are refused
    // in words; `Builtin` is not a rule an owner can add (delta 7).
    AddSplitTab { name: String, rule: SplitTabRule },
    // Puts the split tabs in order: `ids` must hold every tab exactly once, else it is refused.
    ReorderSplitTabs { ids: Vec<String> },
    // Shows or hides one split tab (hiding removes it from the strip and the counts, not from the store).
    SetSplitTabEnabled { id: String, on: bool },
    // Back to the six default tabs in their order and names; tabs the owner added are removed.
    RestoreDefaultSplitTabs,
    // Writes a private note on a thread or one message; `created_id` is the note. Empty words and an unknown target are
    // refused in words. A note is never sent (delta 7, J8).
    AddNote { target: NoteTarget, text: String },
    // Replaces a note's words; the note keeps its place and its written time.
    EditNote { note_id: String, text: String },
    // Removes a note.
    DeleteNote { note_id: String },
    // The owner pressed Enter on a search: the words join the recent searches (newest first, one row per distinct words,
    // at most SEARCH_RECENT_MAX; state `search/recent`). Empty words are refused. Device-local, not synced (delta 8).
    RecordSearch { query: String },
    // Adds an account from a detected or hand-typed configuration (delta 8, J4.2). The password goes to the Keychain
    // through the secrets crate's port; the account row keeps only the reference, never the password. `created_id` is
    // the account. A password the server refused is not added: the error says which server and why. `AddAccount`
    // (address and provider only) stays for callers that have no configuration.
    // Rewrites a smart folder's rule (and, with `parent_id`, moves it; None = the top level). An unknown folder, an empty rule,
    // a parent that is the folder itself or does not exist are refused in words (delta 8, group 4).
    UpdateSmartFolder { folder_id: String, rule_words: String, parent_id: Option<String> },
    // Stars or unstars every message in the list (delta 8, group 5). State `message/<id>/starred`. No undo entry.
    Star { message_ids: Vec<String>, starred: bool },
    // Mutes or unmutes the threads of these messages: a muted thread leaves the inbox and stays out. State `message/<id>/muted`.
    Mute { message_ids: Vec<String>, muted: bool },
    // Marks (`vip` true) or unmarks the ADDRESS of this message's sender as a VIP: the VIPs view and its count follow (vip-mark). No undo entry.
    MarkVip { message_id: String, vip: bool },
    // Moves the messages to spam; they leave every list and count. State `message/<id>/spam`.
    MarkSpam { message_ids: Vec<String> },
    // The reverse of MarkSpam (delta 19): the messages leave Spam and come back to the inbox, and the spam filter learns them as not
    // junk. State `message/<id>/spam` reads false after. A message that is not in Spam is left as it is (the lesson is still kept).
    MarkNotSpam { message_ids: Vec<String> },
    // Saves a draft without sending it; `created_id` is the draft. `from_account_id` None = the reply thread's account, else the
    // first account. A draft with no recipient, no subject and no body is refused in words (delta 8, group 6, J3).
    SaveDraft { from_account_id: Option<String>, to: Vec<String>, cc: Vec<String>, subject: String, body: String, reply_to_thread_id: Option<String> },
    // Sends an outbox entry now instead of at the end of its undo window or its scheduled time (J3.8). An entry already sent or
    // cancelled is refused in words.
    SendNow { outbox_id: String },
    // Removes a split tab the owner added. The six default tabs cannot be removed (hide them with SetSplitTabEnabled).
    RemoveSplitTab { id: String },
    // The owner's answer to an `McpApprovalNeeded` prompt: allow or refuse this one request (delta 8, group 8).
    AnswerMcpApproval { request_id: String, allowed: bool },
    AddAccountConfigured { address: String, display_name: Option<String>, config: AccountConfig, password: SecretText },
    // Creates a label from the add-label sheet (delta 9; the lead's `createLabel(name, parent, colour, description, scope, rules,
    // pinned)`): `CreateLabel` above stays for callers that have a name and a parent only. `created_id` is the new label. Refused
    // in words by `validate_label_spec`. When the rule set has conditions, the label's rule is written with it (action Label).
    CreateLabelFull { spec: LabelSpec },
    // Replaces every editable field of a label with the sheet's values (delta 9): name, parent, colour, description, scope,
    // rules, pinned. "No colour", "no description" and "no rules" are values (`None`, `""`, an empty set), never "unchanged".
    // A built-in category refuses a rename and a new parent in words; its colour, description, rules and pin can change.
    UpdateLabel { label_id: String, spec: LabelSpec },
    // Drag in the sidebar: puts the label (with its sub-labels) under `new_parent_id` (None = top level) at `new_index` among its
    // siblings. The saved order is the tab order of the pinned labels. Refusals in words: `reorder_label`.
    ReorderLabel { label_id: String, new_parent_id: Option<String>, new_index: u32 },
    // Pins or unpins a label: a pinned label is a tab at the top of the list. The last pinned label cannot be unpinned (the list
    // always has a first tab), refused in words.
    PinLabel { label_id: String, pinned: bool },
    // Deletes an owner's label and the labels under it; the messages stay, they lose the label. A built-in category is refused in
    // words. No undo entry (delta 9).
    DeleteLabel { label_id: String },
    // Re-classifies the messages filed under a label after its description or rules changed (delta 9). The rules part runs now
    // and writes label_assignment with `decided_by` rule(rule_id); the meaning part belongs to the decision model and waits when
    // it is unavailable (state `label/<id>/rerun/model_pending`). Declining is simply not sending this command.
    RerunLabel { label_id: String },
    // Delta 17: switches one card of the label's flow on or off (Entry rules, AI sorting, Exit filter). Off keeps everything the card holds
    // (its chains, its inputs, the fallback): only the card stops working. Turning the exit filter ON with no fallback is refused in words
    // with code `exit_needs_fallback` (`check_set_card`). A view is refused as before (code `view_not_a_label`).
    SetLabelCard { label_id: String, card: FlowCard, enabled: bool },
    // Delta 17: what the AI reads for this label (subject, body, attachments). The three ticks are replaced whole. All three off is
    // refused in words with code `ai_reads_nothing` (`check_set_ai_reads`).
    SetLabelAiReads { label_id: String, reads: AiReads },
    // Delta 17: where the messages the exit filter rejects go: a label id, "primary" or "junk". `None` clears it and is refused while the
    // exit filter is on (same code `exit_needs_fallback`). Bad values are refused with code `bad_exit_fallback` (`check_set_exit_fallback`).
    SetLabelExitFallback { label_id: String, fallback: Option<String> },
    // Delta 17: adds a chain ("Autre règle") to a card. `created_id` is the chain's number (`next_chain_id`), as decimal words. The
    // chain is one rule record: its conditions and its match mode. Refused in words with code `bad_chain` (`validate_chain`).
    AddLabelChain { label_id: String, placement: ChainPlacement, chain: RuleSet },
    // Delta 17: replaces a chain whole (its conditions and its mode): the grid, the rows and the "Une / Toutes" choice are one value.
    // An unknown chain is `NotFound`; an empty or invalid chain is refused with code `bad_chain`; to drop a chain use `RemoveLabelChain`.
    UpdateLabelChain { label_id: String, placement: ChainPlacement, chain_id: u32, chain: RuleSet },
    // Delta 17: removes one chain. The other chains keep their numbers. Removing the last chain of a card leaves the card on with no chain
    // (the entry card then catches nothing; the exit card then keeps everything: `exit_verdict`).
    RemoveLabelChain { label_id: String, placement: ChainPlacement, chain_id: u32 },
    // Takes up a suggestion: writes what `suggestion_effect` says (a block, a rule, a mute, an unsubscribe) and marks it accepted.
    AcceptSuggestion { suggestion_id: String },
    // Drops a suggestion without writing anything.
    DismissSuggestion { suggestion_id: String },
    // Blocks a sender address: their mail goes to junk on arrival. Idempotent: a second block changes nothing.
    BlockSender { address: String },
    // Blocks a whole domain (and its sub-domains). Idempotent.
    BlockDomain { domain: String },
    // Opens a composer draft that answers everyone: the sender and every original recipient minus the owner's own addresses
    // (`reply_all_recipients`), "Re: " subject, the original quoted. Saved through the SaveDraft path; `created_id` is the draft. Delta 9, group 9.
    ReplyAll { message_id: String },
    // Opens a composer draft that forwards the message: no recipients, "Fwd: " subject, the original as the body with its attachments listed by name
    // (`forward_body`), AND (delta 22) every attachment of the original on the draft by default (`forward_attachments`: `StoredMessage` sources, fetched when the
    // message is built, never re-downloaded by hand; one over the per-file cap is listed `TooLarge`). The owner removes any with `RemoveDraftAttachment`; the name
    // list in the body is text and does not change when one is removed. `created_id` is the draft; state `draft/<id>/attachments` lists the files. Delta 9, group 9.
    Forward { message_id: String },
    // Renames a smart folder. An empty name, a name another folder has (any case) and an unknown folder are refused in words.
    RenameSmartFolder { folder_id: String, name: String },
    // Deletes a smart folder. The folders inside it move up to its parent (no mail is touched: a smart folder holds a rule, not messages).
    DeleteSmartFolder { folder_id: String },
    // Quick reply from a chosen account (the From selector, delta 10). `QuickSend` stays: it sends from the account that
    // received the thread. A UniFFI enum field cannot carry a default, so this is a second variant, not a new field.
    QuickSendFrom { thread_id: String, body: String, from_account_id: String },
    // Takes one label off these messages (the list's Y; delta 10). Every assignment of it goes, whoever filed it: the owner's
    // word wins over a rule's or the model's. Undoable (one entry). A label the messages do not carry is a no-op.
    RemoveLabel { message_ids: Vec<String>, label_id: String },
    // Takes every label off these messages (⇧Y). Undoable: Z brings each assignment back with its decider.
    RemoveAllLabels { message_ids: Vec<String> },
    // Deletes a selection with ONE undo entry for the whole batch (`DeleteMessage` stays for one message).
    DeleteMessages { message_ids: Vec<String> },
    // V: moves the messages to a label, i.e. labels them and archives them, as ONE action with ONE undo (addition 13).
    // Z takes the label off only where this added it and unarchives only what this archived.
    MoveToLabel { message_ids: Vec<String>, label_id: String },
    // Deletes a saved draft (the thread's draft card, Delete; delta 11). Undoable: Z brings the draft back with its id.
    DeleteDraft { draft_id: String },
    // Renames an account (its display name; delta 11). Trimmed; an empty name is refused in words. The address never changes.
    RenameAccount { account_id: String, name: String },
    // Pauses or resumes an account: a paused account fetches and sends nothing and its mail stays where it is (delta 11).
    PauseAccount { account_id: String, paused: bool },
    // Removes an account from this Mac. What it removes is `account_removal(account_id)`, which the UI states in the
    // confirm and again in the result; nothing is deleted on the server. Not undoable (the sign-in goes with it).
    RemoveAccount { account_id: String },
    // Sets an account's signature (delta 11). An empty text clears it: the account then signs nothing.
    SetSignature { account_id: String, text: String },
    // Mark not done (⇧E): the messages come back to the inbox. Undoable. State `message/<id>/archived` false.
    Unarchive { message_ids: Vec<String> },
    // Leaves the mailing list of this message by `unsubscribe_method` (one-click when offered, else the mail link, else
    // the page, which the UI opens). The UI asks the owner first. State `message/<id>/unsubscribed`.
    Unsubscribe { message_id: String },
    // Remind me if no reply by `at_ms` (⌘⇧H) on a draft or an outbox entry: when no reply in its thread arrived by then,
    // the message comes back to the top of the inbox. `at_ms` in the past is refused. State `reply_reminder/<draft_id>/{at_ms,fired}`.
    RemindIfNoReply { draft_id: String, at_ms: i64 },
    // Delta 18. Clears the reminder `RemindIfNoReply` set on this draft or outbox entry (the same id), or dismisses a fired one from
    // the Reminders view. Nothing to clear is refused in words (`reminder_nothing_to_clear`). State `thread/<id>/reminder` reads "none".
    ClearReplyReminder { draft_id: String },
    // Delta 18, import (PRD A83). Looks for mail stores on this Mac (Apple Mail, and any `.olm` or mbox file the owner picked: `picked_paths`,
    // from the file dialog; a path of another kind is refused in words) and writes state `import/sources`. Changes no mail. Safe to repeat.
    ImportScan { picked_paths: Vec<String> },
    // Starts importing one source of the last scan (`source_id` = `ImportSource.id`) into `into`. `created_id` = the run id; state
    // `import/<run id>/progress` then `/result`. One run at a time; an unreadable source, an unknown source or account are refused in
    // words (`check_import_start`). Imported mail is local only: nothing is sent to any server. Not undoable.
    ImportStart { source_id: String, into: ImportInto },
    // Stops a running import. What it already stored stays (a re-run skips it: the same message is one row). Not running = refused in words.
    ImportCancel { run_id: String },
    // Saves a reopened draft in place (delta 11, item 8): the draft keeps its id and its reply thread; recipients, subject and
    // body are replaced; `from_account_id` None keeps its account. No undo entry: editing a draft is not an action Z reverses
    // (Delete is). An unknown draft is NotFound; a draft left with nothing in it is refused in the SaveDraft words. `created_id`
    // is the same draft id; its `saved_at` moves to now. `bcc` (delta 12) replaces the draft's Bcc like the other lists.
    UpdateDraft { draft_id: String, from_account_id: Option<String>, to: Vec<String>, cc: Vec<String>, bcc: Vec<String>, subject: String, body: String },
    // Bring-your-own OAuth2 (delta 14): the client ID the person registered in THEIR OWN Microsoft or Google admin, for the
    // account being added (`account_draft_id` = `oauth_account_draft_id(address)`). Checked by `check_oauth_client_id` and
    // kept in the Keychain (never in the store or a state path). Whoosh ships no client ID of its own. A second call
    // replaces the first and forgets any sign-in made with the old one.
    SetOAuthClient { account_draft_id: String, provider: OAuthProvider, client_id: String },
    // Opens the provider's sign-in in the browser with PKCE and a loopback redirect on 127.0.0.1 (an ephemeral port), then
    // waits for the answer; `oauth_sign_in_state` reads where it stands. Refused in words with no client ID set. The
    // refresh token goes to the Keychain; the access token is refreshed before it expires (OAUTH_REFRESH_MARGIN_S).
    StartOAuthSignIn { account_draft_id: String },
    // ---- Delta 20: AI replies (docs/JOURNEYS.md J10; the pane each command serves is named). A refusal is `CoreError::Invalid { field: <code> }` in
    // English words (reply_ai_logic.rs). `kind` is the use: Auto (Brouillon automatique) or OnDemand (Sur demande, ⌘J).
    // Brouillon automatique and Sur demande: the master switch of a use. Off keeps every other setting.
    SetReplyUseEnabled { kind: ReplyUseKind, enabled: bool },
    // The line Ton: `tone_id` is "voice" (Ma voix) or a tone's id; an unknown tone is NotFound (`check_tone_choice`).
    SetReplyUseTone { kind: ReplyUseKind, tone_id: String },
    // The switch "Dans un fil, écrire comme mes réponses précédentes".
    SetReplyUseMatchThread { kind: ReplyUseKind, on: bool },
    // The line Prompt: which prompt the use reads (the one prompt is "replies"); an unknown prompt is NotFound.
    SetReplyUsePrompt { kind: ReplyUseKind, prompt_id: String },
    // Quels messages, the Étiquettes row of the entry card: all labels or exactly these (`check_reply_scope_labels`; code `reply_scope_empty`).
    SetReplyFilterScope { scope: ReplyLabelScope },
    // Quels messages, the switch of one of the three cards (Entry, Ai, Exit). Off keeps everything the card holds. The AI card ON with a blank
    // question is refused (code `reply_ai_needs_description`, `check_set_reply_card`).
    SetReplyFilterCard { card: FlowCard, enabled: bool },
    // Quels messages, the entry rule ("Et quand" Toutes or Une, the grid of facts, the text rows) replaced whole, as the label flow's chain is. An
    // EMPTY rule is a value: it passes everything. Refused with code `bad_reply_rule` (`validate_reply_rule`).
    SetReplyFilterEntryRule { rule: RuleSet },
    // Quels messages, the exit rule ("Garder quand"), replaced whole; same rules.
    SetReplyFilterExitRule { rule: RuleSet },
    // Quels messages, "L'IA lit": the four ticks replaced whole. All four off is refused (code `reply_reads_nothing`).
    SetReplyFilterAiReads { reads: AiReads },
    // Quels messages, the description under the AI card. Blank is refused while the AI card is on (code `reply_ai_needs_description`).
    SetReplyFilterAiDescription { description: String },
    // Sur demande, one of the two switches: the field before the draft (InstructionBefore), the field under it (ChangeAfter).
    SetOnDemandOption { option: OnDemandOption, enabled: bool },
    // Recherche and the line "chercher dans": ONE setting changes (`apply_retrieval_change`; code `bad_retrieval_setting`).
    SetRetrievalSetting { change: RetrievalChange },
    // Sources, a tile or a calendar switch: a folder of an account, a folder of this Mac, a calendar. Enabling a folder starts its background sync; the
    // item count and the progress arrive as `ReplySourceProgress` events and in state `reply/sources`. A source that does not exist is NotFound.
    SetReplySourceEnabled { source: ReplySourceRef, enabled: bool },
    // Sources, the tile "Ajouter un dossier": adds a folder of this Mac, enabled, from the path the owner picked (`check_local_folder_path`; code
    // `bad_local_folder`). `created_id` is the folder's source key. The same folder twice is one source.
    AddReplyLocalFolder { path: String },
    // Sources, removing a folder of this Mac: the source and its indexed excerpts go; the files on the disk are not touched.
    RemoveReplyLocalFolder { path: String },
    // Voix, the button Analyser: reads the last `sample_size` sent messages and writes the tone text (`check_voice_sample_size`; code `bad_voice_sample`).
    // Progress is state `reply/voice` `analysis`. A second run while one runs is refused in words.
    RunVoiceAnalysis { sample_size: u32 },
    // Voix, the tone text edited by hand (blank is a value: no tone yet).
    SetVoiceText { text: String },
    // Voix, "Joindre mes réponses à la même personne": 0 to REPLY_EXAMPLES_CEILING earlier replies (`check_same_person_examples`).
    SetVoiceSamePersonExamples { count: u32 },
    // Voix, "Autres tons": an empty `tone.id` creates one (`created_id` = its id), else replaces it (`check_reply_tone`; code `bad_reply_tone`).
    SaveReplyTone { tone: ReplyTone },
    // Voix, removing another tone. A use that chose it falls back to "voice". Unknown = NotFound.
    DeleteReplyTone { tone_id: String },
    // Le prompt, saving the text as a NEW version that becomes the current one (`check_reply_prompt_text`; code `bad_reply_prompt`). `created_id` =
    // the version number as decimal words. Versions are never edited or deleted.
    SaveReplyPromptVersion { prompt_id: String, text: String },
    // Le prompt, the line Versions: makes an earlier version the current one by saving its text as a NEW version (history only grows). `created_id` =
    // the new version number.
    RestoreReplyPromptVersion { prompt_id: String, version: u32 },
    // Le prompt, Notes fixes: an empty `note.id` creates one (`created_id` = its id), else replaces it (`check_fixed_note`; code `bad_fixed_note`).
    SaveFixedNote { note: FixedNote },
    // Le prompt, removing a fixed note.
    DeleteFixedNote { note_id: String },
    // ⌘J in a thread: drafts a reply on demand. `instruction` is the field before the draft (None or blank = none; refused with code
    // `reply_instruction_off` when that switch is off; `reply_no_model` when no model is set). `created_id` = the draft's id. A NEW draft fires `ReplyDraftReady`
    // (thread id, draft id) when its text exists; a REWRITTEN one (`RequestReplyChange`, accept, discard, sent) fires `ReplyDraftChanged`; one that could not be
    // written fires `ReplyWorkFailed` (the pending state of the screen ends there). A new draft replaces the
    // thread's live one (the old becomes Discarded). Nothing is sent.
    DraftReplyOnDemand { thread_id: String, instruction: Option<String> },
    // The field under a draft: asks the model to rewrite it. Refused when the draft is sent or discarded (code `reply_draft_closed`), when the change field
    // is switched off (code `reply_change_off`) and with no words (code `bad_reply_text`). The new text arrives as `ReplyDraftChanged`, revision + 1.
    RequestReplyChange { draft_id: String, text: String },
    // The owner keeps the draft: it becomes an ordinary composer draft (`created_id` = that draft's id) and the reply draft reads Edited. A closed or
    // already accepted draft is refused (code `reply_draft_closed`).
    AcceptReplyDraft { draft_id: String },
    // The owner throws the draft away: it reads Discarded and nothing is kept of it but the row. A closed draft is refused. Sends nothing, changes no message.
    DiscardReplyDraft { draft_id: String },
    // "Essayer sur un message": runs retrieval and the three steps on this message, changes nothing and stores no draft. `created_id` = the run id;
    // the result is state `reply/try/<run id>` (`ReplyTryReport`).
    TryReply { message_id: String, kind: ReplyUseKind },
    // ---- Delta 21: the shell pushes its calendars (docs/lanes/settingscalendars-mac.md "The seam to the core"). The core keeps the `enabled` flag of an id it knows,
    // starts a new one OFF, drops the ids that are gone, and keeps only the events of an enabled calendar inside the window (`apply_calendar_sync`; code
    // `bad_calendar_sync`). The shell reads which calendars are enabled from state `reply/calendars/enabled` BEFORE it reads events. `created_id` = None.
    // ---- Delta 22: attachments on a draft (why separate commands: reply_ai_d22.rs header). `Draft.attachment_ids` lists the ids of the draft's files.
    // Attaches one file to a SAVED draft: a file of this Mac (read when the message is built) or an attachment of a stored message. `created_id` = the attachment id
    // (the existing one when the same source is already on the draft: adding is idempotent). Refused in words (`plan_add_attachment`): `attachment_file_gone`,
    // `attachment_too_large`, `attachment_total_too_large`, `bad_attachment`, `attachment_draft_closed` (the draft was queued or sent). An unknown draft or stored
    // attachment is NotFound. No undo entry (editing a draft is not an action Z reverses, as UpdateDraft).
    // ---- Delta 23: received files. Fetches one received file in the background so a tile can open it. ANSWERS AT ONCE (<50 ms, a store read and a queued fetch): the file's
    // state is `attachment/<id>/transfer` ("not_on_this_mac", "downloading", "on_this_mac", "failed" + words), and `AttachmentTransferChanged` fires when it lands or fails.
    // Idempotent (a file already here or already coming is not an error); an unknown file is NotFound; one over ATTACHMENT_KEEP_MAX_BYTES is refused in words (code
    // `attachment_too_large_to_keep`). No view waits for the network: a tile reads the state, never the bytes.
    // Delta 23: the owner's yes (`given` true) or no to sending the mail text a label's AI card ticks to a HOSTED decision model (`decision_consent.rs` says exactly what it
    // covers). Stored per provider; the read is `WhooshCore::decision_consent` and state `decision_consent/<jev>`. A provider that stays on this Mac is refused (code
    // `decision_consent_not_needed`). `SaveDecisionModel` with a hosted provider and no consent is refused (code `decision_consent_needed`, `check_decision_model_consent`).
    GiveDecisionConsent { provider: DecisionProviderKind, given: bool },
    DownloadAttachment { attachment_id: String },
    AddDraftAttachment { draft_id: String, source: AttachmentSource },
    // Removes one file from a saved draft (a file of this Mac is not deleted from the disk). An unknown file is NotFound; a closed draft is refused as above.
    RemoveDraftAttachment { draft_id: String, attachment_id: String },
    SyncReplyCalendars { calendars: Vec<ReplyCalendarInfo>, window_start_ms: i64, window_end_ms: i64, events: Vec<CalendarEvent> },
    // ---- fix-teachcore (docs/lanes/hid-ai.md F6; docs/DECISION-MODEL.md:63 "a Test button that sends one fixed sample"): the Test button of a kept key.
    // Only "jev" has one (the other key providers are checked when their key is saved: SaveProviderKey). ANSWERS AT ONCE; the key is read from the Keychain at the
    // call and ONE fixed sample (never the owner's mail) goes through the decision crate's Jev client; state `ai_provider/<id>/state` reads "checking", then
    // "connected" when Jev answered it, or "refused" with Jev's own words at `ai_provider/<id>/refusal`. No key kept: refused in words, nothing sent. Saving the key
    // still sends nothing. Another provider id is refused in words; an unknown one is NotFound. `created_id` = None.
    TestProviderKey { provider_id: String },
    // fix-teachcore (docs/lanes/hid-ai.md F5; docs/DECISION-MODEL.md v2 §3 "Refit the model"): the owner's ask to refit the on-device model (Laya) on their corrections.
    // Refused in words for a hosted model (Jev has no refit: "Make a rule from these" is its fix), for rules only, and below DECISION_REFIT_EXAMPLES examples (state
    // `decision_teaching/examples`). An accepted ask is counted at state `decision_teaching/refits`; the answer says in words what this Mac can and cannot do
    // (engine decision_teaching.rs). No undo entry.
    RefitDecisionModel,
}

/// What a command produced. Each field is None when the command produces no such thing.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct CommandOutcome {
    pub undo_entry_id: Option<String>,
    pub created_id: Option<String>,
}

/// A span of text, in UTF-8 bytes from the start of the text it belongs to.
#[derive(uniffi::Record, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange { pub start: u32, pub end: u32 }

/// One message's body as the reader shows it (added 2026-11-07, messagebody lane). The HTML has
/// already been through the parse crate's sanitiser (allow-list, no script, remote images replaced
/// and counted); a plain-text message has `safe_html` None. Offsets are UTF-8 bytes.
/// Delta 26 (body-wire): the mail content engine's result (`parse::prepare_in_background`), computed ONCE when the body is
/// fetched, in the background, kept in the store with the body and read back when the mail opens (never computed on open).
/// A body stored before delta 26 (or still downloading) has every delta-26 field empty: `segments` [], `fold_from` None,
/// `layout` None, `layout_reasons` [], `display_html` None, `written_html` None, `body_warnings` []; the reader then shows
/// `safe_html` / `plain_text` as before. No longer `Eq` (a segment's confidence is a float).
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct MessageBody {
    pub message_id: String,
    pub safe_html: Option<String>,
    pub plain_text: String,
    /// remote images and pixels blocked in this message
    pub trackers_blocked: u32,
    /// where the quoted part starts in `safe_html`, when there is visible text before it
    pub quote_offset: Option<u32>,
    /// the quoted spans of `plain_text`
    pub quoted_ranges: Vec<TextRange>,
    /// delta 26: the message's content in order, each part classified (the sender's words, quoted history, a signature...).
    #[uniffi(default = [])]
    pub segments: Vec<BodySegment>,
    /// delta 26 (owner pick F2): the index in `segments` from which everything to the end is folded behind three dots; None = no fold.
    #[uniffi(default = None)]
    pub fold_from: Option<u32>,
    /// delta 26 (owner pick B3): `Designed` keeps its own paper (`display_html`), `Written` reads in the app's colours (`written_html`).
    /// None = the engine has not run on this body (stored before delta 26).
    #[uniffi(default = None)]
    pub layout: Option<BodyLayout>,
    /// delta 26: why the layout was chosen, closed words of the parse crate (`body_background`, `nested_tables`, ..., `no_design_signal`).
    #[uniffi(default = [])]
    pub layout_reasons: Vec<String>,
    /// delta 26: the whole message, cleaned and safe to show, the sender's style rules inlined (a designed mail draws this one).
    #[uniffi(default = None)]
    pub display_html: Option<String>,
    /// delta 26: for a `Written` mail, `display_html` without the sender's fonts, sizes, text colours and backgrounds; None for a designed one.
    #[uniffi(default = None)]
    pub written_html: Option<String>,
    /// delta 26: the engine's warnings in words (a charset read tolerantly, inlining skipped by a cap, the time limit...); [] = none.
    #[uniffi(default = [])]
    pub body_warnings: Vec<String>,
    /// delta 26 (engine round 6): remote PICTURES not loaded (each keeps its address in `data-whoosh-src` of `display_html`, never `src`).
    /// The view's line "N pictures not loaded, M trackers blocked" reads this and `pixel_trackers`; `trackers_blocked` still counts every
    /// remote image. 0 for a body the engine has not prepared.
    #[uniffi(default = 0)]
    pub pictures_blocked: u32,
    /// delta 26 (engine round 6): remote images that say they are invisible (a stated size under 3 px, or hidden): tracking pixels. 0 when not prepared.
    #[uniffi(default = 0)]
    pub pixel_trackers: u32,
}

/// Delta 26: one classified part of a message body, in order. `text` is exactly the engine's canonical text in `start..end`
/// (UTF-8 bytes of that canonical text: LF line ends, NFC; NOT offsets into `MessageBody.plain_text`). `safe_html` is safe to show.
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct BodySegment {
    pub kind: SegmentKind,
    pub text: String,
    pub safe_html: String,
    /// 0.0..=1.0
    pub confidence: f32,
    pub visibility: SegmentVisibility,
    /// closed words of the parse crate (`attribution_line`, `blockquote`, `signature_closing`, ...)
    pub reasons: Vec<String>,
    pub start: u32,
    pub end: u32,
}

/// Delta 26: what a body segment is.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SegmentKind { NewContent, InlineQuote, ReplyHistory, ForwardedContent, Signature, Disclaimer, Unknown }

/// Delta 26: how the reader shows a segment. `Hide` only on a structural anchor at high confidence; the sender's words always `Show`.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SegmentVisibility { Show, Collapse, Hide }

/// Delta 26 (owner pick B3): a designed mail keeps its paper, a written mail reads in the app's colours.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyLayout { Designed, Written }

/// One account as the lists show it (appshell's AccountBook, the sidebar, the iPhone filter): a read
/// projection of account + account-icon + sync-state + the unread count, derived at read, never stored.
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct AccountSummary {
    pub id: String,
    /// the display name ("Provanta RFP")
    pub name: String,
    pub address: String,
    pub provider: MailProvider,
    /// "#RRGGBB"
    pub colour: String,
    pub icon_id: Option<String>,
    pub unread: u32,
    /// None while healthy; the failure in words otherwise
    pub health: Option<ActionFailed>,
    /// the sidebar group the owner put the account in ("Work"); None = ungrouped (added 2026-11-07, delta 5).
    /// The defaults keep Swift callers written before delta 5 compiling; the core always fills all three.
    #[uniffi(default = None)]
    pub group: Option<String>,
    /// sync switched off by the owner: the account stays listed, nothing is fetched or sent for it
    #[uniffi(default = false)]
    pub paused: bool,
    /// share of the account's history already synced, 0.0..=1.0; None = not tracked (a new account that
    /// has not started). 1.0 = fully synced. Derived at read from the sync position, never stored twice.
    #[uniffi(default = None)]
    pub backfill: Option<f64>,
    /// the account's signature; "" = none (delta 11, SetSignature)
    #[uniffi(default = "")]
    pub signature: String,
    /// the owner's own image for the account: the local path `SetAccountIcon { Uploaded }` kept (delta 11, item 2b). None =
    /// no upload; the list shows the library glyph `icon_id`, which an upload never clears, so picking a library icon again
    /// sets this back to None and the glyph returns.
    #[uniffi(default = None)]
    pub icon_file: Option<String>,
}

/// What removing an account takes off this Mac (delta 11), read before RemoveAccount and stated in its result.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct AccountRemoval {
    pub account_id: String,
    pub account_name: String,
    pub messages: u32,
    pub drafts: u32,
    /// messages waiting to send from it
    pub outbox: u32,
    /// labels that belong to this account only
    pub labels: u32,
    /// the sentence the owner reads, for example "Removes Provanta RFP from this Mac: 12 messages, 1 draft and its sign-in.
    /// Nothing is deleted on the server."
    pub words: String,
}

/// The sentence for an account removal, once: the counts that are not zero, then the sign-in, then the server line.
#[uniffi::export]
pub fn account_removal_words(account_name: String, messages: u32, drafts: u32, outbox: u32, labels: u32) -> String {
    let plural = |n: u32, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let mut parts: Vec<String> = Vec::new();
    if messages > 0 { parts.push(plural(messages, "message", "messages")); }
    if drafts > 0 { parts.push(plural(drafts, "draft", "drafts")); }
    if outbox > 0 { parts.push(format!("{} waiting to send", outbox)); }
    if labels > 0 { parts.push(plural(labels, "label of its own", "labels of its own")); }
    parts.push("its sign-in".to_string());
    let list = match parts.len() {
        1 => parts[0].clone(),
        n => format!("{} and {}", parts[..n - 1].join(", "), parts[n - 1]),
    };
    format!("Removes {account_name} from this Mac: {list}. Nothing is deleted on the server.")
}

/// One saved draft as the thread shows it (delta 11): the draft card at the bottom of a thread, the row's draft mark.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct DraftSummary {
    pub id: String,
    pub thread_id: String,
    /// the account the draft will be sent from
    pub account_id: String,
    /// when it was last saved, milliseconds since the Unix epoch
    pub saved_at: i64,
    /// the start of the body: its first DRAFT_SUMMARY_LINES non-empty lines, joined by a newline
    pub first_lines: String,
    /// The draft's header, so a reopened draft restores it and not only its words (delta 12, quickreply round 5).
    /// Defaults keep Swift callers written before compiling; the cores always fill them.
    #[uniffi(default = [])]
    pub to: Vec<String>,
    #[uniffi(default = [])]
    pub cc: Vec<String>,
    #[uniffi(default = [])]
    pub bcc: Vec<String>,
    #[uniffi(default = "")]
    pub subject: String,
}

/// Which view of the inbox a list shows.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct InboxQuery {
    pub tab_id: String,
    /// None = every included account
    pub account_id: Option<String>,
    pub label_id: Option<String>,
    /// which mailbox the list shows; None (the default) is the inbox with its tabs, Some ignores `tab_id` (delta 9)
    #[uniffi(default = None)]
    pub mailbox: Option<Mailbox>,
    /// the accounts the list shows, as a SET (delta 10): empty = every account, else exactly these. Canonical; the older
    /// `account_id` is read only when this is empty (`query_accounts`).
    #[uniffi(default = [])]
    pub account_ids: Vec<String>,
    /// filters on the current list, all of which a row must pass (delta 10, ⇧U ⇧S ⇧I ⇧R)
    #[uniffi(default = [])]
    pub filters: Vec<InboxFilter>,
    /// a saved search (delta 14): Some lists exactly what that smart folder's saved words find, newest first, and ignores
    /// `tab_id`, `mailbox` and the labels; the account set and the filters still apply. Its count is the unread rows of the
    /// same query, so the sidebar's figure and the list cannot disagree.
    #[uniffi(default = None)]
    pub smart_folder_id: Option<String>,
}

/// One list row: a projection of message + pin + label-assignment + account, derived at read,
/// never stored (a second store of a derived value drifts).
#[derive(uniffi::Record, Debug, Clone, PartialEq)]
pub struct InboxRow {
    pub message_id: String,
    pub thread_id: String,
    pub account_id: String,
    pub sender_name: String,
    pub subject: String,
    pub preview: String,
    pub received_at_ms: i64,
    pub unread: bool,
    pub pinned: bool,
    pub label_names: Vec<String>,
    pub attachment_count: u32,
    pub thread_count: u32,
    /// the sender's address ("orders@linea-supply-example.com"); added 2026-11-07, delta 6, so a row can open the
    /// sender's card without a second read. Default "" keeps Swift callers written before delta 6 compiling;
    /// the core always fills it.
    #[uniffi(default = "")]
    pub sender_address: String,
    /// who placed the row in its tab: the same enum MessageCategory carries; None when the row has no category
    /// yet (added 2026-11-07, delta 6). The row badge reads it and equals message_category.decided_by.
    #[uniffi(default = None)]
    pub decided_by: Option<DecidedBy>,
    /// gap-remind: true when a reminder ("Remind me if no reply") on this row's thread FIRED: its time came and no reply had arrived
    /// by then. The inbox list shows the thread again with a reminder chip. False for a waiting, an answered or no reminder. Default
    /// false keeps Swift callers written before this field compiling; the core fills it at every inbox read.
    #[uniffi(default = false)]
    pub reminder_fired: bool,
}

#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamKind { Inbox, Counts, Thread, SyncStatus, Outbox, Search, Agent }

/// Swift implements this to receive a stream.
#[uniffi::export(with_foreign)]
pub trait EventSink: Send + Sync {
    fn on_event(&self, event: CoreEvent);
}

/// The core, as every view sees it. The Rust core implements it over the store; the Swift
/// FixtureCore implements it over docs/design/fixtures/mac-main-sample.json. The views are
/// the same in both cases: only the launch flag differs.
#[uniffi::export(with_foreign)]
pub trait WhooshCore: Send + Sync {
    fn run(&self, command: Command) -> Result<CommandOutcome, CoreError>;
    fn inbox(&self, query: InboxQuery) -> Vec<InboxRow>;
    /// The list a query shows, up to `rows` rows. `inbox` serves the first page; the list asks here for more as the owner nears the end
    /// of what it holds (owner's store, 14 Nov 2026: a tab of 178 conversations showed 50, said "End of All", and Down stopped on the
    /// 50th). Reads the local database only, never the network.
    fn inbox_rows(&self, query: InboxQuery, rows: u32) -> Vec<InboxRow>;
    /// How this message's list can be left (delta 10); None = not from a mailing list. The UI shows it in the confirm.
    fn unsubscribe_method(&self, message_id: String) -> Option<UnsubscribeMethod>;
    /// The message's attachments with what the Mac needs to open them: `local_path` when downloaded (Quick Look, open,
    /// drag out), `size_bytes`, `mime_type` (delta 10). Empty for an unknown message.
    fn attachments(&self, message_id: String) -> Vec<AttachmentFile>;
    /// The saved drafts that reply in this thread, newest first (delta 11). Empty when there is none.
    fn drafts(&self, thread_id: String) -> Vec<DraftSummary>;
    /// What RemoveAccount would take off this Mac (delta 11); None for an unknown account.
    fn account_removal(&self, account_id: String) -> Option<AccountRemoval>;
    fn counts(&self) -> Counts;
    /// The thread with its message ids, oldest first (the design fixture's open thread carries real ids too, delta 6).
    fn thread(&self, thread_id: String) -> Option<MailThread>;
    /// One message as the reader header and the notch show it: from name and address, time, attachment names,
    /// the sign-in code if any, tracker count. None when the id is unknown (added 2026-11-07, delta 6).
    fn message(&self, message_id: String) -> Option<Message>;
    /// Every contact the search dropdown offers as a person: one entry per person with EVERY address, across accounts
    /// (name, addresses, mail count per account), best-known first (delta 8). `contact(address)` still answers one card.
    fn contacts(&self) -> Vec<Contact>;
    /// SPEC A153: the domains of the senders of RECEIVED mail that start with `prefix` (case and accents folded; a leading "@" is
    /// ignored), each with its number of different sender addresses and of mails, most mails first (ties: domain A to Z), at most
    /// 8 (SENDER_DOMAINS_MAX). An empty prefix answers nothing. The query text for the filter is `from:@<domain>`.
    fn sender_domains(&self, prefix: String) -> Vec<SenderDomain>;
    /// The contact card for an address (any address a merged card holds); None when no mail came from it.
    fn contact(&self, address: String) -> Option<Contact>;
    /// Every message from the contact's addresses, newest first, across accounts and mailboxes; each row
    /// carries its account_id (C-J2.7).
    fn contact_messages(&self, contact_id: String) -> Vec<InboxRow>;
    /// The list row of each of these messages, whatever folder holds it (SPEC A141, rows-engine): the search screen draws every
    /// hit with it. One row per id that names a stored message, in the order of the ids given; an id that names no stored message
    /// is skipped; a repeated id gives one row, at its first place. Each row is the message's own row, built as `inbox` builds it
    /// (its thread id and the count of its thread), for any folder (Inbox, Sent, Archive, a custom one), archived or not, snoozed
    /// or not: no tab, mailbox, account filter or page limit applies. Reads the local database only, never the network.
    fn rows_for_messages(&self, message_ids: Vec<String>) -> Vec<InboxRow>;
    /// How many messages a smart-folder rule written in words matches right now ("0 matches" before saving, C-J2.6).
    fn rule_match_count(&self, rule_words: String) -> u32;
    /// Every label, as flat rows in tree order (a parent before its children); the UI builds the tree from parent_id.
    fn labels(&self) -> Vec<MailLabel>;
    /// The pinned labels in tree order: the tab strip, left to right (delta 9). The tab ids of `counts().tabs` are these ids.
    fn pinned_labels(&self) -> Vec<MailLabel>;
    /// The live match count of a rule set over the local corpus (delta 9, the add-label sheet's "14 messages").
    fn rule_set_count(&self, rule_set: RuleSet) -> RuleSetCount;
    /// Whether the owner allowed the label AI's mail text to go to this decision provider (delta 23). False before any answer; false for a provider that needs none.
    fn decision_consent(&self, provider: DecisionProviderKind) -> bool { let _ = provider; false }
    /// The ids of the messages a rule set matches in the local corpus, newest first, at most `limit` (delta 20, labels-r5: the inspector draws its rows and per-rule counts from the core's facts).
    fn rule_set_matches(&self, rule_set: RuleSet, limit: u32) -> Vec<String>;
    /// How many messages are filed under a label now: the "Re-run on N filed messages" offer (delta 9).
    fn label_rerun_count(&self, label_id: String) -> u32;
    /// The suggestions on offer, newest first (delta 9); an accepted or dismissed one is not listed.
    fn suggestions(&self) -> Vec<Suggestion>;
    /// The blocked senders and domains, newest first (delta 9).
    fn blocks(&self) -> Vec<Block>;
    /// Every account in the owner's order, with its colour and icon (added 2026-11-07: the sidebar and
    /// the iPhone read the fixture file directly because the core had no accounts read).
    fn accounts(&self) -> Vec<AccountSummary>;
    /// Every smart folder in the owner's order, parents before children (added 2026-11-07, delta 5).
    fn smart_folders(&self) -> Vec<SmartFolder>;
    /// The message's body; None when the message is unknown or its body is not downloaded yet.
    fn body(&self, message_id: String) -> Option<MessageBody>;
    /// The finished answer of an Ask conversation (the words and the messages they rest on); None while the
    /// conversation has none, for instance after `AgentConsentNeeded` (delta 7).
    fn answer(&self, conversation_id: String) -> Option<AskAnswer>;
    /// The proposals one Ask conversation made, oldest first; nothing in them has changed until approved (delta 7).
    fn proposals(&self, conversation_id: String) -> Vec<AgentProposal>;
    /// The notch settings as saved (delta 7); defaults before any save.
    fn notch_setting(&self) -> NotchSetting;
    /// The notch's alert settings as saved; the picked start values (`notch_alerts_default`) before any save.
    fn notch_alerts(&self) -> NotchAlerts;
    /// The kind of one stored mail, for the notch (`notch_kind_of` over the mail's facts); Primary for an id the store does not hold.
    fn notch_kind(&self, message_id: String) -> NotchKind;
    /// One notification setting per account, in account order (delta 7).
    fn notification_settings(&self) -> Vec<NotificationSetting>;
    /// Every AI provider with whether a key is saved and whether mail excerpts may go to it (delta 7).
    fn ai_providers(&self) -> Vec<AiProvider>;
    /// Every agent, the built-in ones first (delta 7).
    fn agent_configs(&self) -> Vec<AgentConfig>;
    /// The decision model as saved; None before the first save (delta 7).
    fn decision_model(&self) -> Option<DecisionModel>;
    /// The kept preview verdicts, newest message first; empty before a preview (delta 7).
    fn decision_preview(&self) -> Vec<DecisionVerdict>;
    /// The chosen speech engine and whether it is ready (delta 7).
    fn speech_engine(&self) -> SpeechEngine;
    /// Every app the MCP server knows with its grant (delta 7).
    fn mcp_grants(&self) -> Vec<McpGrant>;
    /// The split tabs in the owner's order, hidden ones included (delta 7).
    fn split_tabs(&self) -> Vec<SplitTab>;
    /// Messages that are snoozed now with the time each returns, soonest first (delta 8, group 5).
    fn snoozed_messages(&self) -> Vec<Snooze>;
    /// How many messages in the inbox a split-tab rule would hold, before the tab is saved (delta 8, group 8).
    fn split_tab_rule_match_count(&self, rule: SplitTabRule) -> u32;
    /// The notes written on exactly this target, oldest first (delta 7). Ask for the thread and for a message separately.
    fn notes(&self, target: NoteTarget) -> Vec<Note>;
    /// The signature the composer and quick reply append for an address; None when that address has none (delta 7).
    fn signature(&self, address: String) -> Option<Signature>;
    /// The shortcut preset in force (delta 7).
    fn shortcut_preset(&self) -> ShortcutPreset;
    /// Looks up the mail settings for an address in the owner's order (provider database, the domain's own file, DNS SRV,
    /// Autodiscover, a guess) and answers a configuration or a refusal in words. One `DetectionProgress` event per step
    /// rides the SyncStatus stream while it runs (delta 8, J4.1).
    fn detect_provider(&self, email: String) -> DetectionResult;
    /// Where the OAuth2 sign-in of an account being added stands (delta 14). `NeedsClientId` for an unknown draft.
    fn oauth_sign_in_state(&self, account_draft_id: String) -> OAuthSignInState;
    /// Checks a configuration with a real login: IMAP connect, login, list; SMTP ehlo, auth. Changes nothing and stores
    /// nothing; the password is used for this call only (delta 8, J4.2).
    fn validate_account(&self, config: AccountConfig, password: SecretText) -> ValidationReport;
    /// The test probe behind the socket verb `core state <path>`: the stored value at a path
    /// (for example `message/msg-01/archived`, `counts/primary`), JSON-encoded; None when absent.
    fn state(&self, path: String) -> Option<String>;
    /// Every outbox entry that is not yet sent or cancelled, oldest first (added 2026-11-07, spine lane).
    fn outbox(&self) -> Vec<OutboxEntry>;
    /// One sync state per account, in account order (added 2026-11-07, spine lane).
    fn sync_states(&self) -> Vec<SyncState>;
    /// Events reach a sink in commit order, from one serial queue per core; on subscribe the core
    /// sends the stream's current state first (Inbox, Counts, SyncStatus one per account, Outbox one
    /// OutboxTick per open entry: an empty outbox sends none, so read `outbox()` for the empty state).
    fn subscribe(&self, stream: StreamKind, sink: Arc<dyn EventSink>) -> u64;
    fn unsubscribe(&self, subscription: u64);
}
