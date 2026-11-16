//! reply-engine (docs/lanes/reply-engine.md): the working commands of AI replies in the real core, behind `ReplyWork`, with a FAKE model (no model and
//! no mail server is ever called: the IMAP account is the sync-imap fake on 127.0.0.1). Every case below was seen red on a single-edit mutant of the
//! engine's code; the runner is _captures/reply-engine/run-mutants.py and its log _captures/reply-engine/mutants-run.log (mutant ids in brackets).
//!   on demand: the stored draft holds the text, the excerpt ids and the instruction; one live draft per thread; the events; the command answers in
//!     under 50 ms while the model takes 500 ms [W1 W2 W3 W4 W5 W6]
//!   a change request: revision + 1, the change and the current text reach the model [W7]
//!   accept (an ordinary composer draft, Edited) and discard [W8 W9]
//!   the auto pass over five messages the IMAP sync stores: each step's yes and no, the pass rows, the counts of `reply/counts`; a mail excerpt found
//!     by the word index [W10 W11 W12 W13]
//!   never for the owner's own message, never twice for one message [W14 W15]
//!   TryReply stores nothing [W16]
//!   the voice analysis: running, then the tone stored, a second run refused [W17 W18]
//!   a folder of this Mac: progress events, found by a retrieval, gone from the index when removed [W19 W20]
//!   no model set: the refusal code, in words [W21]
//!   a model that fails leaves no half draft and the old draft as it was [W22]
//!   the chosen model (ai_settings::model_for): one retry when the provider says later, none on a refusal, its words, the prompt cut [W23 W24 W26]
//!   the background waits while paused [W25]
//!   a core with NO work installed still answers the contract's not-built-yet code [W31]
//!   without the hold a hand-seeded count is replaced by the real figure; with it, it stays (the hold hides no defect) [W36 W37]
//!   a mail excerpt is named by its chunk's own id (delta 21) [W38]
//!   the crawl's pause holds the auto pass, never a draft the owner asks for [W39 W40]
//!   the Writer's model from Settings over its REAL type (ai_settings::ModelChoice, a scripted transport): one retry, and each job's own timeout [W41 W42]
//!   the calendars the shell pushes (delta 21): a new calendar starts off, its events are kept once it is on, and they reach the writer [W32 W33 W34]
#[allow(dead_code, unused_imports)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code, unused_imports)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::replies_work::{hold_background, indexed_excerpts, set_reply_model, set_reply_pause, settle, CallKind, Completer, ModelCall, Pause, ProviderModel, TextModel, REPLY_VOICE_RUNNING_CODE};
use engine::replies::{install_reply_work, NotBuiltYet};
use engine::{RustCore, TestTransport};
use imap_fakes::{smtp::FakeSmtp, FakeImap};
use secrets::{MemoryKeychain, Namespace};
use store::{NewMessage, Store};

const USER: &str = "owner@example.test";
const PW: &str = "app-password";
const ANSWER: &str = "Thank you, here is my answer.";
const VOICE: &str = "Je vouvoie les clients. J'écris court.";

// ---------------------------------------------------------------- the fake model

/// Answers each kind of call as a model would, after `delay`; records every call; a kind can be made to fail.
struct Fake { delay: Duration, yes: Vec<String>, calls: Mutex<Vec<ModelCall>>, down: Mutex<Vec<CallKind>> }

impl Fake {
    fn new(delay_ms: u64, yes: &[&str]) -> Arc<Fake> {
        Arc::new(Fake { delay: Duration::from_millis(delay_ms), yes: yes.iter().map(|s| s.to_string()).collect(), calls: Mutex::default(), down: Mutex::default() })
    }
    fn count(&self, kind: CallKind) -> usize { self.calls.lock().unwrap().iter().filter(|c| c.kind == kind).count() }
    fn last(&self, kind: CallKind) -> ModelCall { self.calls.lock().unwrap().iter().rev().find(|c| c.kind == kind).cloned().unwrap_or_else(|| panic!("no {kind:?} call")) }
    fn fail(&self, kind: CallKind) { self.down.lock().unwrap().push(kind); }
}

impl TextModel for Fake {
    fn execute(&self, call: &ModelCall) -> Result<String, CoreError> {
        std::thread::sleep(self.delay);
        self.calls.lock().unwrap().push(call.clone());
        if self.down.lock().unwrap().contains(&call.kind) {
            return Err(CoreError::Action { failure: ActionFailed { account_name: "Fake".into(), reason_words: "the fake model is down".into(), retryable: false, code: None } });
        }
        Ok(match call.kind {
            CallKind::Decide => if self.yes.iter().any(|w| call.prompt.contains(w.as_str())) { "yes, it waits for my answer".into() } else { "no, nothing to answer".into() },
            CallKind::Questions => "contract price\nmessage".into(),
            CallKind::Judge => "yes".into(),
            CallKind::Draft if call.prompt.contains("Rewrite the draft with this change") => format!("Rewritten: {}", call.prompt.rsplit(": ").next().unwrap_or("")),
            CallKind::Draft => ANSWER.into(),
            CallKind::Voice => VOICE.into(),
        })
    }
    fn prompt_chars(&self) -> usize { 20_000 }
}

// ---------------------------------------------------------------- the core, mail, events

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn now() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64 }
fn config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}
fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(20) {
        if ok() { return; }
        std::thread::sleep(Duration::from_millis(25));
    }
    panic!("never reached within 20 s: {what}");
}
fn mail(account: &str, id: &str, thread: &str, from: &str, subject: &str, body: &str, at: i64, folder: &str) -> NewMessage {
    let mine = from == USER;
    NewMessage {
        message: Message {
            id: id.into(), thread_id: thread.into(), account_id: account.into(), sender_name: String::new(), from_address: from.into(),
            to: vec![if mine { "claire@client.test".into() } else { USER.into() }], subject: subject.into(), preview: body.into(), received_at_ms: at, read: mine, archived: false,
            folder: folder.into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: mine,
        },
        remote_id: Some(id.into()), body_text: Some(body.into()), body_html: None, has_list_header: false, size_bytes: Some(100), tab_id: None,
    }
}
/// A core over a fresh store with one account (no server: nothing syncs) and the fake model.
fn seeded(fake: Option<Arc<Fake>>) -> (RustCore, String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    let core = RustCore::with_parts(store, kc(), detector(), TestTransport::PlainLoopback);
    set_reply_model(core.store().path(), fake.map(|f| f as Arc<dyn TextModel>));
    (core, account, dir)
}
fn claire(core: &RustCore, account: &str) {
    core.store().insert_messages(vec![mail(account, "m1", "t1", "claire@client.test", "Contract", "Hello, what is the contract price? Claire", now() - 60_000, "INBOX")]).unwrap();
}

