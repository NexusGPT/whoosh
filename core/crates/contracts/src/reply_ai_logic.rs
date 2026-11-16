//! Delta 20: the pure rules of AI replies. Every function here is what the Rust core, the store tests and the Swift FixtureCore all call, so a
//! rule is written once. Refusals are `CoreError::Invalid { field: <code from reply_ai.rs>, reason_words }` in English words, as delta 17.
use crate::*;

const MS_PER_DAY: i64 = 86_400_000; // a unit, not a product value
const KEY_SEP: char = '\u{1f}';

/// The ceilings of the Recherche pane's numbers. PROPOSED for docs/CONSTANTS.md (REPLY_QUESTIONS_CEILING and siblings), not written there: the lanes that
/// own the ledger decide. Each guards a typo (a thousand excerpts per question), not a product choice.
pub const REPLY_QUESTIONS_CEILING: u32 = 10;
pub const REPLY_EXCERPTS_CEILING: u32 = 20;
pub const REPLY_KEEP_CEILING: u32 = 50;
pub const REPLY_SAMPLE_CEILING: u32 = 1_000;
pub const REPLY_EXAMPLES_CEILING: u32 = 10;

fn refuse(code: &str, words: impl Into<String>) -> CoreError { CoreError::Invalid { field: code.into(), reason_words: words.into() } }
fn blank(s: &str) -> bool { s.trim().is_empty() }

/// The days the auto pane's counts cover: `REPLY_COUNTS_DAYS`, once, for the Swift side (a constant is not exported by UniFFI, so a screen or the fixture asks).
#[uniffi::export]
pub fn reply_counts_days() -> u32 { REPLY_COUNTS_DAYS }

// ---------------------------------------------------------------- the filter

/// Whether a message passes one reply rule (entry or exit). An EMPTY rule passes everything: a filter with no condition has nothing to filter by.
/// (The label flow's `rule_set_matches` says the opposite for an empty set, on purpose: a label with no rule files nothing by rule.)
#[uniffi::export]
pub fn reply_rule_passes(rule: RuleSet, facts: MessageFacts) -> bool { rule.conditions.is_empty() || rule_set_matches(rule, facts) }

/// Checks a reply rule before it is written. Empty is legal (it passes everything); otherwise the label flow's `validate_chain`, re-coded.
#[uniffi::export]
pub fn validate_reply_rule(rule: RuleSet) -> Result<(), CoreError> {
    if rule.conditions.is_empty() { return Ok(()); }
    validate_chain(rule).map_err(|e| match e { CoreError::Invalid { reason_words, .. } => refuse(REPLY_BAD_RULE_CODE, reason_words), other => other })
}

/// Whether the entry card's label ticks admit a message carrying these labels. `All` admits every message; a list admits one that carries any of
/// them; an empty list (every ticked label was deleted) admits nothing.
#[uniffi::export]
pub fn reply_scope_admits(scope: ReplyLabelScope, message_label_ids: Vec<String>) -> bool {
    match scope { ReplyLabelScope::All => true, ReplyLabelScope::Labels { label_ids } => label_ids.iter().any(|l| message_label_ids.contains(l)) }
}

/// Refuses a scope that ticks no label (to read every label, tick "Toutes": `All`) or a blank label id.
#[uniffi::export]
pub fn check_reply_scope(scope: ReplyLabelScope) -> Result<(), CoreError> {
    match scope {
        ReplyLabelScope::All => Ok(()),
        ReplyLabelScope::Labels { label_ids } if label_ids.is_empty() => Err(refuse(REPLY_SCOPE_EMPTY_CODE, "tick at least one label, or tick all labels")),
        ReplyLabelScope::Labels { label_ids } if label_ids.iter().any(|l| blank(l)) => Err(refuse(REPLY_SCOPE_EMPTY_CODE, "a ticked label has no id")),
        ReplyLabelScope::Labels { .. } => Ok(()),
    }
}

/// Refuses a scope that names a label that does not exist (`labels` = every label).
#[uniffi::export]
pub fn check_reply_scope_labels(scope: ReplyLabelScope, labels: Vec<MailLabel>) -> Result<(), CoreError> {
    check_reply_scope(scope.clone())?;
    if let ReplyLabelScope::Labels { label_ids } = scope {
        if let Some(missing) = label_ids.iter().find(|id| !labels.iter().any(|l| &l.id == *id)) {
            return Err(CoreError::NotFound { kind: "label".into(), id: missing.clone() });
        }
    }
    Ok(())
}

