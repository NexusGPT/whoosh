//! Delta 20: how AI replies are READ and how their closed words are SPELT. The established route for reads is `core state <path>` (no new
//! `WhooshCore` method, so the Rust core and every other implementor need no stub). Each state path below returns ONE JSON value; the encoders here are
//! the one writer of that JSON, so the Rust core and the Swift FixtureCore (which calls them) cannot spell a key differently. The word spellings
//! (`*_key` / `*_from_key`) are the one spelling of each closed vocabulary in JSON and in the store's columns.
//!
//! STATE PATHS (all read-only; a write is a command):
//!   reply/use/auto, reply/use/on_demand   ReplyUse             {kind, enabled, tone_id, match_thread, prompt_id}
//!   reply/filter                          ReplyFilter          {scope, entry, ai, exit}                          (auto use)
//!   reply/on_demand                       OnDemandOptions      {instruction_before, change_after}
//!   reply/counts                          ReplyStepCounts      {days, received, after_entry, after_ai, after_exit, drafts}
//!   reply/sources                         ReplySourcesRead     {accounts, local_folders, calendars, scope, enabled_total}
//!   reply/retrieval                       RetrievalSettings
//!   reply/voice                           {voice: ReplyVoice, analysis, tones: [ReplyTone]}
//!   reply/prompt/current, reply/prompt/versions, reply/prompt/variables, reply/prompt/preview
//!   reply/notes                           [FixedNote]
//!   reply/draft/<thread id>               ReplyDraft, absent when the thread has no live draft
//!   reply/try/<run id>                    ReplyTryReport
use crate::*;

/// The state-path prefix of every reply read.
pub const REPLY_STATE_PREFIX: &str = "reply/";

// ---------------------------------------------------------------- spellings

macro_rules! keys {
    ($ty:ident, $to:ident, $from:ident, [$($v:ident => $k:literal),+ $(,)?]) => {
        #[uniffi::export]
        pub fn $to(v: $ty) -> String { match v { $($ty::$v => $k),+ }.to_string() }
        #[uniffi::export]
        pub fn $from(key: String) -> Option<$ty> { match key.as_str() { $($k => Some($ty::$v)),+, _ => None } }
    };
}
keys!(ReplyUseKind, reply_use_kind_key, reply_use_kind_from_key, [Auto => "auto", OnDemand => "on_demand"]);
keys!(ReplyStepResult, reply_step_result_key, reply_step_result_from_key, [Passed => "passed", Skipped => "skipped", Stopped => "stopped", NotReached => "not_reached"]);
keys!(ReplyMadeBy, reply_made_by_key, reply_made_by_from_key, [Auto => "auto", OnDemand => "on_demand"]);
keys!(ReplyDraftState, reply_draft_state_key, reply_draft_state_from_key, [Proposed => "proposed", Edited => "edited", Sent => "sent", Discarded => "discarded"]);
keys!(ExcerptSize, excerpt_size_key, excerpt_size_from_key, [Sentence => "sentence", Paragraph => "paragraph", Section => "section"]);
keys!(ClashRule, clash_rule_key, clash_rule_from_key, [KeepBothDated => "keep_both_dated", KeepNewest => "keep_newest", KeepOldest => "keep_oldest"]);
keys!(RetrievalScope, retrieval_scope_key, retrieval_scope_from_key, [ThisAccount => "this_account", AllAccounts => "all_accounts"]);
keys!(OnDemandOption, on_demand_option_key, on_demand_option_from_key, [InstructionBefore => "instruction_before", ChangeAfter => "change_after"]);
keys!(MatchMode, match_mode_key, match_mode_from_key, [Any => "any", All => "all"]);

// ---------------------------------------------------------------- a tiny JSON writer (no parser: stored JSON is read by SQLite's own functions)

