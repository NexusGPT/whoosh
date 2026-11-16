//! The text of a draft, in the order of the model note (docs/design/AI-REPLIES-MODEL.md section 2, "when a draft is written"): (1) look up
//! (reply-retrieval), (2) choose the voice (reply-voice: my replies in this thread, else to this person, and the tone), (3) write: the use's prompt
//! rendered with `render_reply_prompt` and its seven variables, ONE model call. Orchestration only: every outside answer comes through a port.
//! A change request reuses the same assembly and adds the current draft and the change.
use contracts::{
    reply_account_sources, reply_sources_read, CoreError, PromptValue, ReplyCalendar, ReplySource, ReplySourceRef, ReplySourcesRead, ReplyUseKind, render_reply_prompt,
};
use decision::thread_context::{fit, render};
use reply_retrieval::{Budgets, ExcerptIndex, ExcerptJudge, QueryWriter, ReplyMessage, RetrievalOutcome, RetrievalRequest, Retriever};
use reply_voice::{voice_for_draft, DraftContext, SentMail, VoiceLimits};
use smart_inbox::MessageFacts;
use store::Store;

use crate::replies_work::adapters::stored_excerpts::StoredExcerpts;
use crate::replies_work::adapters::model_ports::Calls;
use crate::replies_work::adapters::calendars::utc_offset_s;
use crate::replies_work::adapters::provider::cut;
use crate::replies_work::usecases::calendar_free::{event_lines, free_busy_text, looks_all_day, only_enabled, Ev};
use crate::replies_work::limits::*;
use crate::replies_work::model::{CallKind, TextModel};

/// What the writer is told, as the system text: the reply only, never sent.
pub(crate) const WRITER_SYSTEM: &str = "You write the email reply the owner will read, edit and send. Answer with the text of the reply only: no subject line, no quoted history, no note about what you did.";
/// The calendar variable while no calendar reaches the core: the writer must not invent a time (the model note, section 4).
pub(crate) const NO_CALENDAR: &str = "No calendar is connected: do not offer or accept a date or a time; leave a marked blank for the owner instead.";

/// The calendar variable when the enabled calendars hold no event in the window.
pub(crate) const NO_EVENT: &str = "My calendar holds no event in the coming days.";

/// Everything one draft rests on, beside its text.
pub(crate) struct Assembled { pub prompt: String, pub retrieval: RetrievalOutcome }

/// The ports a draft runs on.
pub(crate) struct Ports<'a> { pub store: &'a Store, pub index: &'a dyn ExcerptIndex, pub sent: &'a dyn SentMail, pub model: &'a dyn TextModel, pub memory: &'a StoredExcerpts }

/// The Sources pane as retrieval reads it: one card per account (its mail folders), the folders of this Mac, the calendars (by id: their names are the
/// Agendas pane's), the scope.
pub(crate) fn sources_read(store: &Store) -> Result<ReplySourcesRead, CoreError> {
    let rows: Vec<ReplySource> = store.reply_sources()?;
    let mut accounts: Vec<String> = vec![];
    let (mut local, mut calendars) = (vec![], vec![]);
    for s in &rows {
        match &s.source {
            ReplySourceRef::MailFolder { account_id, .. } => if !accounts.contains(account_id) { accounts.push(account_id.clone()) },
            ReplySourceRef::LocalFolder { .. } => local.push(s.clone()),
            ReplySourceRef::Calendar { calendar_id } => calendars.push(ReplyCalendar { calendar_id: calendar_id.clone(), name: calendar_id.clone(), account_label: String::new(), colour: String::new(), enabled: s.enabled }),
        }
    }
    let cards = accounts.into_iter().map(|a| reply_account_sources(a, rows.clone())).collect();
    Ok(reply_sources_read(cards, local, calendars, store.reply_retrieval()?.scope))
}

/// The address the reply goes to: the sender, or (the owner wrote the newest message) the first other recipient.
pub(crate) fn recipient(f: &MessageFacts) -> String {
    if f.from_owner { f.to.first().cloned().unwrap_or_default() } else { f.from_address.clone() }
}

fn sender_words(f: &MessageFacts) -> String { if f.from_name.trim().is_empty() { f.from_address.clone() } else { f.from_name.clone() } }

/// Retrieval for one message (the instruction joins the questions). Never fails: a step that cannot run is named in the trace.
pub(crate) fn retrieve(p: &Ports<'_>, f: &MessageFacts, instruction: Option<&str>) -> Result<RetrievalOutcome, CoreError> {
    let room = p.model.prompt_chars();
    let calls = Calls(p.model);
    let (writer, judge): (&dyn QueryWriter, &dyn ExcerptJudge) = (&calls, &calls);
    let request = RetrievalRequest {
        message: ReplyMessage { message_id: f.message_id.clone(), account_id: f.account_id.clone(), subject: f.subject.clone(), body: f.body.clone() },
        instruction: instruction.map(str::to_string),
        settings: p.store.reply_retrieval()?,
        sources: sources_read(p.store)?,
        budgets: Budgets { question_prompt_chars: room / SHARE_QUESTION, judge_prompt_chars: room / SHARE_JUDGE, context_chars: room / SHARE_EXCERPTS },
    };
    Ok(Retriever { writer, index: p.index, judge }.run(&request))
}