/// The default of the AI card: subject, body and the rest of the thread; not the attachments (what the board ticks).
#[uniffi::export]
pub fn default_reply_ai_reads() -> AiReads { AiReads { rest_of_thread: true, ..default_ai_reads() } }

/// Refuses an AI card that reads nothing (to stop the AI deciding, switch the card off).
#[uniffi::export]
pub fn check_reply_ai_reads(reads: AiReads) -> Result<(), CoreError> {
    if reads.subject || reads.body || reads.attachments || reads.rest_of_thread { return Ok(()); }
    Err(refuse(REPLY_READS_NOTHING_CODE, "the AI has to read at least one of: subject, body, rest of the thread, attachments. To stop the AI deciding, switch the card off"))
}

/// The AI question a fresh install starts with (English; the owner's language seeds it, the i18n lane). Pinned equal to the migration's seed by a test.
#[uniffi::export]
pub fn default_reply_ai_description() -> String {
    "Emails that wait for a reply from me: a question, a request, a decision that someone is waiting for.".to_string()
}

/// The filter a fresh install starts with: every label, the three cards on with empty rules (so they pass everything), the AI reading subject, body
/// and the rest of the thread with the default question. The master switch of the use is what keeps it quiet until the owner turns it on.
#[uniffi::export]
pub fn default_reply_filter() -> ReplyFilter {
    let empty = RuleSet { conditions: vec![], match_mode: MatchMode::All };
    ReplyFilter { scope: ReplyLabelScope::All, entry_enabled: true, entry_rule: empty.clone(), ai_enabled: true, ai_reads: default_reply_ai_reads(),
        ai_description: default_reply_ai_description(), exit_enabled: true, exit_rule: empty }
}

/// The two switches' defaults: both on (the field before and the field after the draft).
#[uniffi::export]
pub fn default_on_demand_options() -> OnDemandOptions { OnDemandOptions { instruction_before: true, change_after: true } }

/// Refuses writing a blank question while the AI card is on. (A blank one is fine while the card is off: nothing reads it.)
#[uniffi::export]
pub fn check_reply_ai_description(ai_enabled: bool, description: String) -> Result<(), CoreError> {
    if ai_enabled && blank(&description) { return Err(refuse(REPLY_AI_NEEDS_DESCRIPTION_CODE, "say what the AI should look for before you turn its card on")); }
    Ok(())
}

/// Refuses switching a card of the filter in words: the AI card ON needs its question. The other switches always pass; off keeps everything the card holds.
#[uniffi::export]
pub fn check_set_reply_card(filter: ReplyFilter, card: FlowCard, enabled: bool) -> Result<(), CoreError> {
    if card == FlowCard::Ai { check_reply_ai_description(enabled, filter.ai_description)?; }
    Ok(())
}

/// What the entry card does with a message: skipped when off; passed when the label ticks admit it and the rule passes it; else stopped.
#[uniffi::export]
pub fn reply_entry_step(filter: ReplyFilter, message_label_ids: Vec<String>, facts: MessageFacts) -> ReplyStepResult {
    if !filter.entry_enabled { return ReplyStepResult::Skipped; }
    if reply_scope_admits(filter.scope, message_label_ids) && reply_rule_passes(filter.entry_rule, facts) { ReplyStepResult::Passed } else { ReplyStepResult::Stopped }
}

/// What the AI card does, given the model's answer: skipped when off; `Some(true)` passes, `Some(false)` stops; no answer yet (the model was not
/// reachable) leaves the message `NotReached`, to be asked again, never silently passed or stopped.
#[uniffi::export]
pub fn reply_ai_step(filter: ReplyFilter, model_says_reply_wanted: Option<bool>) -> ReplyStepResult {
    if !filter.ai_enabled { return ReplyStepResult::Skipped; }
    match model_says_reply_wanted { Some(true) => ReplyStepResult::Passed, Some(false) => ReplyStepResult::Stopped, None => ReplyStepResult::NotReached }
}

