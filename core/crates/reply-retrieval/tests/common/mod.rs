//! Fake ports and fixtures shared by the reply-retrieval tests. The fakes answer what each test scripts and record what they were asked.
#![allow(dead_code)]
use contracts::*;
use reply_retrieval::*;
use std::sync::{Arc, Mutex};

pub const NOV_09: i64 = 1_794_182_400_000; // 2026-11-09
pub const NOV_01: i64 = 1_793_491_200_000; // 2026-11-01
pub const OCT_01: i64 = 1_790_812_800_000; // 2026-10-01
pub const SEP_15: i64 = 1_789_430_400_000; // 2026-09-15

pub type Log<T> = Arc<Mutex<Vec<T>>>;

pub fn log<T>() -> Log<T> {
    Arc::new(Mutex::new(Vec::new()))
}

/// A question model that answers `answer` and records every prompt.
pub struct Writer {
    pub answer: Result<String, CoreError>,
    pub prompts: Log<String>,
}
impl QueryWriter for Writer {
    fn execute(&self, prompt: &str) -> Result<String, CoreError> {
        self.prompts.lock().unwrap().push(prompt.to_string());
        self.answer.clone()
    }
}
pub fn writer(answer: &str) -> Writer {
    Writer { answer: Ok(answer.to_string()), prompts: log() }
}

/// An index that answers per question text (`by_question`, else `rest`), or fails for the questions in `fail`, and records every query.
/// It IGNORES the sources it is given, as a faulty index would: the crate must drop what lies outside the scope itself.
pub struct Index {
    pub by_question: Vec<(String, Vec<FoundExcerpt>)>,
    pub rest: Vec<FoundExcerpt>,
    pub fail: Vec<String>,
    pub queries: Log<ExcerptQuery>,
}
impl ExcerptIndex for Index {
    fn execute(&self, query: &ExcerptQuery) -> Result<Vec<FoundExcerpt>, CoreError> {
        self.queries.lock().unwrap().push(query.clone());
        if self.fail.contains(&query.text) {
            return Err(CoreError::Unexpected { reason_words: "index unavailable".into() });
        }
        Ok(self.by_question.iter().find(|(q, _)| *q == query.text).map(|(_, l)| l.clone()).unwrap_or_else(|| self.rest.clone()))
    }
}
pub fn index(rest: Vec<FoundExcerpt>) -> Index {
    Index { by_question: vec![], rest, fail: vec![], queries: log() }
}
pub fn index_by(by_question: Vec<(&str, Vec<FoundExcerpt>)>) -> Index {
    Index { by_question: by_question.into_iter().map(|(q, l)| (q.to_string(), l)).collect(), rest: vec![], fail: vec![], queries: log() }
}

/// A judge that answers "no" when the prompt holds one of `no_when`, `garbage` when it holds one of `unclear_when`, else "yes".
pub struct Judge {
    pub no_when: Vec<String>,
    pub unclear_when: Vec<String>,
    pub prompts: Log<String>,
}
impl ExcerptJudge for Judge {
    fn execute(&self, prompt: &str) -> Result<String, CoreError> {
        self.prompts.lock().unwrap().push(prompt.to_string());
        if self.unclear_when.iter().any(|m| prompt.contains(m)) {
            return Ok("Peut-être, cela dépend.".into());
        }
        if self.no_when.iter().any(|m| prompt.contains(m)) {
            return Ok("Non.".into());
        }
        Ok("Yes, it helps.".into())
    }
}
pub fn judge() -> Judge {
    Judge { no_when: vec![], unclear_when: vec![], prompts: log() }
}

pub fn mail(account: &str, folder: &str) -> ReplySourceRef {
    ReplySourceRef::MailFolder { account_id: account.into(), folder: folder.into() }
}

/// An excerpt of mail account a1's INBOX.
pub fn ex(chunk: &str, score: f32, date: Option<i64>, subject: &str, text: &str) -> FoundExcerpt {
    ex_in(mail("a1", "INBOX"), chunk, score, date, subject, text)
}
pub fn ex_in(source: ReplySourceRef, chunk: &str, score: f32, date: Option<i64>, subject: &str, text: &str) -> FoundExcerpt {
    FoundExcerpt {
        chunk_id: chunk.into(),
        source,
        date_ms: date,
        sender: Some("Anna".into()),
        folder: Some("INBOX".into()),
        subject: (!subject.is_empty()).then(|| subject.to_string()),
        text: text.into(),
        score,
    }
}

fn src(r: ReplySourceRef, enabled: bool) -> ReplySource {
    ReplySource { source: r, enabled, item_count: 10, progress: ReplySyncProgress::UpToDate }
}

/// Account a1: INBOX and Clients on, Archive off. Account a2: INBOX on. A folder of this Mac on, one off. Calendar cal-1 on, cal-2 off.
pub fn sources() -> ReplySourcesRead {
    let a1 = reply_account_sources("a1".into(), vec![src(mail("a1", "INBOX"), true), src(mail("a1", "Clients"), true), src(mail("a1", "Archive"), false)]);
    let a2 = reply_account_sources("a2".into(), vec![src(mail("a2", "INBOX"), true)]);
    let local = vec![src(ReplySourceRef::LocalFolder { path: "~/Documents/Devis".into() }, true), src(ReplySourceRef::LocalFolder { path: "~/Old".into() }, false)];
    let cal = |id: &str, on: bool| ReplyCalendar { calendar_id: id.into(), name: id.into(), account_label: "a1".into(), colour: "#3366ff".into(), enabled: on };
    reply_sources_read(vec![a1, a2], local, vec![cal("cal-1", true), cal("cal-2", false)], RetrievalScope::ThisAccount)
}

pub fn message() -> ReplyMessage {
    ReplyMessage { message_id: "m1".into(), account_id: "a1".into(), subject: "Re: Devis toiture".into(), body: "Bonjour, pouvez-vous confirmer le prix du devis ?".into() }
}

pub const BIG: usize = 100_000;

/// The default settings with the AI filter OFF (each test turns on what it tests), big budgets.
pub fn request() -> RetrievalRequest {
    let mut settings = default_retrieval_settings();
    settings.ai_filter_question = None;
    RetrievalRequest { message: message(), instruction: None, settings, sources: sources(), budgets: Budgets { question_prompt_chars: BIG, judge_prompt_chars: BIG, context_chars: BIG } }
}

pub fn run(req: &RetrievalRequest, w: &Writer, i: &Index, j: &Judge) -> RetrievalOutcome {
    Retriever { writer: w, index: i, judge: j }.run(req)
}

pub fn kept_ids(o: &RetrievalOutcome) -> Vec<String> {
    o.context.iter().map(|c| c.excerpt.chunk_id.clone()).collect()
}

pub fn dropped_with(o: &RetrievalOutcome, reason: &DropReason) -> Vec<String> {
    o.trace.dropped.iter().filter(|d| &d.reason == reason).map(|d| d.chunk_id.clone()).collect()
}