#[derive(Default)]
struct Events(Mutex<Vec<CoreEvent>>);
impl EventSink for Events { fn on_event(&self, e: CoreEvent) { self.0.lock().unwrap().push(e); } }
impl Events {
    fn all(&self) -> Vec<CoreEvent> { self.0.lock().unwrap().clone() }
    fn ready(&self) -> Vec<String> { self.all().into_iter().filter_map(|e| if let CoreEvent::ReplyDraftReady { draft_id, .. } = e { Some(draft_id) } else { None }).collect() }
    fn changed(&self) -> Vec<ReplyDraft> { self.all().into_iter().filter_map(|e| if let CoreEvent::ReplyDraftChanged { draft, .. } = e { Some(draft) } else { None }).collect() }
}
fn listen(core: &RustCore, stream: StreamKind) -> Arc<Events> { let e = Arc::new(Events::default()); core.subscribe(stream, e.clone()); e }

fn run(core: &RustCore, c: Command) -> CommandOutcome { core.run(c).unwrap_or_else(|e| panic!("refused: {e}")) }
fn code(core: &RustCore, c: Command) -> (String, String) {
    match core.run(c).expect_err("a refusal") {
        CoreError::Action { failure } => (failure.code.unwrap_or_default(), failure.reason_words),
        CoreError::Invalid { field, reason_words } => (field, reason_words),
        other => (format!("{other:?}"), String::new()),
    }
}
fn val(core: &RustCore, path: &str) -> serde_json::Value { serde_json::from_str(&core.state(path.into()).unwrap_or_else(|| panic!("no answer for {path}"))).unwrap() }
fn count(core: &RustCore, sql: &'static str) -> i64 { core.store().write(move |c| Ok(c.tx.query_row(sql, [], |r| r.get::<_, i64>(0))?)).unwrap() }
fn settled(core: &RustCore) { assert!(settle(core.store().path(), Duration::from_secs(20)), "the reply threads did not settle"); }
fn draft(core: &RustCore, id: &str) -> ReplyDraft {
    let id = id.to_string();
    core.store().reply_draft(&id).unwrap().unwrap_or_else(|| panic!("no draft {id}"))
}
/// A folder of this Mac holding one note, added on the Sources pane, walked to its end.
fn folder_with_note(core: &RustCore) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("prices.md"), "# Prices\n\nThe contract price is 42 euros a month, paid each quarter.").unwrap();
    let path = dir.path().to_string_lossy().to_string();
    run(core, Command::AddReplyLocalFolder { path: path.clone() });
    wait_until("the folder of this Mac is up to date", || {
        let v = val(core, "reply/sources");
        v["local_folders"].as_array().is_some_and(|a| a.iter().any(|f| f["progress"]["state"] == "up_to_date" && f["item_count"] == 1))
    });
    (dir, path)
}

// ---------------------------------------------------------------- on demand

#[test]
fn a_draft_on_demand_is_written_in_the_background_with_its_excerpts_and_instruction_and_replaces_the_live_one() {
    let fake = Fake::new(500, &[]);
    let (core, account, _d) = seeded(Some(fake.clone()));
    claire(&core, &account);
    let (_folder, _) = folder_with_note(&core);
    let thread = listen(&core, StreamKind::Thread);

    let t0 = Instant::now();
    let out = run(&core, Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: Some("  Say the price  ".into()) });
    let took = t0.elapsed();
    assert!(took < Duration::from_millis(50), "the command waited {took:?} on a model that takes 500 ms");
    let first = out.created_id.expect("the draft's id");
    wait_until("the first draft is ready", || thread.ready().contains(&first));

    let d = draft(&core, &first);
    assert_eq!((d.text.as_str(), d.instruction.as_deref(), d.state, d.made_by, d.revision, d.message_id.as_str()), (ANSWER, Some("Say the price"), ReplyDraftState::Proposed, ReplyMadeBy::OnDemand, 0, "m1"));
    assert!(d.excerpt_ids.iter().any(|e| e.starts_with("local\u{1f}")), "the draft rests on the note of the folder: {:?}", d.excerpt_ids);
    assert_eq!(val(&core, "reply/draft/t1")["id"], first.as_str());
    let prompt = fake.last(CallKind::Draft).prompt;
    for wanted in ["Say the price", "42 euros", "what is the contract price?"] { assert!(prompt.contains(wanted), "the writer's prompt lacks {wanted:?}:\n{prompt}"); }
    assert!(!prompt.contains("// One sentence per rule"), "a note line of the prompt reached the model");

    let second = run(&core, Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: None }).created_id.unwrap();
    wait_until("the second draft is ready", || thread.ready().contains(&second));
    assert_eq!(draft(&core, &first).state, ReplyDraftState::Discarded, "the old live draft is discarded");
    assert_eq!(draft(&core, &second).instruction, None);
    assert_eq!(count(&core, "SELECT count(*) FROM reply_draft WHERE thread_id = 't1' AND state IN ('proposed','edited')"), 1, "one live draft per thread");
    assert!(thread.changed().iter().any(|c| c.id == first && c.state == ReplyDraftState::Discarded), "the replaced draft is announced");
}

#[test]
fn a_change_request_rewrites_the_draft_with_the_same_context_and_counts_a_revision() {
    let fake = Fake::new(0, &[]);
    let (core, account, _d) = seeded(Some(fake.clone()));
    claire(&core, &account);
    let thread = listen(&core, StreamKind::Thread);
    let id = run(&core, Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: Some("be warm".into()) }).created_id.unwrap();
    wait_until("the draft is ready", || thread.ready().contains(&id));
    run(&core, Command::RequestReplyChange { draft_id: id.clone(), text: "  make it shorter  ".into() });
    wait_until("the draft changed", || thread.changed().iter().any(|d| d.id == id && d.revision == 1));
    let d = draft(&core, &id);
    assert_eq!((d.text.as_str(), d.revision, d.state, d.instruction.as_deref()), ("Rewritten: make it shorter", 1, ReplyDraftState::Proposed, Some("be warm")));
    let prompt = fake.last(CallKind::Draft).prompt;
    assert!(prompt.contains(ANSWER) && prompt.contains("be warm") && prompt.contains("what is the contract price?"), "the change keeps the context and the current text:\n{prompt}");
}