/// What the exit card does with a message the AI let through: skipped when off; passed when its rule passes it; else stopped.
#[uniffi::export]
pub fn reply_exit_step(filter: ReplyFilter, facts: MessageFacts) -> ReplyStepResult {
    if !filter.exit_enabled { return ReplyStepResult::Skipped; }
    if reply_rule_passes(filter.exit_rule, facts) { ReplyStepResult::Passed } else { ReplyStepResult::Stopped }
}

fn through(r: ReplyStepResult) -> bool { matches!(r, ReplyStepResult::Passed | ReplyStepResult::Skipped) }

/// The legal shapes of a pass (the store's CHECK repeats them): the first step is always reached; a step that stopped the message, or has not
/// answered, leaves every later step `NotReached`; a draft exists only when all three steps let the message through.
#[uniffi::export]
pub fn reply_pass_is_legal(pass: ReplyPass) -> bool {
    let (e, a, x) = (pass.entry, pass.ai, pass.exit);
    if e == ReplyStepResult::NotReached { return false; }
    if !through(e) && (a != ReplyStepResult::NotReached || x != ReplyStepResult::NotReached) { return false; }
    if !through(a) && x != ReplyStepResult::NotReached { return false; }
    if pass.draft_id.is_some() && !(through(e) && through(a) && through(x)) { return false; }
    true
}

/// The figures over the last `days` days (a message counts when it arrived in `(now - days, now]`): received; let through the entry card; then the AI;
/// then the exit card; and the drafts made. Derived from the passes, never stored, so a message counts once: `ReplyPass.message_id` is the row's key.
/// A step counts only when every step before it let the message through, so each figure is a subset of the one before.
#[uniffi::export]
pub fn count_reply_steps(passes: Vec<ReplyPass>, now_ms: i64, days: u32) -> ReplyStepCounts {
    let from = now_ms - i64::from(days) * MS_PER_DAY;
    let mut c = ReplyStepCounts { days, received: 0, after_entry: 0, after_ai: 0, after_exit: 0, drafts: 0 };
    // one row per message: a message listed twice counts once, by its LAST row (the store replaces the row, never adds a second)
    let mut latest: Vec<&ReplyPass> = vec![];
    for p in &passes {
        match latest.iter().position(|q| q.message_id == p.message_id) { Some(i) => latest[i] = p, None => latest.push(p) }
    }
    for p in latest.into_iter().filter(|p| p.received_at_ms > from && p.received_at_ms <= now_ms) {
        c.received += 1;
        if !through(p.entry) { continue; }
        c.after_entry += 1;
        if !through(p.ai) { continue; }
        c.after_ai += 1;
        if !through(p.exit) { continue; }
        c.after_exit += 1;
        if p.draft_id.is_some() { c.drafts += 1; }
    }
    c
}

// ---------------------------------------------------------------- sources

/// The key a source is stored and addressed by, once: the kind, then its parts, separated by U+001F (no folder name or path holds it).
#[uniffi::export]
pub fn reply_source_key(source: ReplySourceRef) -> String {
    match source {
        ReplySourceRef::MailFolder { account_id, folder } => format!("mail{KEY_SEP}{account_id}{KEY_SEP}{folder}"),
        ReplySourceRef::LocalFolder { path } => format!("local{KEY_SEP}{path}"),
        ReplySourceRef::Calendar { calendar_id } => format!("calendar{KEY_SEP}{calendar_id}"),
    }
}

/// The source a key names; None for a key that is not one (a reader of a stored row refuses it in words, never guesses).
#[uniffi::export]
pub fn reply_source_from_key(key: String) -> Option<ReplySourceRef> {
    let parts: Vec<&str> = key.split(KEY_SEP).collect();
    match parts.as_slice() {
        ["mail", account_id, folder] if !account_id.is_empty() && !folder.is_empty() => Some(ReplySourceRef::MailFolder { account_id: account_id.to_string(), folder: folder.to_string() }),
        ["local", path] if !path.is_empty() => Some(ReplySourceRef::LocalFolder { path: path.to_string() }),
        ["calendar", id] if !id.is_empty() => Some(ReplySourceRef::Calendar { calendar_id: id.to_string() }),
        _ => None,
    }
}