fn s(v: &str) -> String {
    let mut o = String::from("\"");
    for c in v.chars() { match c { '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), '\n' => o.push_str("\\n"), '\r' => o.push_str("\\r"), '\t' => o.push_str("\\t"), c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)), c => o.push(c) } }
    o.push('"');
    o
}
fn b(v: bool) -> String { v.to_string() }
fn os(v: &Option<String>) -> String { v.as_deref().map(s).unwrap_or_else(|| "null".to_string()) }
fn oi(v: Option<i64>) -> String { v.map(|n| n.to_string()).unwrap_or_else(|| "null".to_string()) }
fn obj(fields: Vec<(&str, String)>) -> String { format!("{{{}}}", fields.into_iter().map(|(k, v)| format!("{}:{}", s(k), v)).collect::<Vec<_>>().join(",")) }
fn arr(items: Vec<String>) -> String { format!("[{}]", items.join(",")) }

fn condition(c: &RuleCondition) -> String {
    match c {
        RuleCondition::From { words } => obj(vec![("kind", s("from")), ("words", s(words))]),
        RuleCondition::To { words } => obj(vec![("kind", s("to")), ("words", s(words))]),
        RuleCondition::Cc { words } => obj(vec![("kind", s("cc")), ("words", s(words))]),
        RuleCondition::Domain { domain } => obj(vec![("kind", s("domain")), ("domain", s(domain))]),
        RuleCondition::SubjectContains { words } => obj(vec![("kind", s("subject")), ("words", s(words))]),
        RuleCondition::BodyContains { words } => obj(vec![("kind", s("body")), ("words", s(words))]),
        RuleCondition::ListHeader { words } => obj(vec![("kind", s("list_header")), ("words", s(words))]),
        RuleCondition::Account { account_id } => obj(vec![("kind", s("account")), ("account_id", s(account_id))]),
        RuleCondition::HasAttachment => obj(vec![("kind", s("has_attachment"))]),
        RuleCondition::Fact { fact, wanted } => obj(vec![("kind", s("fact")), ("fact", s(&message_fact_key(*fact))), ("wanted", b(*wanted))]),
        RuleCondition::Not { field, value } => obj(vec![("kind", s("not")), ("field", s(field.key())), ("value", s(value))]),
    }
}

/// The conditions of a rule as stored and as read: the same `{"kind": ..., <field>: ...}` objects the store's `rule.conditions_json` holds (delta 9, 17).
#[uniffi::export]
pub fn conditions_json(conditions: Vec<RuleCondition>) -> String { arr(conditions.iter().map(condition).collect()) }

/// A rule: its mode and its conditions, plus the eight cells of the facts grid and the text rows under it (`fact_states`, `row_conditions`), so a screen
/// draws the card from one value.
fn rule(r: &RuleSet) -> String {
    let facts = fact_states(r.conditions.clone()).into_iter().map(|f| {
        let st = match f.state { FactState::Any => "any", FactState::Yes => "yes", FactState::No => "no" };
        (message_fact_key(f.fact), s(st))
    }).collect::<Vec<_>>();
    let facts = format!("{{{}}}", facts.into_iter().map(|(k, v)| format!("{}:{}", s(&k), v)).collect::<Vec<_>>().join(","));
    obj(vec![("match", s(&match_mode_key(r.match_mode))), ("conditions", conditions_json(r.conditions.clone())), ("facts", facts), ("rows", conditions_json(row_conditions(r.conditions.clone())))])
}

// ---------------------------------------------------------------- the encoders

#[uniffi::export]
pub fn reply_use_json(u: ReplyUse) -> String {
    obj(vec![("kind", s(&reply_use_kind_key(u.kind))), ("enabled", b(u.enabled)), ("tone_id", s(&u.tone_id)), ("match_thread", b(u.match_thread)), ("prompt_id", s(&u.prompt_id))])
}

#[uniffi::export]
pub fn reply_filter_json(f: ReplyFilter) -> String {
    let scope = match &f.scope {
        ReplyLabelScope::All => obj(vec![("all", b(true)), ("label_ids", arr(vec![]))]),
        ReplyLabelScope::Labels { label_ids } => obj(vec![("all", b(false)), ("label_ids", arr(label_ids.iter().map(|l| s(l)).collect()))]),
    };
    let ai = obj(vec![("enabled", b(f.ai_enabled)), ("subject", b(f.ai_reads.subject)), ("body", b(f.ai_reads.body)),
        ("rest_of_thread", b(f.ai_reads.rest_of_thread)), ("attachments", b(f.ai_reads.attachments)), ("description", s(&f.ai_description))]);
    obj(vec![("scope", scope), ("entry", obj(vec![("enabled", b(f.entry_enabled)), ("rule", rule(&f.entry_rule))])), ("ai", ai),
        ("exit", obj(vec![("enabled", b(f.exit_enabled)), ("rule", rule(&f.exit_rule))]))])
}

#[uniffi::export]
pub fn on_demand_json(o: OnDemandOptions) -> String { obj(vec![("instruction_before", b(o.instruction_before)), ("change_after", b(o.change_after))]) }

#[uniffi::export]
pub fn reply_counts_json(c: ReplyStepCounts) -> String {
    obj(vec![("days", c.days.to_string()), ("received", c.received.to_string()), ("after_entry", c.after_entry.to_string()), ("after_ai", c.after_ai.to_string()),
        ("after_exit", c.after_exit.to_string()), ("drafts", c.drafts.to_string())])
}

fn source_ref(r: &ReplySourceRef) -> String {
    match r {
        ReplySourceRef::MailFolder { account_id, folder } => obj(vec![("kind", s("mail_folder")), ("account_id", s(account_id)), ("folder", s(folder))]),
        ReplySourceRef::LocalFolder { path } => obj(vec![("kind", s("local_folder")), ("path", s(path))]),
        ReplySourceRef::Calendar { calendar_id } => obj(vec![("kind", s("calendar")), ("calendar_id", s(calendar_id))]),
    }
}
fn progress(p: &ReplySyncProgress) -> String {
    match p {
        ReplySyncProgress::NotStarted => obj(vec![("state", s("not_started"))]),
        ReplySyncProgress::Syncing { percent } => obj(vec![("state", s("syncing")), ("percent", percent.to_string())]),
        ReplySyncProgress::UpToDate => obj(vec![("state", s("up_to_date"))]),
        ReplySyncProgress::Failed { reason_words } => obj(vec![("state", s("failed")), ("reason_words", s(reason_words))]),
        ReplySyncProgress::Gone => obj(vec![("state", s("gone"))]),
    }
}
fn source(x: &ReplySource) -> String { source_with(x, &[]) }
fn source_with(x: &ReplySource, choices: &[SourceChoice]) -> String {
    let key = reply_source_key(x.source.clone());
    // delta 24: the owner's remembered switch of a GONE folder (null for every other source), so the pane can show and flip it after a relaunch
    let owner = choices.iter().find(|c| c.source_key == key).map(|c| b(c.owner_enabled)).unwrap_or_else(|| "null".to_string());
    obj(vec![("source", source_ref(&x.source)), ("key", s(&key)), ("enabled", b(x.enabled)), ("owner_enabled", owner), ("item_count", x.item_count.to_string()), ("progress", progress(&x.progress))])
}

/// The owner's remembered switch of one gone source (`SourcePresence.owner_enabled`), by `reply_source_key`: what `reply/sources` carries as `owner_enabled`.
#[derive(uniffi::Record, Debug, Clone, PartialEq, Eq)]
pub struct SourceChoice { pub source_key: String, pub owner_enabled: bool }

/// One source's progress as the `ReplySourceProgress` event and the sources read spell it.
#[uniffi::export]
pub fn reply_source_json(x: ReplySource) -> String { source(&x) }

#[uniffi::export]
pub fn reply_sources_json(r: ReplySourcesRead) -> String { reply_sources_json_with_choices(r, vec![]) }

/// `reply/sources` with the owner's remembered switch of each gone folder (`owner_enabled`: bool for a gone source in `choices`, null for every other).
#[uniffi::export]
pub fn reply_sources_json_with_choices(r: ReplySourcesRead, choices: Vec<SourceChoice>) -> String {
    let accounts = r.accounts.iter().map(|a| obj(vec![("account_id", s(&a.account_id)), ("enabled_folders", a.enabled_folders.to_string()), ("total_folders", a.total_folders.to_string()),
        ("enabled_items", a.enabled_items.to_string()), ("folders", arr(a.folders.iter().map(|f| source_with(f, &choices)).collect()))])).collect();
    let calendars = r.calendars.iter().map(|c| obj(vec![("calendar_id", s(&c.calendar_id)), ("name", s(&c.name)), ("account_label", s(&c.account_label)), ("colour", s(&c.colour)), ("enabled", b(c.enabled))])).collect();
    obj(vec![("accounts", arr(accounts)), ("local_folders", arr(r.local_folders.iter().map(|f| source_with(f, &choices)).collect())), ("calendars", arr(calendars)),
        ("scope", s(&retrieval_scope_key(r.scope))), ("enabled_total", r.enabled_total.to_string())])
}

#[uniffi::export]
pub fn retrieval_json(r: RetrievalSettings) -> String {
    let f = r.attached_fields;
    obj(vec![("retrieval_prompt", s(&r.retrieval_prompt)), ("questions_max", r.questions_max.to_string()), ("excerpts_per_question", r.excerpts_per_question.to_string()),
        ("excerpt_size", s(&excerpt_size_key(r.excerpt_size))),
        ("attached_fields", obj(vec![("date", b(f.date)), ("sender", b(f.sender)), ("folder", b(f.folder)), ("subject", b(f.subject))])),
        ("keep_n", r.keep_n.to_string()), ("clash_rule", s(&clash_rule_key(r.clash_rule))), ("ai_filter_question", os(&r.ai_filter_question)), ("scope", s(&retrieval_scope_key(r.scope)))])
}

#[uniffi::export]
pub fn reply_voice_json(voice: ReplyVoice, analysis: ReplyVoiceAnalysis, tones: Vec<ReplyTone>) -> String {
    let an = match analysis {
        ReplyVoiceAnalysis::Idle => obj(vec![("state", s("idle"))]),
        ReplyVoiceAnalysis::Running { read, of } => obj(vec![("state", s("running")), ("read", read.to_string()), ("of", of.to_string())]),
        ReplyVoiceAnalysis::Failed { reason_words } => obj(vec![("state", s("failed")), ("reason_words", s(&reason_words))]),
    };
    obj(vec![("voice", obj(vec![("tone_text", s(&voice.tone_text)), ("sample_size", voice.sample_size.to_string()), ("analysed_at_ms", oi(voice.analysed_at_ms)),
        ("same_person_examples", voice.same_person_examples.to_string())])), ("analysis", an),
        ("tones", arr(tones.iter().map(|t| obj(vec![("id", s(&t.id)), ("name", s(&t.name)), ("text", s(&t.text))])).collect()))])
}

fn prompt(p: &ReplyPrompt) -> String {
    obj(vec![("prompt_id", s(&p.prompt_id)), ("version", p.version.to_string()), ("text", s(&p.text)), ("created_at_ms", p.created_at_ms.to_string()), ("current", b(p.current))])
}
#[uniffi::export]
pub fn reply_prompt_json(p: ReplyPrompt) -> String { prompt(&p) }
/// Every version of a prompt, newest first.
#[uniffi::export]
pub fn reply_prompt_versions_json(mut versions: Vec<ReplyPrompt>) -> String { versions.sort_by_key(|v| std::cmp::Reverse(v.version)); arr(versions.iter().map(prompt).collect()) }
#[uniffi::export]
pub fn reply_prompt_variables_json(vars: Vec<ReplyPromptVariable>) -> String {
    arr(vars.iter().map(|v| obj(vec![("id", s(&v.id)), ("name_key", s(&v.name_key)), ("meaning_key", s(&v.meaning_key)), ("placeholder", s(&prompt_placeholder(v.id.clone())))])).collect())
}
#[uniffi::export]
pub fn fixed_notes_json(notes: Vec<FixedNote>) -> String { arr(notes.iter().map(|n| obj(vec![("id", s(&n.id)), ("title", s(&n.title)), ("text", s(&n.text))])).collect()) }

fn draft(d: &ReplyDraft) -> String {
    obj(vec![("id", s(&d.id)), ("thread_id", s(&d.thread_id)), ("message_id", s(&d.message_id)), ("text", s(&d.text)), ("made_by", s(&reply_made_by_key(d.made_by))),
        ("excerpt_ids", arr(d.excerpt_ids.iter().map(|e| s(e)).collect())), ("instruction", os(&d.instruction)), ("state", s(&reply_draft_state_key(d.state))),
        ("revision", d.revision.to_string()), ("created_at_ms", d.created_at_ms.to_string()), ("updated_at_ms", d.updated_at_ms.to_string())])
}
#[uniffi::export]
pub fn reply_draft_json(d: ReplyDraft) -> String { draft(&d) }

fn excerpt(e: &ReplyExcerpt) -> String {
    obj(vec![("chunk_id", s(&e.chunk_id)), ("source", source_ref(&e.source)), ("date_ms", oi(e.date_ms)), ("sender", os(&e.sender)), ("folder", os(&e.folder)), ("subject", os(&e.subject)), ("text", s(&e.text))])
}
#[uniffi::export]
pub fn reply_excerpt_json(e: ReplyExcerpt) -> String { excerpt(&e) }

fn step(r: &ReplyStepReport) -> String {
    obj(vec![("result", s(&reply_step_result_key(r.result))), ("matched", conditions_json(r.matched.clone())), ("model_note", os(&r.model_note))])
}
#[uniffi::export]
pub fn reply_try_json(t: ReplyTryReport) -> String {
    obj(vec![("run_id", s(&t.run_id)), ("message_id", s(&t.message_id)), ("kind", s(&reply_use_kind_key(t.kind))),
        ("retrieval", obj(vec![("questions", arr(t.retrieval.questions.iter().map(|q| s(q)).collect())), ("found", t.retrieval.found.to_string()),
            ("kept", arr(t.retrieval.kept.iter().map(excerpt).collect())), ("dropped_by_ai_filter", t.retrieval.dropped_by_ai_filter.to_string())])),
        ("entry", step(&t.entry)), ("ai", step(&t.ai)), ("exit", step(&t.exit)), ("draft_text", os(&t.draft_text))])
}