#[test]
fn accept_hands_the_text_to_an_ordinary_composer_draft_and_discard_throws_the_draft_away() {
    let (core, account, _d) = seeded(Some(Fake::new(0, &[])));
    claire(&core, &account);
    let thread = listen(&core, StreamKind::Thread);
    let id = run(&core, Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: None }).created_id.unwrap();
    wait_until("the draft is ready", || thread.ready().contains(&id));
    let composer = run(&core, Command::AcceptReplyDraft { draft_id: id.clone() }).created_id.expect("the composer draft's id");
    let (body, thread_of, to, subject): (String, String, String, String) = {
        let c = composer.clone();
        core.store().write(move |w| Ok(w.tx.query_row("SELECT body, reply_to_thread_id, to_json, subject FROM draft WHERE id = ?1", [c], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?)).unwrap()
    };
    assert_eq!((body.as_str(), thread_of.as_str(), subject.as_str()), (ANSWER, "t1", "Re: Contract"));
    assert!(to.contains("claire@client.test"), "the reply goes to the sender: {to}");
    assert_eq!(draft(&core, &id).state, ReplyDraftState::Edited);
    assert!(thread.changed().iter().any(|d| d.id == id && d.state == ReplyDraftState::Edited));
    assert_eq!(code(&core, Command::AcceptReplyDraft { draft_id: id.clone() }).0, REPLY_DRAFT_CLOSED_CODE, "a draft already taken is not taken twice");
    assert_eq!(count(&core, "SELECT count(*) FROM outbox_entry"), 0, "nothing is sent");

    let other = run(&core, Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: None }).created_id.unwrap();
    wait_until("the next draft is ready", || thread.ready().contains(&other));
    run(&core, Command::DiscardReplyDraft { draft_id: other.clone() });
    assert_eq!(draft(&core, &other).state, ReplyDraftState::Discarded);
    assert!(thread.changed().iter().any(|d| d.id == other && d.state == ReplyDraftState::Discarded));
    assert_eq!(count(&core, "SELECT count(*) FROM draft"), 1, "a discard makes no composer draft");
}

// ---------------------------------------------------------------- the auto use, through the IMAP sync

fn pass_rows(core: &RustCore) -> Vec<(String, String, String, String, bool)> {
    core.store().write(|w| {
        let mut st = w.tx.prepare("SELECT m.subject, p.entry_result, p.ai_result, p.exit_result, p.draft_id IS NOT NULL FROM reply_pass p JOIN message m ON m.id = p.message_id ORDER BY m.subject")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }).unwrap()
}

/// The auto use on, an entry rule that stops "Message 1", an exit rule that keeps "Message 4" only; the model says yes to 3, 4 and 5.
fn five_message_filter(core: &RustCore) -> Arc<Fake> {
    let fake = Fake::new(0, &["Message 3", "Message 4", "Message 5"]);
    set_reply_model(core.store().path(), Some(fake.clone()));
    let subject = |n: u32| RuleCondition::SubjectContains { words: format!("Message {n}") };
    run(core, Command::SetReplyUseEnabled { kind: ReplyUseKind::Auto, enabled: true });
    run(core, Command::SetReplyFilterEntryRule { rule: RuleSet { conditions: (2..=5).map(subject).collect(), match_mode: MatchMode::Any } });
    run(core, Command::SetReplyFilterExitRule { rule: RuleSet { conditions: vec![subject(4)], match_mode: MatchMode::Any } });
    fake
}
fn five_rows() -> Vec<(String, String, String, String, bool)> {
    let s = |x: &str| x.to_string();
    vec![
        (s("Message 1"), s("stopped"), s("not_reached"), s("not_reached"), false),
        (s("Message 2"), s("passed"), s("stopped"), s("not_reached"), false),
        (s("Message 3"), s("passed"), s("passed"), s("stopped"), false),
        (s("Message 4"), s("passed"), s("passed"), s("passed"), true),
        (s("Message 5"), s("passed"), s("passed"), s("stopped"), false),
    ]
}

#[test]
fn the_auto_use_runs_the_three_steps_on_each_arrival_and_the_counts_read_its_rows() {
    let (core, account, _d) = seeded(None);
    let fake = five_message_filter(&core);
    let t = now();
    core.store().insert_messages((1..=5).map(|n| mail(&account, &format!("m{n}"), &format!("t{n}"), &format!("sender{n}@client.test"), &format!("Message {n}"), &format!("Body of message {n}, please answer."), t - i64::from(n) * 60_000, "INBOX")).collect()).unwrap();
    core.note_stored((1..=5).map(|n| format!("m{n}")).collect());
    core.flush_filing();
    settled(&core);
    assert_eq!(pass_rows(&core), five_rows());
    let counts = val(&core, "reply/counts");
    assert_eq!((counts["received"].as_u64(), counts["after_entry"].as_u64(), counts["after_ai"].as_u64(), counts["after_exit"].as_u64(), counts["drafts"].as_u64()),
        (Some(5), Some(4), Some(3), Some(1), Some(1)), "{counts}");
    assert_eq!(fake.count(CallKind::Decide), 4, "the model is asked only about what the entry filter let through");
    let auto = core.store().write(|w| Ok(w.tx.query_row("SELECT made_by, state, thread_id FROM reply_draft", [], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?)).unwrap();
    assert_eq!(auto, ("auto".to_string(), "proposed".to_string(), "t4".to_string()));
    assert_eq!(count(&core, "SELECT count(*) FROM outbox_entry"), 0, "nothing is sent");

    // never twice: the same message handed again is not looked at again
    core.note_stored(vec!["m4".into(), "m2".into()]);
    core.flush_filing();
    settled(&core);
    assert_eq!((count(&core, "SELECT count(*) FROM reply_draft"), fake.count(CallKind::Decide)), (1, 4), "a message gets at most one auto draft and one question");

    // never for the owner's own message
    core.store().insert_messages(vec![mail(&account, "own1", "town", USER, "My own note", "a note to myself", now(), "INBOX")]).unwrap();
    core.note_stored(vec!["own1".into()]);
    core.flush_filing();
    settled(&core);
    assert_eq!((core.store().reply_pass_of("own1").unwrap(), count(&core, "SELECT count(*) FROM reply_draft WHERE thread_id = 'town'")), (None, 0));
    assert_eq!(val(&core, "reply/counts")["received"].as_u64(), Some(5), "the owner's message is not counted as received");
}

/// A meaning model for the tests: each word lands in a slot of the vector (so the indexer cuts every body into chunks and embeds them).
struct WordEmbedder;
fn word_vector(text: &str) -> Vec<f32> {
    let dim = search_vector::constants::EMBED_DIM as usize;
    let mut v = vec![0f32; dim];
    for w in text.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) { v[w.bytes().fold(7usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize)) % dim] += 1.0; }
    v
}
impl Embedder for WordEmbedder {
    fn dimension(&self) -> u32 { search_vector::constants::EMBED_DIM as u32 }
    fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { Ok(texts.iter().map(|t| word_vector(t)).collect()) }
    fn embed_query(&self, text: String) -> Result<Vec<f32>, CoreError> { Ok(word_vector(&text)) }
    fn embed_titled(&self, _title: Option<String>, texts: Vec<String>) -> Result<Vec<Vec<f32>>, CoreError> { self.embed(texts) }
    fn embed_media(&self, _kind: MediaKind, path: String) -> Result<Vec<f32>, CoreError> { Ok(word_vector(&path)) }
}