/// Reads a folder path the owner picked: absolute (`/...`) or under home (`~/...`), no `..`, no NUL; a trailing slash is dropped. Refused in words with
/// `bad_local_folder`. Whether it exists is the core's to say (it asks the disk); this only says whether it can be a path.
#[uniffi::export]
pub fn check_local_folder_path(path: String) -> Result<String, CoreError> {
    let p = path.trim();
    if p.is_empty() { return Err(refuse(REPLY_BAD_FOLDER_CODE, "pick a folder")); }
    if p.contains('\0') { return Err(refuse(REPLY_BAD_FOLDER_CODE, "that folder name holds a character a path cannot hold")); }
    if !(p.starts_with('/') || p.starts_with("~/")) { return Err(refuse(REPLY_BAD_FOLDER_CODE, "a folder of this Mac starts at the root (/...) or at your home (~/...)")); }
    if p.split('/').any(|c| c == "..") { return Err(refuse(REPLY_BAD_FOLDER_CODE, "a folder path cannot go up with \"..\"")); }
    let t = p.trim_end_matches('/');
    Ok(if t.is_empty() { "/".to_string() } else { t.to_string() })
}

/// Refuses a source that cannot be one: a mail folder without its account or its name, a path that is no path, a calendar without an id.
#[uniffi::export]
pub fn check_source_ref(source: ReplySourceRef) -> Result<(), CoreError> {
    match source {
        ReplySourceRef::MailFolder { account_id, folder } if blank(&account_id) || blank(&folder) => Err(refuse(REPLY_BAD_FOLDER_CODE, "a mail folder needs its account and its name")),
        ReplySourceRef::LocalFolder { path } => check_local_folder_path(path).map(|_| ()),
        ReplySourceRef::Calendar { calendar_id } if blank(&calendar_id) => Err(refuse(REPLY_BAD_FOLDER_CODE, "a calendar needs its id")),
        _ => Ok(()),
    }
}

/// A progress figure as the `Syncing` percent: 0..=100, a bigger number is 100 (a core that over-counts cannot draw a ring past full).
#[uniffi::export]
pub fn sync_percent(done: u64, of: u64) -> u8 { if of == 0 { 0 } else { ((done.min(of) * 100) / of) as u8 } }

/// One account's card: the folders that belong to it (others are dropped), with the header's figures derived from them.
#[uniffi::export]
pub fn reply_account_sources(account_id: String, folders: Vec<ReplySource>) -> ReplyAccountSources {
    let mine: Vec<ReplySource> = folders.into_iter().filter(|s| matches!(&s.source, ReplySourceRef::MailFolder { account_id: a, .. } if *a == account_id)).collect();
    let on = || mine.iter().filter(|s| s.enabled);
    ReplyAccountSources { account_id: account_id.clone(), enabled_folders: on().count() as u32, total_folders: mine.len() as u32, enabled_items: on().map(|s| s.item_count).sum(), folders: mine.clone() }
}

/// The whole pane: the three groups and the figure "N enabled", counted over all three (so the line on the use panes cannot disagree with the tiles).
#[uniffi::export]
pub fn reply_sources_read(accounts: Vec<ReplyAccountSources>, local_folders: Vec<ReplySource>, calendars: Vec<ReplyCalendar>, scope: RetrievalScope) -> ReplySourcesRead {
    let enabled_total = accounts.iter().map(|a| a.enabled_folders).sum::<u32>()
        + local_folders.iter().filter(|s| s.enabled).count() as u32
        + calendars.iter().filter(|c| c.enabled).count() as u32;
    ReplySourcesRead { accounts, local_folders, calendars, scope, enabled_total }
}