/// The prompt of one draft: retrieval, then the voice, then the use's current prompt rendered with its seven variables, cut to the model's room.
/// `f` carries the earlier messages of its thread (`thread_before`).
pub(crate) fn assemble(p: &Ports<'_>, kind: ReplyUseKind, f: &MessageFacts, instruction: Option<&str>) -> Result<Assembled, CoreError> {
    let room = p.model.prompt_chars();
    let retrieval = retrieve(p, f, instruction)?;
    let settings = p.store.reply_use(kind)?;
    let voice = voice_for_draft(&settings, &p.store.reply_voice()?, &p.store.reply_tones()?, &DraftContext { thread_id: &f.thread_id, recipient: &recipient(f) }, p.sent, &VoiceLimits::default())?;
    let mut tone = voice.tone_text.clone();
    if !voice.examples.is_empty() {
        let examples: Vec<String> = voice.examples.iter().map(|e| e.text.clone()).collect();
        tone.push_str(&cut(&format!("\n\nEarlier replies of mine, newest first:\n---\n{}", examples.join("\n---\n")), room / SHARE_EXAMPLES));
    }
    let current = p.store.reply_prompt_versions(&settings.prompt_id)?.into_iter().find(|v| v.current)
        .ok_or_else(|| CoreError::NotFound { kind: "prompt".into(), id: settings.prompt_id.clone() })?;
    let notes = p.store.reply_fixed_notes()?.iter().map(|n| format!("{} : {}", n.title, n.text)).collect::<Vec<_>>().join("\n");
    // `{{calendar}}`: the events of the enabled calendars (the last 7 days and the next 14, reply-sources' window), each with its day and times, those with the
    // correspondent first and marked, then the busy times and the free times inside working hours (calendar_free.rs); times and titles only, as before
    let on: Vec<ReplySourceRef> = sources_read(p.store)?.calendars.iter().filter(|c| c.enabled).map(|c| ReplySourceRef::Calendar { calendar_id: c.calendar_id.clone() }).collect();
    let calendar = if on.is_empty() { NO_CALENDAR.to_string() } else {
        let on_ids: Vec<String> = on.iter().filter_map(|s| match s { ReplySourceRef::Calendar { calendar_id } => Some(calendar_id.clone()), _ => None }).collect();
        let now = crate::rust_core::now_ms();
        let (from, to) = reply_sources::calendar_window(now);
        let off = |ms: i64| utc_offset_s(ms);
        let evs: Vec<Ev> = p.store.reply_calendar_events()?.into_iter().filter(|e| e.end_ms > from && e.start_ms < to).map(|e| Ev {
            all_day: looks_all_day(e.start_ms, e.end_ms, off(e.start_ms)), declined: false, calendar_id: e.calendar_id, title: e.title, start_ms: e.start_ms, end_ms: e.end_ms, attendees: e.attendees, location: e.location,
        }).collect();
        let evs = only_enabled(evs, &on_ids);
        let events: Vec<String> = event_lines(&evs, &recipient(f), &off);
        let held = if events.is_empty() { NO_EVENT.to_string() } else { cut(&format!("My calendar:\n{}", events.join("\n")), room / SHARE_EXAMPLES) };
        format!("{held}\n{}", free_busy_text(&evs, now, reply_sources::REPLY_CALENDAR_DAYS_AHEAD, &off))
    };
    let value = |id: &str, v: String| PromptValue { id: id.to_string(), value: v };
    // `{{message}}` as "Voir ce que le modèle reçoit" shows it (engine/src/replies.rs `preview`), the body cut to its share
    let values = vec![
        value("message", format!("De {} : {}\n{}", sender_words(f), f.subject, cut(&f.body, room / SHARE_BODY))),
        value("thread", render(&fit(&f.thread_before, room / SHARE_THREAD))),
        value("instruction", instruction.unwrap_or_default().to_string()),
        value("excerpts", retrieval.context_text.clone()),
        value("calendar", calendar),
        value("tone", tone),
        value("notes", notes),
    ];
    Ok(Assembled { prompt: cut(&render_reply_prompt(current.text, values), room), retrieval })
}

/// One call to the writer; a blank answer is a failure (a draft is never empty).
pub(crate) fn write(model: &dyn TextModel, prompt: &str) -> Result<String, CoreError> {
    let text = Calls(model).call(CallKind::Draft, Some(WRITER_SYSTEM.to_string()), prompt)?.trim().to_string();
    if text.is_empty() { return Err(CoreError::Unexpected { reason_words: "the model wrote an empty reply".into() }); }
    Ok(text)
}

/// The prompt of a change: the draft's own assembly, the current text, and what the owner asked to change (kept whole: it is the ask).
pub(crate) fn change_prompt(model: &dyn TextModel, assembled: &str, current: &str, change: &str) -> String {
    let tail = format!("\n\nThe current draft:\n{current}\n\nRewrite the draft with this change, and answer with the whole new draft: {change}");
    let room = model.prompt_chars().saturating_sub(tail.chars().count());
    format!("{}{tail}", cut(assembled, room))
}

/// The excerpt ids the draft rests on (the chunks retrieval handed the writer).
pub(crate) fn excerpt_ids(r: &RetrievalOutcome) -> Vec<String> { r.context.iter().map(|c| c.excerpt.chunk_id.clone()).collect() }