#[test]
fn the_messages_the_imap_sync_stores_go_through_the_auto_use_and_the_mail_is_a_source() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.seed("INBOX", 5);
    let smtp = rt.block_on(FakeSmtp::start(USER, PW));
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::with_parts(Store::open(&dir.path().join("whoosh.sqlite")).unwrap(), kc(), detector(), TestTransport::PlainLoopback);
    five_message_filter(&core);
    core.background().set_embedder(Some(Arc::new(WordEmbedder)));
    let account = run(&core, Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).created_id.unwrap();
    wait_until("history fetched", || core.sync_states().iter().any(|s| s.account_id == account && s.history_fetched));
    wait_until("five pass rows", || { core.flush_filing(); settled(&core); pass_rows(&core).len() == 5 });
    assert_eq!(pass_rows(&core), five_rows(), "the sync's own arrivals ran the three steps");

    // the mail itself is a source: the INBOX tile, switched on, is searched by the word index
    wait_until("the INBOX tile", || val(&core, "reply/sources")["accounts"].as_array().is_some_and(|a| a.iter().any(|c| c["folders"].as_array().is_some_and(|f| f.iter().any(|t| t["source"]["folder"] == "INBOX")))));
    run(&core, Command::SetReplySourceEnabled { source: ReplySourceRef::MailFolder { account_id: account.clone(), folder: "INBOX".into() }, enabled: true });
    // the indexer embeds every chunk too (the test's meaning model) at the background priority: on a loaded machine that takes longer than 20 s
    // Wait for what retrieval needs, read from the store: every INBOX body cut into chunks and embedded (`indexed`), then the word index committed.
    // NOT `settle_index` alone: it can answer "idle" in the gap between the indexer taking a job and marking itself busy (indexer.rs, next_job/run),
    // which a loaded machine widens: the draft then ran before anything was indexed. Calm load: under 2 s (docs/lanes/reply-engine.md, round 8).
    let t0 = Instant::now();
    let all_indexed = || core.background().progress().is_ok_and(|p| p.iter().any(|f| f.folder == "INBOX" && f.messages == 5 && f.indexed == 5));
    while !all_indexed() && t0.elapsed() < Duration::from_secs(60) { std::thread::sleep(Duration::from_millis(25)); }
    let indexed_in = t0.elapsed();
    let words_done = core.background().settle_index(Duration::from_secs(60));
    eprintln!("WAIT every INBOX body indexed={} in {indexed_in:?}; word index committed={words_done} in {:?}", all_indexed(), t0.elapsed() - indexed_in);
    assert!(all_indexed() && words_done, "every INBOX body indexed and the word index committed: {:?}", core.background().progress());
    let m1 = core.store().write(|w| Ok(w.tx.query_row("SELECT thread_id FROM message WHERE subject = 'Message 1'", [], |r| r.get::<_, String>(0))?)).unwrap();
    let thread = listen(&core, StreamKind::Thread);
    let id = run(&core, Command::DraftReplyOnDemand { thread_id: m1, instruction: None }).created_id.unwrap();
    wait_until("the draft is ready", || thread.ready().contains(&id));
    let ids = draft(&core, &id).excerpt_ids;
    let reads = core.store().background_reads().unwrap();
    assert!(ids.iter().filter_map(|e| chunk_id_from_excerpt_id(e.clone())).any(|c| reads.chunk(c).unwrap().is_some_and(|(k, _)| k.subject.starts_with("Message"))), "a mail excerpt is named by its chunk's own id: {ids:?}");
}

#[test]
fn the_auto_use_waits_while_the_background_is_paused() {
    let (core, account, _d) = seeded(Some(Fake::new(0, &["Contract"])));
    run(&core, Command::SetReplyUseEnabled { kind: ReplyUseKind::Auto, enabled: true });
    struct Held(AtomicBool);
    impl Pause for Held { fn held(&self) -> bool { self.0.load(Ordering::SeqCst) } }
    let held = Arc::new(Held(AtomicBool::new(true)));
    set_reply_pause(core.store().path(), Some(held.clone()));
    claire(&core, &account);
    core.note_stored(vec!["m1".into()]);
    core.flush_filing();
    assert!(!settle(core.store().path(), Duration::from_millis(1500)), "the background did not wait");
    assert_eq!(core.store().reply_pass_of("m1").unwrap(), None, "nothing ran while paused");
    held.0.store(false, Ordering::SeqCst);
    settled(&core);
    assert!(core.store().reply_pass_of("m1").unwrap().is_some_and(|p| p.draft_id.is_some()), "the pass ran once the pause ended");
}

// ---------------------------------------------------------------- try on a message

#[test]
fn try_on_a_message_reports_the_steps_retrieval_and_text_and_stores_nothing() {
    let (core, account, _d) = seeded(Some(Fake::new(0, &["Contract"])));
    claire(&core, &account);
    let before = core.store().message("m1").unwrap().unwrap();
    let run_id = run(&core, Command::TryReply { message_id: "m1".into(), kind: ReplyUseKind::Auto }).created_id.unwrap();
    wait_until("the try's report", || core.state(format!("reply/try/{run_id}")).is_some());
    let r = val(&core, &format!("reply/try/{run_id}"));
    assert_eq!((r["entry"]["result"].as_str(), r["ai"]["result"].as_str(), r["exit"]["result"].as_str()), (Some("passed"), Some("passed"), Some("passed")), "{r}");
    assert_eq!(r["ai"]["model_note"], "yes, it waits for my answer");
    assert_eq!(r["draft_text"], ANSWER);
    assert_eq!(r["retrieval"]["questions"], serde_json::json!([]), "no source is switched on: nothing was asked");
    assert_eq!((count(&core, "SELECT count(*) FROM reply_draft"), count(&core, "SELECT count(*) FROM reply_pass"), count(&core, "SELECT count(*) FROM draft")), (0, 0, 0), "a try stores nothing");
    assert_eq!(core.store().message("m1").unwrap().unwrap(), before, "a try changes no message");

    let on_demand = run(&core, Command::TryReply { message_id: "m1".into(), kind: ReplyUseKind::OnDemand }).created_id.unwrap();
    wait_until("the second report", || core.state(format!("reply/try/{on_demand}")).is_some());
    assert_eq!(val(&core, &format!("reply/try/{on_demand}"))["entry"]["result"], "skipped", "on demand no filter applies");
}