/// THE scope rule, written once (delta 21 item 5): the sources a retrieval for mail of account `account_id` may read, each once. First this account's enabled
/// mail folders; then, only when `read.scope` is `AllAccounts`, the other accounts' enabled folders; then the enabled folders of this Mac and the enabled
/// calendars, which count for every account. Inside each group the order is the Sources pane's. A disabled source is never in scope.
#[uniffi::export]
pub fn sources_in_scope(read: ReplySourcesRead, account_id: String) -> Vec<ReplySourceRef> {
    let mut out: Vec<ReplySourceRef> = vec![];
    let mut add = |r: ReplySourceRef| if !out.contains(&r) { out.push(r); };
    let folders = || read.accounts.iter().flat_map(|a| a.folders.iter()).filter(|s| s.enabled);
    let mine = |s: &ReplySource| matches!(&s.source, ReplySourceRef::MailFolder { account_id: a, .. } if *a == account_id);
    for f in folders().filter(|s| mine(s)) { add(f.source.clone()); }
    if read.scope == RetrievalScope::AllAccounts { for f in folders().filter(|s| !mine(s)) { add(f.source.clone()); } }
    for l in read.local_folders.iter().filter(|s| s.enabled) { add(l.source.clone()); }
    for c in read.calendars.iter().filter(|c| c.enabled) { add(ReplySourceRef::Calendar { calendar_id: c.calendar_id.clone() }); }
    out
}

// ---------------------------------------------------------------- retrieval

/// What a fresh install retrieves with: three questions, five excerpts each, a paragraph, date + sender + folder + subject, the six best, both sides of a
/// clash kept with their dates, the AI filter on, this account. The prompt and the filter question are English seeds (the owner's language seeds them).
#[uniffi::export]
pub fn default_retrieval_settings() -> RetrievalSettings {
    RetrievalSettings {
        retrieval_prompt: "From {{message}} and {{instruction}}, write up to 3 short questions to find what I need to know before I reply.".to_string(),
        questions_max: 3, excerpts_per_question: 5, excerpt_size: ExcerptSize::Paragraph,
        attached_fields: ExcerptFields { date: true, sender: true, folder: true, subject: true },
        keep_n: 6, clash_rule: ClashRule::KeepBothDated,
        ai_filter_question: Some("Does this excerpt help to answer?".to_string()), scope: RetrievalScope::ThisAccount,
    }
}

fn in_range(n: u32, ceiling: u32, what: &str) -> Result<u32, CoreError> {
    if n == 0 || n > ceiling { return Err(refuse(REPLY_BAD_SETTING_CODE, format!("{what} is between 1 and {ceiling}"))); }
    Ok(n)
}

/// Applies one change to the settings and returns the new settings, or refuses in words with `bad_retrieval_setting`. The only writer of a retrieval setting.
#[uniffi::export]
pub fn apply_retrieval_change(settings: RetrievalSettings, change: RetrievalChange) -> Result<RetrievalSettings, CoreError> {
    let mut s = settings;
    match change {
        RetrievalChange::Prompt { text } => {
            check_prompt_variables(&text, &["message", "instruction"], REPLY_BAD_SETTING_CODE)?;
            s.retrieval_prompt = text;
        }
        RetrievalChange::QuestionsMax { n } => s.questions_max = in_range(n, REPLY_QUESTIONS_CEILING, "the number of questions")?,
        RetrievalChange::ExcerptsPerQuestion { n } => s.excerpts_per_question = in_range(n, REPLY_EXCERPTS_CEILING, "the number of excerpts per question")?,
        RetrievalChange::ExcerptSize { size } => s.excerpt_size = size,
        RetrievalChange::AttachedFields { fields } => s.attached_fields = fields,
        RetrievalChange::KeepN { n } => s.keep_n = in_range(n, REPLY_KEEP_CEILING, "the number of excerpts kept")?,
        RetrievalChange::Clash { rule } => s.clash_rule = rule,
        RetrievalChange::AiFilterQuestion { question } => match question {
            Some(q) if blank(&q) => return Err(refuse(REPLY_BAD_SETTING_CODE, "write the question, or switch the AI filter off")),
            q => s.ai_filter_question = q.map(|q| q.trim().to_string()),
        },
        RetrievalChange::Scope { scope } => s.scope = scope,
    }
    Ok(s)
}

// ---------------------------------------------------------------- voice and tones

/// A fresh install's voice: nothing written, never analysed, a hundred sent messages for the next analysis, three earlier replies to the same person.
#[uniffi::export]
pub fn default_reply_voice() -> ReplyVoice { ReplyVoice { tone_text: String::new(), sample_size: 100, analysed_at_ms: None, same_person_examples: 3 } }

