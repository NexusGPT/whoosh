//! match-inside (SPEC A149): on the real engine, a typed word matches every mail that holds its letters, also inside a longer word
//! or an address, in the subject, the sender's name and address, and the text; the match group comes first in the order of A149
//! (the word itself, then a word that starts with it, then a word that holds it; the subject or sender before the text; then
//! newest first), then the other mails as before. The first list stays inside the owner's 50 ms. Each test runs over the rig's
//! production core with no model (the first list is the answer) and reads the lists sent on the Search stream. Each test names its
//! broken copy: the one-line change that turns it red. No test prints a mail's text or a query.
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use engine::RustCore;
use store::NewMessage;

const USER: &str = "owner@example.test";
/// A ceiling on waits for background work, never a budget (the indexer runs at the OS background priority).
const WAIT: Duration = Duration::from_secs(300);
/// The owner's bound: "The objective is <50ms for any operation".
const BUDGET: Duration = Duration::from_millis(50);

/// Every `SearchResults` the core sent, in order, with when it arrived.
struct Lists(Mutex<Vec<(String, Vec<SearchHit>, Instant)>>);
impl EventSink for Lists {
    fn on_event(&self, e: CoreEvent) {
        if let CoreEvent::SearchResults { query, hits } = e { self.0.lock().unwrap().push((query, hits, Instant::now())); }
    }
}
impl Lists {
    fn first_for(&self, query: &str) -> Option<(Vec<SearchHit>, Instant)> {
        self.0.lock().unwrap().iter().find(|(q, _, _)| q == query).map(|(_, h, t)| (h.clone(), *t))
    }
}

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT {
        if ok() { return; }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("never reached within {WAIT:?}: {what}");
}

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn mail(account: &str, id: &str, sender: (&str, &str), subject: &str, body: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: sender.0.into(), from_address: sender.1.into(),
            to: vec![USER.into()], subject: subject.into(), preview: body.chars().take(60).collect(), received_at_ms: at, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: Some(body.into()), body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

/// The rig's production core over `dir` with `make(account)` stored, every body handed to the indexer, and the indexer settled.
fn production(dir: &std::path::Path, make: impl FnOnce(&str) -> Vec<NewMessage>) -> RustCore {
    let core = RustCore::open_production(dir.to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    let rows = make(&a);
    let ids: Vec<String> = rows.iter().map(|r| r.message.id.clone()).collect();
    store.insert_messages(rows).unwrap();
    let bg = core.background();
    bg.reindex(&ids);
    assert!(bg.settle_index(WAIT), "the indexer finished");
    core
}

/// Runs the search and returns its first list and the time from the command to that list.
fn first_list(core: &RustCore, lists: &Arc<Lists>, query: &str) -> (Vec<SearchHit>, Duration) {
    let t0 = Instant::now();
    assert!(core.run(Command::Search { query: query.into() }).is_ok(), "the engine answers Search");
    wait_until("the first list", || lists.first_for(query).is_some());
    let (hits, at) = lists.first_for(query).unwrap();
    (hits, at.duration_since(t0))
}

fn subscribed(core: &RustCore) -> Arc<Lists> {
    let lists = Arc::new(Lists(Mutex::new(vec![])));
    core.subscribe(StreamKind::Search, lists.clone());
    lists
}

fn ids(hits: &[SearchHit]) -> Vec<String> { hits.iter().map(|h| h.message_id.clone()).collect() }
fn exact(hits: &[SearchHit]) -> Vec<String> { ids(&hits[..hits.iter().position(|h| h.matched_in == MatchedIn::MeaningOnly).unwrap_or(hits.len())]) }

const PLAIN: (&str, &str) = ("Ops", "ops@ext.test");
const T: i64 = 1_700_000_000_000;

/// The match group in the order of SPEC A149, though the dates run the other way: the word itself (subject, then text), a word
/// that starts with it (subject, then text), a word that holds it (subject or sender, the newest first; then text); then the mail
/// that holds nothing, as a meaning row.
/// Broken copy (red): in core/crates/engine/src/search_cmd.rs `exact_group`, `.then_with(|| at.get(b.message_id.as_str()).cmp(&at.get(a.message_id.as_str())))`
/// -> `.then_with(|| at.get(a.message_id.as_str()).cmp(&at.get(b.message_id.as_str())))` (oldest first inside a rank).
#[test]
fn a_the_match_group_is_in_the_order_of_a149_then_newest_first() {
    let dir = tempfile::tempdir().unwrap();
    let core = production(dir.path(), |a| vec![
        mail(a, "whole-head", PLAIN, "Consultant visit", "Hello.", T),
        mail(a, "whole-text", PLAIN, "Hello", "Our consultant arrives.", T + 1_000),
        mail(a, "start-head", PLAIN, "Consultants visit", "Hello.", T + 2_000),
        mail(a, "start-text", PLAIN, "Hello", "The consultants arrive.", T + 3_000),
        mail(a, "inside-sender-older", ("News", "news@brightconsultants.example"), "Hello", "Hello.", T + 4_000),
        mail(a, "inside-head-newer", PLAIN, "Quote from brightconsultants.example", "Hello.", T + 5_000),
        mail(a, "inside-text", PLAIN, "Hello", "Write to brightconsultantsusa.example today.", T + 6_000),
        mail(a, "nothing", PLAIN, "Lunch", "Salmon and rice.", T + 7_000),
    ]);
    let lists = subscribed(&core);
    let (hits, _) = first_list(&core, &lists, "consultant");
    let want = ["whole-head", "whole-text", "start-head", "start-text", "inside-head-newer", "inside-sender-older", "inside-text"];
    assert_eq!(exact(&hits), want.iter().map(|s| s.to_string()).collect::<Vec<_>>(), "the match group of SPEC A149");
    assert_eq!(ids(&hits[want.len()..]), vec!["nothing".to_string()], "then the other mail");
    let by = |id: &str| hits.iter().find(|h| h.message_id == id).unwrap().matched_in.clone();
    assert_eq!(by("inside-sender-older"), MatchedIn::SenderWords { name_ranges: vec![], address_ranges: vec![TextRange { start: 11, end: 21 }] }, "the sender's address, the letters");
    assert_eq!(by("inside-head-newer"), MatchedIn::SubjectWords { ranges: vec![TextRange { start: 17, end: 27 }] }, "the subject, the letters");
    let text = hits.iter().find(|h| h.message_id == "inside-text").unwrap();
    let MatchedIn::TextWords { ranges } = &text.matched_in else { panic!("a text match: {:?}", text.matched_in) };
    assert_eq!(&text.snippet[ranges[0].start as usize..ranges[0].end as usize], "consultant", "the letters in the passage");
}

/// The owner's mail of 2026-11-11 in counts: "consultant" puts the three mails holding its letters (one word that starts with it,
/// two address-like words of 17 and 20 letters) in the match group, and not the mails of "consultancy" or "consulting".
/// Broken copy (red): in core/crates/search-text/src/query.rs `forms_of`, `while scanned < SUBSTRING_SCAN_MAX && all.advance() {`
/// -> `while false && all.advance() {` (the match group falls back to the one mail of the prefix rule).
#[test]
fn b_consultant_puts_the_three_mails_holding_its_letters_in_the_match_group() {
    let dir = tempfile::tempdir().unwrap();
    let core = production(dir.path(), |a| vec![
        mail(a, "digest", ("Daily Digest", "digest@news.test"), "Your daily digest", "Sources and journalists wanted.", T + 9_000),
        mail(a, "plural", PLAIN, "Hello", "Meet our consultants on Monday.", T + 1_000),
        mail(a, "addr17", PLAIN, "Your quote from brightconsultants.example", "Hello.", T + 2_000),
        mail(a, "addr20", PLAIN, "Reply from brightconsultantsusa.example", "Hello.", T + 3_000),
        mail(a, "consultancy", PLAIN, "Consultancy rates", "The consultancy fee.", T + 4_000),
        mail(a, "consulting", PLAIN, "Consulting services", "Our consulting team.", T + 5_000),
    ]);
    let lists = subscribed(&core);
    let (hits, _) = first_list(&core, &lists, "consultant");
    let group: HashSet<String> = exact(&hits).into_iter().collect();
    assert_eq!(group, ["plural", "addr17", "addr20"].iter().map(|s| s.to_string()).collect(), "the three mails that hold the letters");
    assert_eq!(hits.len(), 6, "every mail listed");
}

/// A deterministic mailbox of `n` mails with realistic word counts (the generator of search-text's tests/inside.rs): subjects of
/// 4 to 9 words, bodies of 80 to 400 words drawn with a skew from 6000 made-up words, some holding the letters "con"; one mail in
/// 20 holds a form of "consultant".
fn mailbox(account: &str, n: usize) -> Vec<NewMessage> {
    const SYLLABLES: [&str; 24] = ["con", "sul", "tant", "ma", "ri", "no", "ve", "lo", "pa", "ter", "in", "for", "ion", "de", "ble", "ex", "pro", "ment", "al", "ly", "sta", "gu", "ken", "bor"];
    let mut seed: u64 = 0x5eed_1234_abcd_0001;
    let mut next = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as usize
    };
    let vocabulary: Vec<String> = (0..6000).map(|_| {
        let parts = 2 + next() % 3;
        (0..parts).map(|_| SYLLABLES[next() % SYLLABLES.len()]).collect::<String>()
    }).collect();
    let word = |next: &mut dyn FnMut() -> usize| -> String {
        let r = (next() % 10_000) as f64 / 10_000.0;
        vocabulary[((r * r * r) * vocabulary.len() as f64) as usize].clone()
    };
    let forms = ["consultant", "consultants", "brightconsultants.example", "consultantsusa"];
    (0..n).map(|i| {
        let subject_words = 4 + next() % 6;
        let mut subject: Vec<String> = (0..subject_words).map(|_| word(&mut next)).collect();
        let body_words = 80 + next() % 321;
        let mut body: Vec<String> = (0..body_words).map(|_| word(&mut next)).collect();
        if i % 20 == 0 {
            let f = forms[(i / 20) % forms.len()].to_string();
            if i % 40 == 0 { subject.push(f) } else { body.insert(body.len() / 2, f) }
        }
        let name = format!("{} {}", word(&mut next), word(&mut next));
        let address = format!("{}@{}.com", word(&mut next), word(&mut next));
        mail(account, &format!("m{i}"), (name.as_str(), address.as_str()), &subject.join(" "), &body.join(" "), T + i as i64 * 60_000)
    }).collect()
}

/// The first list (the match group, then every other mail by date) arrives within 50 ms of the command on 2000 mails, for a
/// 3-letter and a 10-letter word. Prints each time (never the word). One search for another word runs first, unmeasured (the
/// first search of a launch opens the read connections); its time is printed too.
/// Broken copy (red): in core/crates/search-text/src/query.rs `run_ranked`, after `let searcher = reader.searcher();` add
/// `std::thread::sleep(std::time::Duration::from_millis(60));`.
#[test]
fn c_the_first_list_arrives_within_50_ms_on_2000_mails() {
    let dir = tempfile::tempdir().unwrap();
    let core = production(dir.path(), |a| mailbox(a, 2000));
    let lists = subscribed(&core);
    let (_, warm) = first_list(&core, &lists, "zzzz");
    println!("first list, warm-up search: {:.2} ms", warm.as_secs_f64() * 1000.0);
    for (label, typed) in [("3-letter word", "con"), ("10-letter word", "consultant")] {
        let (hits, took) = first_list(&core, &lists, typed);
        let group = exact(&hits).len();
        println!("first list, {label}: {group} matches, {} rows, in {:.2} ms on 2000 mails", hits.len(), took.as_secs_f64() * 1000.0);
        assert!(group > 0, "{label}: the mailbox holds it");
        assert!(took < BUDGET, "{label}: the first list took {took:?}, over {BUDGET:?}");
    }
}