// ---------------------------------------------------------------- the voice

#[test]
fn the_voice_analysis_runs_in_the_background_and_stores_the_tone_with_its_count() {
    let fake = Fake::new(400, &[]);
    let (core, account, _d) = seeded(Some(fake.clone()));
    core.store().insert_messages((1..=3).map(|i| mail(&account, &format!("s{i}"), &format!("ts{i}"), USER, "Re: hello", &format!("Bonjour, merci pour votre message {i}. Cordialement"), now() - i * 1000, "Sent")).collect()).unwrap();
    let t0 = Instant::now();
    run(&core, Command::RunVoiceAnalysis { sample_size: 10 });
    assert!(t0.elapsed() < Duration::from_millis(50), "the command waited on the model");
    assert_eq!(val(&core, "reply/voice")["analysis"]["state"], "running");
    assert_eq!(code(&core, Command::RunVoiceAnalysis { sample_size: 10 }).0, REPLY_VOICE_RUNNING_CODE, "a second run while one runs is refused");
    wait_until("the analysis is done", || val(&core, "reply/voice")["analysis"]["state"] == "idle");
    let v = val(&core, "reply/voice");
    assert_eq!((v["voice"]["tone_text"].as_str(), v["voice"]["sample_size"].as_u64()), (Some(VOICE), Some(3)), "{v}");
    assert!(v["voice"]["analysed_at_ms"].as_i64().is_some_and(|t| t > now() - 60_000), "the time of the analysis is kept: {v}");
    let prompt = fake.last(CallKind::Voice).prompt;
    assert!(prompt.contains("merci pour votre message 1") && !prompt.contains("claire@client.test"), "the samples reach the model, no address does:\n{prompt}");
}

// ---------------------------------------------------------------- a folder of this Mac

#[test]
fn a_folder_of_this_mac_is_walked_with_progress_found_by_a_retrieval_and_leaves_the_index_when_removed() {
    let (core, account, _d) = seeded(Some(Fake::new(0, &[])));
    claire(&core, &account);
    let status = listen(&core, StreamKind::SyncStatus);
    let (_folder, path) = folder_with_note(&core);
    let source = ReplySourceRef::LocalFolder { path: path.clone() };
    let progress: Vec<ReplySyncProgress> = status.all().into_iter().filter_map(|e| match e { CoreEvent::ReplySourceProgress { source: s, progress, .. } if s == source => Some(progress), _ => None }).collect();
    assert_eq!(progress.first(), Some(&ReplySyncProgress::Syncing { percent: 0 }), "{progress:?}");
    assert_eq!(progress.last(), Some(&ReplySyncProgress::UpToDate), "{progress:?}");
    assert!(indexed_excerpts(core.store().path(), &source) > 0);

    let id = run(&core, Command::TryReply { message_id: "m1".into(), kind: ReplyUseKind::OnDemand }).created_id.unwrap();
    wait_until("the try's report", || core.state(format!("reply/try/{id}")).is_some());
    let kept = val(&core, &format!("reply/try/{id}"))["retrieval"]["kept"].clone();
    assert!(kept.as_array().is_some_and(|k| k.iter().any(|e| e["text"].as_str().is_some_and(|t| t.contains("42 euros")))), "the note is found: {kept}");

    run(&core, Command::RemoveReplyLocalFolder { path });
    settled(&core);
    assert_eq!(indexed_excerpts(core.store().path(), &source), 0, "a removed folder's excerpts leave the index");
}

// ---------------------------------------------------------------- no model, a failing model, the provider adapter

#[test]
fn with_no_model_set_the_working_commands_refuse_in_words_with_the_code() {
    let (core, account, _d) = seeded(None);
    claire(&core, &account);
    core.store().write(|w| Ok(w.tx.execute("INSERT INTO reply_draft (id, thread_id, message_id, text, made_by, state, created_at_ms, updated_at_ms) VALUES ('d1', 't1', 'm1', 'Hello', 'on_demand', 'proposed', 1, 1)", [])?)).unwrap();
    for c in [
        Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: None },
        Command::TryReply { message_id: "m1".into(), kind: ReplyUseKind::Auto },
        Command::RunVoiceAnalysis { sample_size: 10 },
        Command::RequestReplyChange { draft_id: "d1".into(), text: "shorter".into() },
    ] {
        let (c2, words) = code(&core, c.clone());
        assert_eq!(c2, AI_NO_MODEL_CODE, "{c:?}");
        assert!(words.contains("Settings"), "the refusal says where to set it: {words}");
    }
    // a Writer whose provider is not connected is no model either
    core.store().write(|w| {
        w.tx.execute("INSERT INTO ai_provider (id, name, connected, updated_at_ms) VALUES ('openai', 'OpenAI', 0, 1)", [])?;
        Ok(w.tx.execute("INSERT INTO agent_config (id, name, provider_id, model, updated_at_ms) VALUES ('writer', 'Writer', 'openai', 'gpt-test', 1)", [])?)
    }).unwrap();
    let (c2, words) = code(&core, Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: None });
    assert_eq!(c2, AI_NO_MODEL_CODE);
    assert!(words.contains("OpenAI is not connected"), "{words}");
    assert_eq!(count(&core, "SELECT count(*) FROM reply_draft"), 1, "nothing was written");
}

#[test]
fn a_model_that_fails_leaves_no_half_draft_and_the_live_draft_as_it_was() {
    let fake = Fake::new(0, &[]);
    let (core, account, _d) = seeded(Some(fake.clone()));
    claire(&core, &account);
    let thread = listen(&core, StreamKind::Thread);
    let id = run(&core, Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: None }).created_id.unwrap();
    wait_until("the draft is ready", || thread.ready().contains(&id));
    fake.fail(CallKind::Draft);
    let lost = run(&core, Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: None }).created_id.unwrap();
    run(&core, Command::RequestReplyChange { draft_id: id.clone(), text: "shorter".into() });
    settled(&core);
    assert_eq!(core.store().reply_draft(&lost).unwrap(), None, "no half draft");
    let d = draft(&core, &id);
    assert_eq!((d.state, d.text.as_str(), d.revision), (ReplyDraftState::Proposed, ANSWER, 0), "the live draft is as it was");
    assert_eq!(count(&core, "SELECT count(*) FROM reply_draft"), 1);
}