/// Refuses an analysis of no message or of more than `REPLY_SAMPLE_CEILING`.
#[uniffi::export]
pub fn check_voice_sample_size(n: u32) -> Result<u32, CoreError> {
    if n == 0 || n > REPLY_SAMPLE_CEILING { return Err(refuse(REPLY_BAD_SAMPLE_CODE, format!("an analysis reads between 1 and {REPLY_SAMPLE_CEILING} sent messages"))); }
    Ok(n)
}

/// Refuses more than `REPLY_EXAMPLES_CEILING` earlier replies to the same person (0 is legal: none).
#[uniffi::export]
pub fn check_same_person_examples(n: u32) -> Result<u32, CoreError> {
    if n > REPLY_EXAMPLES_CEILING { return Err(refuse(REPLY_BAD_SAMPLE_CODE, format!("at most {REPLY_EXAMPLES_CEILING} earlier replies can join a draft"))); }
    Ok(n)
}

/// Refuses a tone with no name or no text, a name another tone has (any case), and the reserved id of "Ma voix". `existing` = every tone now.
#[uniffi::export]
pub fn check_reply_tone(tone: ReplyTone, existing: Vec<ReplyTone>) -> Result<(), CoreError> {
    if tone.id == REPLY_TONE_VOICE_ID { return Err(refuse(REPLY_BAD_TONE_CODE, "that id is the voice's own: edit the voice text instead")); }
    if blank(&tone.name) { return Err(refuse(REPLY_BAD_TONE_CODE, "give the tone a name")); }
    if blank(&tone.text) { return Err(refuse(REPLY_BAD_TONE_CODE, "describe the tone")); }
    if existing.iter().any(|t| t.id != tone.id && t.name.trim().eq_ignore_ascii_case(tone.name.trim())) { return Err(refuse(REPLY_BAD_TONE_CODE, "another tone has that name")); }
    Ok(())
}

/// Refuses a tone choice that is neither the voice nor an existing tone.
#[uniffi::export]
pub fn check_tone_choice(tone_id: String, tones: Vec<ReplyTone>) -> Result<(), CoreError> {
    if tone_id == REPLY_TONE_VOICE_ID || tones.iter().any(|t| t.id == tone_id) { return Ok(()); }
    Err(CoreError::NotFound { kind: "tone".into(), id: tone_id })
}

// ---------------------------------------------------------------- prompt and notes

/// The seven variables, in the order the pane lists them. Their words are catalogue keys `reply.var.<id>.name` and `reply.var.<id>.meaning`.
#[uniffi::export]
pub fn reply_prompt_variables() -> Vec<ReplyPromptVariable> {
    ["message", "thread", "instruction", "excerpts", "calendar", "tone", "notes"].iter()
        .map(|id| ReplyPromptVariable { id: id.to_string(), name_key: format!("reply.var.{id}.name"), meaning_key: format!("reply.var.{id}.meaning") }).collect()
}

/// How a variable is written in a stored prompt.
#[uniffi::export]
pub fn prompt_placeholder(id: String) -> String { format!("{{{{{id}}}}}") }

/// The variables a text places, in order of first use, each once. An unterminated `{{` is not a variable.
#[uniffi::export]
pub fn prompt_variables_used(text: String) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut rest = text.as_str();
    while let Some(i) = rest.find("{{") {
        let after = &rest[i + 2..];
        match after.find("}}") {
            Some(j) => { let id = after[..j].trim().to_string(); if !out.contains(&id) { out.push(id); } rest = &after[j + 2..]; }
            None => break,
        }
    }
    out
}

/// What the model reads of a text: the lines that start with `//` (a note for the owner) are dropped.
#[uniffi::export]
pub fn strip_prompt_notes(text: String) -> String {
    text.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n")
}

/// True when a `{{` has no `}}` after it, or a `}}` comes with no `{{` before it.
fn braces_unbalanced(s: &str) -> bool {
    let mut rest = s;
    loop {
        match (rest.find("{{"), rest.find("}}")) {
            (None, None) => return false,
            (Some(i), Some(j)) if i < j => rest = &rest[j + 2..],
            _ => return true,
        }
    }
}