/// The chosen model as a script: each call takes the next answer and records the prompt it was handed.
struct Scripted { answers: Mutex<Vec<Result<String, CoreError>>>, prompts: Mutex<Vec<String>> }
impl Completer for Scripted {
    fn complete(&self, _system: Option<String>, prompt: String, _max: u32) -> Result<String, CoreError> {
        self.prompts.lock().unwrap().push(prompt);
        let mut a = self.answers.lock().unwrap();
        if a.is_empty() { Err(CoreError::Unexpected { reason_words: "no more answers".into() }) } else { a.remove(0) }
    }
}
fn provider_said(words: &str, retryable: bool) -> CoreError { CoreError::Action { failure: ActionFailed { account_name: "OpenRouter".into(), reason_words: words.into(), retryable, code: None } } }

#[test]
fn the_chosen_model_is_retried_once_when_the_provider_says_later_never_on_a_refusal_and_cut_to_its_room() {
    let call = ModelCall { kind: CallKind::Draft, system: Some("sys".into()), prompt: "x".repeat(60_000) };
    let busy = Arc::new(Scripted { answers: Mutex::new(vec![Err(provider_said("overloaded", true)), Ok("Hello Claire".into())]), prompts: Mutex::default() });
    struct Shared(Arc<Scripted>);
    impl Completer for Shared { fn complete(&self, s: Option<String>, p: String, m: u32) -> Result<String, CoreError> { self.0.complete(s, p, m) } }
    let m = ProviderModel::new(Box::new(Shared(busy.clone())), false);
    assert_eq!(m.execute(&call).unwrap(), "Hello Claire");
    let prompts = busy.prompts.lock().unwrap().clone();
    assert_eq!(prompts.len(), 2, "one retry");
    assert_eq!(prompts[0].chars().count(), m.prompt_chars(), "the prompt is cut to the model's room");
    assert!(ProviderModel::new(Box::new(Shared(busy.clone())), true).prompt_chars() < m.prompt_chars(), "a model on this Mac has the smaller room");

    let refusing = Arc::new(Scripted { answers: Mutex::new(vec![Err(provider_said("invalid api key", false)), Ok("Hello".into())]), prompts: Mutex::default() });
    let m = ProviderModel::new(Box::new(Shared(refusing.clone())), false);
    match m.execute(&call) {
        Err(CoreError::Action { failure }) => assert_eq!(failure.reason_words, "invalid api key", "the provider's own words"),
        other => panic!("{other:?}"),
    }
    assert_eq!(refusing.prompts.lock().unwrap().len(), 1, "a refused key is not asked twice");
}

// ---------------------------------------------------------------- the port's default, kept provable

#[test]
fn a_core_with_no_work_installed_still_answers_the_not_built_yet_code() {
    let (core, account, _d) = seeded(Some(Fake::new(0, &[])));
    claire(&core, &account);
    core.store().write(|w| Ok(w.tx.execute("INSERT INTO reply_draft (id, thread_id, message_id, text, made_by, state, created_at_ms, updated_at_ms) VALUES ('d1', 't1', 'm1', 'Hello', 'on_demand', 'proposed', 1, 1)", [])?)).unwrap();
    install_reply_work(core.store().path(), Arc::new(NotBuiltYet));
    for c in [
        Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: None },
        Command::TryReply { message_id: "m1".into(), kind: ReplyUseKind::Auto },
        Command::RunVoiceAnalysis { sample_size: 10 },
        Command::RequestReplyChange { draft_id: "d1".into(), text: "shorter".into() },
        Command::AcceptReplyDraft { draft_id: "d1".into() },
        Command::DiscardReplyDraft { draft_id: "d1".into() },
    ] {
        assert_eq!(code(&core, c.clone()).0, "engine.not_built_yet", "{c:?}");
    }
    assert_eq!(draft(&core, "d1").state, ReplyDraftState::Proposed, "nothing changed");
}

// ---------------------------------------------------------------- calendars pushed by the shell (delta 21)

#[test]
fn the_calendars_the_shell_pushes_start_off_and_once_on_their_events_reach_the_writer() {
    let fake = Fake::new(0, &[]);
    let (core, account, _d) = seeded(Some(fake.clone()));
    claire(&core, &account);
    let work = ReplyCalendarInfo { calendar_id: "cal-work".into(), name: "Travail".into(), account_label: "iCloud".into(), colour: "#3366FF".into() };
    let at = now() + 86_400_000;
    let event = CalendarEvent { event_id: "e1".into(), calendar_id: "cal-work".into(), title: "Board meeting with Claire".into(), start_ms: at, end_ms: at + 3_600_000, attendees: vec!["Claire".into()], location: Some("Brussels".into()) };
    let push = |core: &RustCore| run(core, Command::SyncReplyCalendars { calendars: vec![work.clone()], window_start_ms: now() - 7 * 86_400_000, window_end_ms: now() + 14 * 86_400_000, events: vec![event.clone()] });
    push(&core);
    settled(&core);
    let source = ReplySourceRef::Calendar { calendar_id: "cal-work".into() };
    assert_eq!(count(&core, "SELECT count(*) FROM calendar_event"), 0, "a new calendar starts off: its events are not kept");
    assert_eq!(val(&core, "reply/sources")["calendars"][0]["enabled"], false);

    run(&core, Command::SetReplySourceEnabled { source: source.clone(), enabled: true });
    push(&core);
    settled(&core);
    assert_eq!(count(&core, "SELECT count(*) FROM calendar_event"), 1);
    assert_eq!(indexed_excerpts(core.store().path(), &source), 1, "the event is an excerpt");
    let thread = listen(&core, StreamKind::Thread);
    let id = run(&core, Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: None }).created_id.unwrap();
    wait_until("the draft is ready", || thread.ready().contains(&id));
    let prompt = fake.last(CallKind::Draft).prompt;
    assert!(prompt.contains("My calendar:") && prompt.contains("Board meeting with Claire") && !prompt.contains("No calendar is connected"), "the writer reads the calendar:\n{prompt}");

    run(&core, Command::SetReplySourceEnabled { source: source.clone(), enabled: false });
    settled(&core);
    assert_eq!(indexed_excerpts(core.store().path(), &source), 0, "a calendar switched off leaves the index");
}

// gap-replies (item 7604cdeb): what the writer is told about the calendar, read from the prompt of the one Draft call of the fake model.
// Red-first broken copies, each one line of core/crates/engine/src/replies_work/usecases/write_draft.rs:
//   no_calendar_*: `let calendar = if on.is_empty() { NO_CALENDAR.to_string() } else {` -> `if false`            (the no-calendar test is red)
//   no_calendar_*: `if events.is_empty() { NO_EVENT.to_string() }` -> `if false { NO_EVENT.to_string() }`      (the no-event half is red)
//   only_the_events_of_a_selected_*: sync_sources.rs:53 `.filter(|r| r.enabled)` -> `.filter(|r| true || r.enabled)`     (the unselected calendar's event is kept and read: red)
//   the_calendar_is_no_accounts_*: write_draft.rs `calendars.iter().filter(|c| c.enabled)` -> `calendars.iter().filter(|c| c.enabled && false)` (every draft loses the event: red)

fn calendar(id: &str, name: &str) -> ReplyCalendarInfo { ReplyCalendarInfo { calendar_id: id.into(), name: name.into(), account_label: "iCloud".into(), colour: "#3366FF".into() } }
fn event_in(id: &str, cal: &str, title: &str) -> CalendarEvent {
    let at = now() + 86_400_000;
    CalendarEvent { event_id: id.into(), calendar_id: cal.into(), title: title.into(), start_ms: at, end_ms: at + 3_600_000, attendees: vec![], location: None }
}
fn push_calendars(core: &RustCore, calendars: &[ReplyCalendarInfo], events: &[CalendarEvent]) {
    run(core, Command::SyncReplyCalendars { calendars: calendars.to_vec(), window_start_ms: now() - 7 * 86_400_000, window_end_ms: now() + 14 * 86_400_000, events: events.to_vec() });
    settled(core);
}
/// One draft on demand for `thread`, and the prompt the writer got for it.
fn writer_prompt(core: &RustCore, fake: &Fake, thread: &str) -> String {
    let events = listen(core, StreamKind::Thread);
    let id = run(core, Command::DraftReplyOnDemand { thread_id: thread.into(), instruction: None }).created_id.unwrap();
    wait_until("the draft is ready", || events.ready().contains(&id));
    fake.last(CallKind::Draft).prompt
}

#[test]
fn no_calendar_connected_tells_the_writer_not_to_invent_a_time_and_a_calendar_without_events_says_so() {
    let fake = Fake::new(0, &[]);
    let (core, account, _d) = seeded(Some(fake.clone()));
    claire(&core, &account);
    let none = writer_prompt(&core, &fake, "t1");
    assert!(none.contains("No calendar is connected") && none.contains("do not offer or accept a date or a time"), "no calendar: the writer must not offer a time:\n{none}");
    assert!(!none.contains("My calendar:") && !none.contains("holds no event"), "no calendar is not an empty calendar:\n{none}");

    // a calendar is pushed and switched on, with no event in the window: the writer is told it holds none (never that no calendar exists)
    push_calendars(&core, &[calendar("cal-work", "Travail")], &[]);
    run(&core, Command::SetReplySourceEnabled { source: ReplySourceRef::Calendar { calendar_id: "cal-work".into() }, enabled: true });
    push_calendars(&core, &[calendar("cal-work", "Travail")], &[]);
    let empty = writer_prompt(&core, &fake, "t1");
    assert!(empty.contains("My calendar holds no event in the coming days.") && !empty.contains("No calendar is connected"), "a calendar with no event:\n{empty}");
}

#[test]
fn only_the_events_of_a_selected_calendar_reach_the_writer_and_none_of_an_unselected_one() {
    let fake = Fake::new(0, &[]);
    let (core, account, _d) = seeded(Some(fake.clone()));
    claire(&core, &account);
    let (work, home) = (calendar("cal-work", "Travail"), calendar("cal-home", "Maison"));
    let events = [event_in("e-work", "cal-work", "Quarterly review"), event_in("e-home", "cal-home", "Dentist appointment")];
    push_calendars(&core, &[work.clone(), home.clone()], &events);
    run(&core, Command::SetReplySourceEnabled { source: ReplySourceRef::Calendar { calendar_id: "cal-work".into() }, enabled: true });
    push_calendars(&core, &[work.clone(), home.clone()], &events);   // the shell pushes again: the unselected calendar's events are still not kept
    assert_eq!(count(&core, "SELECT count(*) FROM calendar_event"), 1, "only the selected calendar's event is kept");
    let prompt = writer_prompt(&core, &fake, "t1");
    assert!(prompt.contains("Quarterly review"), "the selected calendar's event reaches the writer:\n{prompt}");
    assert!(!prompt.contains("Dentist appointment"), "an unselected calendar's event never reaches the writer:\n{prompt}");

    // the owner switches the other one on, and the first off: the writer follows the switches
    run(&core, Command::SetReplySourceEnabled { source: ReplySourceRef::Calendar { calendar_id: "cal-home".into() }, enabled: true });
    run(&core, Command::SetReplySourceEnabled { source: ReplySourceRef::Calendar { calendar_id: "cal-work".into() }, enabled: false });
    push_calendars(&core, &[work, home], &events);
    let swapped = writer_prompt(&core, &fake, "t1");
    assert!(swapped.contains("Dentist appointment") && !swapped.contains("Quarterly review"), "the switches decide what the writer reads:\n{swapped}");
}

#[test]
fn the_calendar_is_no_accounts_own_so_a_second_accounts_thread_reads_the_selected_calendar_too() {
    let fake = Fake::new(0, &[]);
    let (core, first, _d) = seeded(Some(fake.clone()));
    let second = core.store().add_account("second@example.test", "Second", MailProvider::Imap, "svc\u{1f}acct2", false, config(1, 1), 2).unwrap();
    assert_ne!(first, second);
    core.store().insert_messages(vec![mail(&second, "m9", "t9", "dana@client.test", "Schedule", "Can we meet next week? Dana", now() - 60_000, "INBOX")]).unwrap();
    let work = calendar("cal-work", "Travail");
    let events = [event_in("e-work", "cal-work", "Quarterly review")];
    push_calendars(&core, &[work.clone()], &events);
    run(&core, Command::SetReplySourceEnabled { source: ReplySourceRef::Calendar { calendar_id: "cal-work".into() }, enabled: true });
    push_calendars(&core, &[work], &events);
    let prompt = writer_prompt(&core, &fake, "t9");
    assert!(prompt.contains("Dana") && prompt.contains("Quarterly review"), "the second account's thread is answered with the selected calendar:\n{prompt}");
}