fn check_prompt_variables(text: &str, allowed: &[&str], code: &str) -> Result<(), CoreError> {
    let seen = strip_prompt_notes(text.to_string());
    if blank(&seen) { return Err(refuse(code, "the prompt is empty: the model would read nothing")); }
    let used = prompt_variables_used(seen.clone());
    if braces_unbalanced(&seen) { return Err(refuse(code, "a variable is not closed: write it between {{ and }}")); }
    if let Some(bad) = used.iter().find(|id| !allowed.contains(&id.as_str())) { return Err(refuse(code, format!("\"{bad}\" is not a variable of this prompt"))); }
    Ok(())
}

/// Refuses a reply prompt that is empty once its note lines are dropped, that places an unknown or unclosed variable, or that never places the message.
#[uniffi::export]
pub fn check_reply_prompt_text(text: String) -> Result<(), CoreError> {
    let ids: Vec<String> = reply_prompt_variables().into_iter().map(|v| v.id).collect();
    let allowed: Vec<&str> = ids.iter().map(String::as_str).collect();
    check_prompt_variables(&text, &allowed, REPLY_BAD_PROMPT_CODE)?;
    if !prompt_variables_used(strip_prompt_notes(text)).iter().any(|v| v == "message") {
        return Err(refuse(REPLY_BAD_PROMPT_CODE, "the prompt has to place the message: add {{message}}"));
    }
    Ok(())
}

/// One value for one variable, for `render_reply_prompt`.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct PromptValue { pub id: String, pub value: String }

/// What the model receives ("Voir ce que le modèle reçoit"): the note lines dropped, each variable replaced by its value (a variable with no value is
/// replaced by nothing). An unknown placeholder is left as written (`check_reply_prompt_text` refuses it before it is saved).
#[uniffi::export]
pub fn render_reply_prompt(text: String, values: Vec<PromptValue>) -> String {
    let mut out = strip_prompt_notes(text);
    for v in reply_prompt_variables() {
        let value = values.iter().find(|p| p.id == v.id).map(|p| p.value.clone()).unwrap_or_default();
        out = out.replace(&prompt_placeholder(v.id), &value);
    }
    out
}

/// The prompt a fresh install starts with (English seed). Pinned equal to the migration's seed by a test.
#[uniffi::export]
pub fn default_reply_prompt_text() -> String {
    "You are replying to {{message}}. You have read {{thread}} and {{excerpts}}. My availability is in {{calendar}}.\n\
Write the reply in this tone: {{tone}}. Follow {{instruction}} when there is one. Always keep in mind: {{notes}}.\n\
Answer the question first. Never promise a price or a deadline.\n\
// One sentence per rule. This line is a note: the model does not read it.".to_string()
}

/// The version number a new version takes: one past the highest, 1 when there is none. A number is never reused.
#[uniffi::export]
pub fn next_prompt_version(existing: Vec<u32>) -> u32 { existing.into_iter().max().map(|m| m + 1).unwrap_or(1) }

/// Refuses restoring a version that does not exist.
#[uniffi::export]
pub fn check_prompt_version_exists(existing: Vec<u32>, version: u32) -> Result<(), CoreError> {
    if existing.contains(&version) { Ok(()) } else { Err(CoreError::NotFound { kind: "prompt version".into(), id: version.to_string() }) }
}

/// Refuses a fixed note with no title or no text.
#[uniffi::export]
pub fn check_fixed_note(note: FixedNote) -> Result<(), CoreError> {
    if blank(&note.title) { return Err(refuse(REPLY_BAD_NOTE_CODE, "give the note a title")); }
    if blank(&note.text) { return Err(refuse(REPLY_BAD_NOTE_CODE, "write the note")); }
    Ok(())
}

// ---------------------------------------------------------------- drafts

/// The moves a draft makes, closed: Proposed to Edited, Sent or Discarded; Edited to Sent or Discarded; Sent and Discarded go nowhere (sealed).
#[uniffi::export]
pub fn reply_draft_can_move(from: ReplyDraftState, to: ReplyDraftState) -> bool {
    use ReplyDraftState::*;
    matches!((from, to), (Proposed, Edited) | (Proposed, Sent) | (Proposed, Discarded) | (Edited, Sent) | (Edited, Discarded))
}