// ---------------------------------------------------------------- the test hold of the settings lane's tests

#[test]
fn without_the_hold_a_hand_seeded_count_is_replaced_by_the_real_figure() {
    let inbox_count = |core: &RustCore| val(core, "reply/sources")["accounts"][0]["folders"].as_array().and_then(|f| f.iter().find(|t| t["source"]["folder"] == "INBOX")).map(|t| t["item_count"].clone());
    let seed = |core: &RustCore, account: &str| {
        let inbox = ReplySourceRef::MailFolder { account_id: account.to_string(), folder: "INBOX".into() };
        core.store().reply_source_ensure(inbox.clone(), 1).unwrap();
        core.store().reply_source_progress(inbox, 12, ReplySyncProgress::UpToDate, 1).unwrap();
    };
    // held: the hand-seeded 12 stays
    let (held, account, _d) = seeded(Some(Fake::new(0, &[])));
    hold_background(held.store().path(), true);
    claire(&held, &account);
    seed(&held, &account);
    held.note_stored(vec!["m1".into()]);
    held.flush_filing();
    assert!(!settle(held.store().path(), Duration::from_millis(800)), "the held background did not run");
    assert_eq!(inbox_count(&held), Some(serde_json::json!(12)), "the hold keeps what the test seeded");
    hold_background(held.store().path(), false);
    // not held: the real figure (one message in INBOX) replaces the seeded 12
    let (core, account, _d2) = seeded(Some(Fake::new(0, &[])));
    claire(&core, &account);
    seed(&core, &account);
    core.note_stored(vec!["m1".into()]);
    core.flush_filing();
    settled(&core);
    assert_eq!(inbox_count(&core), Some(serde_json::json!(1)), "the real figure replaces a hand-seeded count");
}

// ---------------------------------------------------------------- the crawl's pause (sync-full-r1 round 2)

#[test]
fn the_crawls_pause_holds_the_auto_pass_but_never_a_draft_the_owner_asks_for() {
    let (core, account, _d) = seeded(Some(Fake::new(0, &["Contract"])));
    run(&core, Command::SetReplyUseEnabled { kind: ReplyUseKind::Auto, enabled: true });
    claire(&core, &account);
    core.background().hold_crawl(true);
    assert!(core.background().paused().is_some(), "the crawl reads paused");
    core.note_stored(vec!["m1".into()]);
    core.flush_filing();
    let thread = listen(&core, StreamKind::Thread);
    let id = run(&core, Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: None }).created_id.unwrap();
    wait_until("the draft the owner asked for, while the background is paused", || thread.ready().contains(&id));
    assert!(!settle(core.store().path(), Duration::from_millis(1500)), "the auto pass did not wait");
    assert_eq!(core.store().reply_pass_of("m1").unwrap(), None, "no auto pass while paused");
    core.background().hold_crawl(false);
    settled(&core);
    assert!(core.store().reply_pass_of("m1").unwrap().is_some(), "the auto pass ran once the pause ended");
}

// ---------------------------------------------------------------- the Writer's model over its real type

/// The providers' HTTP seam as a script: each request takes the next answer.
struct ScriptedHttp { answers: Mutex<Vec<ai_providers::transport::Response>>, seen: Mutex<usize> }
impl ai_providers::transport::Transport for ScriptedHttp {
    fn send(&self, _r: &ai_providers::transport::Request) -> Result<ai_providers::transport::Response, String> {
        *self.seen.lock().unwrap() += 1;
        let mut a = self.answers.lock().unwrap();
        if a.is_empty() { Err("no more answers".into()) } else { Ok(a.remove(0)) }
    }
}

#[test]
fn the_writers_model_from_settings_is_retried_once_over_its_real_type_with_each_jobs_own_timeout() {
    use secrets::{Secret, SecretKind, StoreSecret};
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    let keys = kc();
    let key = StoreSecret::execute(&*keys, SecretKind::ProviderCredential, "openrouter", &Secret::new("test-key")).unwrap();
    let key_ref = format!("{}\u{1f}{}", key.service, key.account);
    store.write(move |w| {
        w.tx.execute("INSERT INTO ai_provider (id, name, connected, keychain_ref, updated_at_ms) VALUES ('openrouter', 'OpenRouter', 1, ?1, 1)", [key_ref])?;
        Ok(w.tx.execute("INSERT INTO agent_config (id, name, provider_id, model, updated_at_ms) VALUES ('writer', 'Writer', 'openrouter', 'some/model', 1)", [])?)
    }).unwrap();
    let core = RustCore::with_parts(store, keys.clone(), detector(), TestTransport::PlainLoopback);
    let http = Arc::new(ScriptedHttp { answers: Mutex::new(vec![
        ai_providers::transport::Response { status: 503, body: r#"{"error":{"message":"overloaded"}}"#.into() },
        ai_providers::transport::Response { status: 200, body: r#"{"choices":[{"message":{"content":"Hello Claire"}}]}"#.into() },
    ]), seen: Mutex::new(0) });
    engine::ai_settings::set_scripted_transport(core.store().path(), Some(http.clone()));
    let keyring: Arc<dyn engine::Keyring> = keys;
    let (draft_job, voice_job) = (engine::ai_settings::model_for(&core.store(), &keyring, engine::ai_settings::AiJob::Draft).unwrap(), engine::ai_settings::model_for(&core.store(), &keyring, engine::ai_settings::AiJob::VoiceAnalysis).unwrap());
    assert_eq!((draft_job.timeout_ms, voice_job.timeout_ms), (engine::ai_settings::AI_DRAFT_TIMEOUT_MS, engine::ai_settings::AI_VOICE_TIMEOUT_MS), "each job has its own bound");
    claire(&core, &account);
    let thread = listen(&core, StreamKind::Thread);
    let id = run(&core, Command::DraftReplyOnDemand { thread_id: "t1".into(), instruction: None }).created_id.unwrap();
    wait_until("the draft is ready", || thread.ready().contains(&id));
    assert_eq!(draft(&core, &id).text, "Hello Claire");
    assert_eq!(*http.seen.lock().unwrap(), 2, "the 503 is asked once more, through the real ModelChoice");
    engine::ai_settings::set_scripted_transport(core.store().path(), None);
}