/// Refuses any change to a sent or discarded draft, in words with `reply_draft_closed`.
#[uniffi::export]
pub fn check_reply_draft_open(draft: ReplyDraft) -> Result<(), CoreError> {
    match draft.state {
        ReplyDraftState::Proposed | ReplyDraftState::Edited => Ok(()),
        ReplyDraftState::Sent => Err(refuse(REPLY_DRAFT_CLOSED_CODE, "this draft was already sent")),
        ReplyDraftState::Discarded => Err(refuse(REPLY_DRAFT_CLOSED_CODE, "this draft was discarded")),
    }
}

/// The instruction the owner typed before the draft: trimmed, blank = none. Refused in words with `reply_instruction_off` when the field is switched off.
#[uniffi::export]
pub fn check_on_demand_instruction(options: OnDemandOptions, instruction: Option<String>) -> Result<Option<String>, CoreError> {
    let text = instruction.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    if text.is_some() && !options.instruction_before { return Err(refuse(REPLY_INSTRUCTION_OFF_CODE, "the instruction field is switched off in Settings, Replies, On demand")); }
    Ok(text)
}

/// Refuses a change request on a closed draft, with no words, or while the change field is switched off. Returns the trimmed text.
#[uniffi::export]
pub fn check_change_request(options: OnDemandOptions, draft: ReplyDraft, text: String) -> Result<String, CoreError> {
    check_reply_draft_open(draft)?;
    if !options.change_after { return Err(refuse(REPLY_CHANGE_OFF_CODE, "the change field is switched off in Settings, Replies, On demand")); }
    let t = text.trim();
    if t.is_empty() { return Err(refuse(REPLY_BAD_TEXT_CODE, "say what to change")); }
    Ok(t.to_string())
}

/// The draft after the model rewrote it: new text, one more revision, the state unchanged. Refused when the draft is closed.
#[uniffi::export]
pub fn reply_draft_after_change(draft: ReplyDraft, new_text: String, now_ms: i64) -> Result<ReplyDraft, CoreError> {
    check_reply_draft_open(draft.clone())?;
    if blank(&new_text) { return Err(refuse(REPLY_BAD_TEXT_CODE, "the rewritten draft is empty")); }
    Ok(ReplyDraft { text: new_text, revision: draft.revision + 1, updated_at_ms: now_ms, ..draft })
}

/// The draft after the owner took it into the composer: Proposed becomes Edited. A draft already taken, sent or discarded is refused.
#[uniffi::export]
pub fn reply_draft_after_accept(draft: ReplyDraft, now_ms: i64) -> Result<ReplyDraft, CoreError> {
    check_reply_draft_open(draft.clone())?;
    if !reply_draft_can_move(draft.state, ReplyDraftState::Edited) { return Err(refuse(REPLY_DRAFT_CLOSED_CODE, "this draft is already in your composer")); }
    Ok(ReplyDraft { state: ReplyDraftState::Edited, updated_at_ms: now_ms, ..draft })
}

/// The draft after the owner discarded it. A closed draft is refused.
#[uniffi::export]
pub fn reply_draft_after_discard(draft: ReplyDraft, now_ms: i64) -> Result<ReplyDraft, CoreError> {
    check_reply_draft_open(draft.clone())?;
    Ok(ReplyDraft { state: ReplyDraftState::Discarded, updated_at_ms: now_ms, ..draft })
}

/// The draft after its reply was sent (Proposed or Edited). A closed draft is refused.
#[uniffi::export]
pub fn reply_draft_after_sent(draft: ReplyDraft, now_ms: i64) -> Result<ReplyDraft, CoreError> {
    check_reply_draft_open(draft.clone())?;
    Ok(ReplyDraft { state: ReplyDraftState::Sent, updated_at_ms: now_ms, ..draft })
}

/// What happens to the thread's live draft when a new one arrives: it is discarded (one live draft per thread). None when there was none, or when it
/// was already sealed (a sent draft stays sent).
#[uniffi::export]
pub fn reply_draft_replaced(live: Option<ReplyDraft>, now_ms: i64) -> Option<ReplyDraft> {
    live.filter(|d| matches!(d.state, ReplyDraftState::Proposed | ReplyDraftState::Edited)).map(|d| ReplyDraft { state: ReplyDraftState::Discarded, updated_at_ms: now_ms, ..d })
}
