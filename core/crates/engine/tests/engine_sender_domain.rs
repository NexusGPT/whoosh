//! SPEC A153 (the sender-domain filter, engine side): `from:@<domain>` inside the query of `Command::Search` keeps exactly the mails
//! whose SENDER address is at that domain (the domain itself or a sub-domain of it, case blind; a look-alike such as
//! `notlorvan-hotels.example` is not at `lorvan-hotels.example`), and `WhooshCore::sender_domains(prefix)` lists the domains of the
//! senders of RECEIVED mail that start with the letters typed, each with its people and mails, most mails first, at most 8.
//! Each test runs over the rig's production core with no model. Every test has a ceiling on its waits; the read's time is PRINTED.
//! No address or query is printed except the synthetic ones of this file's own mail (`.example`, `.test`).
//! Each test names its one-line broken copy (red first).
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use contracts::*;
use engine::RustCore;
use store::NewMessage;

const USER: &str = "owner@example.test";
const WAIT: Duration = Duration::from_secs(120);
/// How long a test waits to see that no further list came (no model: one send).
const QUIET: Duration = Duration::from_millis(600);
/// The owner's bar for any operation (the lead's standing rule): the read must answer inside it on 2,000 mails.
const READ_BUDGET: Duration = Duration::from_millis(50);

struct Lists(Mutex<Vec<(String, Vec<SearchHit>)>>);
impl EventSink for Lists {
    fn on_event(&self, e: CoreEvent) { if let CoreEvent::SearchResults { query, hits } = e { self.0.lock().unwrap().push((query, hits)); } }
}

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn mail(account: &str, id: &str, folder: &str, from: &str, to: &[&str], subject: &str, body: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: "Sender".into(), from_address: from.into(),
            to: to.iter().map(|s| s.to_string()).collect(), subject: subject.into(), preview: body.into(), received_at_ms: at, read: false, archived: false,
            folder: folder.into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: Some(body.into()), body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

fn open(dir: &std::path::Path) -> (RustCore, String) {
    let core = RustCore::open_production(dir.to_str().unwrap(), true).expect("the rig core opens");
    let a = core.store().add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    (core, a)
}

/// The people list is built beside the open: the tests wait for `people/ready` before they read.
fn wait_people(core: &RustCore) {
    let t0 = Instant::now();
    while core.state("people/ready".into()).as_deref() != Some("true") && t0.elapsed() < WAIT { std::thread::sleep(Duration::from_millis(20)); }
    assert_eq!(core.state("people/ready".into()).as_deref(), Some("true"), "the first build of the people list ends");
}

/// The one list a search sends with no model.
fn list(core: &RustCore, query: &str) -> Vec<SearchHit> {
    let lists = Arc::new(Lists(Mutex::new(vec![])));
    let sub = core.subscribe(StreamKind::Search, lists.clone());
    let answer = core.run(Command::Search { query: query.into() });
    assert!(answer.is_ok(), "the real engine answers Search {query:?} (it answered {answer:?})");
    let t0 = Instant::now();
    while lists.0.lock().unwrap().is_empty() && t0.elapsed() < WAIT { std::thread::sleep(Duration::from_millis(20)); }
    std::thread::sleep(QUIET);
    core.unsubscribe(sub);
    let got = lists.0.lock().unwrap().clone();
    assert!(!got.is_empty(), "a list for {query:?}");
    let (q, hits) = got.last().unwrap().clone();
    assert_eq!(q, query, "the query string as sent");
    hits
}
fn set(hits: &[SearchHit]) -> HashSet<String> { hits.iter().map(|h| h.message_id.clone()).collect() }
fn of(ids: &[&str]) -> HashSet<String> { ids.iter().map(|s| s.to_string()).collect() }

// Filter fixture. Lorvan: A (reservations@, Jan, "Booking invoice"), B (RESERVATIONS@LORVAN-HOTELS.EXAMPLE, Feb, "Your stay", the word
// "booking" in the text), S (mail.lorvan-hotels.example sub-domain, Mar, "Booking offer"). L: the look-alike notlorvan-hotels.example (Apr,
// "Booking invoice"). Z: zoe@acme.test (May, "Booking invoice"). T: the owner's mail in Sent to a lorvan address (Jun).
const A: &str = "a-lorvan-invoice";
const B: &str = "b-lorvan-stay";
const S: &str = "s-lorvan-sub";
const L: &str = "l-lookalike";
const Z: &str = "z-acme";
const T: &str = "t-sent-to-lorvan";
const DAY: i64 = 86_400_000;
const JAN: i64 = 1_767_225_600_000; // 2026-01-01T00:00:00Z

fn filter_core(dir: &std::path::Path) -> RustCore {
    let (core, a) = open(dir);
    core.store().insert_messages(vec![
        mail(&a, A, "INBOX", "reservations@lorvan-hotels.example", &[USER], "Booking invoice", "Your invoice.", JAN + 14 * DAY),
        mail(&a, B, "INBOX", "RESERVATIONS@LORVAN-HOTELS.EXAMPLE", &[USER], "Your stay", "The booking is confirmed.", JAN + 45 * DAY),
        mail(&a, S, "INBOX", "news@mail.lorvan-hotels.example", &[USER], "Booking offer", "An offer.", JAN + 73 * DAY),
        mail(&a, L, "INBOX", "x@notlorvan-hotels.example", &[USER], "Booking invoice", "Not the hotel.", JAN + 104 * DAY),
        mail(&a, Z, "INBOX", "zoe@acme.test", &[USER], "Booking invoice", "The booking numbers.", JAN + 134 * DAY),
        mail(&a, T, "Sent", USER, &["reservations@lorvan-hotels.example"], "Re: booking", "Thanks.", JAN + 165 * DAY),
    ]).unwrap();
    let bg = core.background();
    bg.reindex(&[A, B, S, L, Z, T].iter().map(|s| s.to_string()).collect::<Vec<_>>());
    assert!(bg.settle_index(WAIT), "the indexer finished");
    core
}

/// `from:@<domain>` keeps the mails whose sender is at the domain or a sub-domain of it, case blind, and drops a look-alike domain.
/// Broken copy: `sender_is_at` returns `host.ends_with(domain)` (the look-alike L is kept, a test below fails) or the filter compares
/// the address with `contains` as the person filter does (L kept too), or it ignores case (B dropped).
#[test]
fn a_the_domain_filter_keeps_its_senders_case_blind_and_drops_a_look_alike() {
    let dir = tempfile::tempdir().unwrap();
    let core = filter_core(dir.path());
    // the sub-domain is kept: the test engine's rule (SearchEngine.senderIsAt: `host == d || host.hasSuffix("." + d)`)
    assert_eq!(set(&list(&core, "from:@lorvan-hotels.example")), of(&[A, B, S]), "the domain, its upper-case sender and its sub-domain; not the look-alike, not the owner's Sent mail");
    assert_eq!(set(&list(&core, "from:@LORVAN-Hotels.Example")), of(&[A, B, S]), "the pill's letters in any case");
    assert_eq!(set(&list(&core, "FROM:@lorvan-hotels.example")), of(&[A, B, S]), "the key in any case");
    assert_eq!(set(&list(&core, "from:\"@lorvan-hotels.example\"")), of(&[A, B, S]), "in quotes");
    assert_eq!(set(&list(&core, "from:@mail.lorvan-hotels.example")), of(&[S]), "a sub-domain keeps only its own mail");
    assert_eq!(set(&list(&core, "from:@notlorvan-hotels.example")), of(&[L]), "the look-alike has its own mail");
    assert!(list(&core, "from:@hotels.example").is_empty(), "a part of a label is not a domain: nothing");
    assert!(list(&core, "from:@lorvan-hotels.exampl").is_empty(), "a cut-off domain keeps nothing");
}

/// The person form stays a person filter: `from:lorvan` (no @) still matches by name or address text, so it keeps the look-alike too.
/// Broken copy: the parser takes any `from:` value as a domain (the person form then keeps nothing).
#[test]
fn b_without_the_at_from_is_still_the_person_filter() {
    let dir = tempfile::tempdir().unwrap();
    let core = filter_core(dir.path());
    assert_eq!(set(&list(&core, "from:lorvan")), of(&[A, B, S, L]), "an address that holds the letters, as before");
    assert_eq!(set(&list(&core, "from:zoe")), of(&[Z]));
}

/// The domain pill combines with the other pills and with the words, as the other filters do.
/// Broken copy: the domain test is dropped when `subject:` or `after:` or words are present (A, B, S all come back).
#[test]
fn c_the_domain_filter_combines_with_other_pills_and_with_words() {
    let dir = tempfile::tempdir().unwrap();
    let core = filter_core(dir.path());
    assert_eq!(set(&list(&core, "from:@lorvan-hotels.example subject:booking")), of(&[A, S]), "with a subject pill: the lorvan mails whose subject holds booking; L and Z hold it too and are dropped");
    assert_eq!(set(&list(&core, "from:@lorvan-hotels.example after:2026-02-01")), of(&[B, S]), "with a date pill");
    assert_eq!(set(&list(&core, "from:@lorvan-hotels.example from:@mail.lorvan-hotels.example")), of(&[S]), "two domain pills must both hold");
    assert_eq!(set(&list(&core, "from:@lorvan-hotels.example from:reservations")), of(&[A, B]), "with a person pill");
    // with words: the filter scopes the list, the words rank inside it; Z and L hold "booking" too and stay out
    let hits = list(&core, "from:@lorvan-hotels.example booking");
    assert_eq!(set(&hits), of(&[A, B, S]), "the list holds the lorvan mails only");
    assert!(!hits.is_empty() && [A, B, S].contains(&hits[0].message_id.as_str()), "the first hit is a lorvan mail");
    // a word that only a dropped mail holds finds nothing inside the domain
    assert_eq!(set(&list(&core, "from:@acme.test numbers")), of(&[Z]), "the word and the domain both hold");
    assert!(set(&list(&core, "from:@lorvan-hotels.example numbers")).iter().all(|id| id != Z), "a word of another domain's mail does not bring that mail in");
}

/// `from:@` with no domain is no filter: it stays in the text as words (like every filter with an empty value).
/// Broken copy: an empty domain is pushed as a filter (`ends_with("")` keeps every mail, or none).
#[test]
fn d_an_empty_domain_is_no_filter() {
    let dir = tempfile::tempdir().unwrap();
    let core = filter_core(dir.path());
    let hits = list(&core, "from:@ invoice");
    // no filter: every mail in scope is listed (words rank, the rest follows)
    assert_eq!(set(&hits), of(&[A, B, S, L, Z, T]), "no filter: every mail in scope is listed, as for any words-only search");
}

/// The saved smart folders' reader (`exact_matches`) and the search share `parse_query` and `in_scope`: the parse is pinned in the
/// unit tests of search_cmd.rs (`domain_filter_tests`); here the search's own `search/last/query` keeps the text as typed.
/// Broken copy: the engine rewrites the query text (the state shows another text).
#[test]
fn e_the_last_query_state_keeps_the_text_as_typed() {
    let dir = tempfile::tempdir().unwrap();
    let core = filter_core(dir.path());
    let _ = list(&core, "from:@lorvan-hotels.example booking");
    assert_eq!(core.state("search/last/query".into()).as_deref(), Some("\"from:@lorvan-hotels.example booking\""));
}

// ---- the read ----

fn insert_from(core: &RustCore, account: &str, prefix: &str, senders: &[(&str, u32)]) {
    let mut rows = Vec::new();
    for (sender, n) in senders {
        for i in 0..*n { rows.push(mail(account, &format!("{prefix}:{sender}:{i}"), "INBOX", sender, &[USER], "Subject", "Body.", 1_000 + i64::from(i))); }
    }
    core.store().insert_messages(rows).unwrap();
}

fn row(d: &SenderDomain) -> (String, u32, u32) { (d.domain.clone(), d.people, d.mails) }
fn rows(core: &RustCore, prefix: &str) -> Vec<(String, u32, u32)> { core.sender_domains(prefix.into()).iter().map(row).collect() }

/// Counts and order: per domain the different sender addresses (any case is one address) and the mails, most mails first, then A to Z;
/// a sub-domain is a domain of its own here; the owner's own address and the people the owner wrote to are not senders of received mail.
/// Broken copy: `people` counts mails (or `mails` counts people); or the sort is ascending; or from_count is replaced by total()
/// (client.test, a recipient of the owner's Sent mail, shows up); or the address is not lower-cased (reservations counts twice).
#[test]
fn f_the_read_counts_people_and_mails_most_mails_first() {
    let dir = tempfile::tempdir().unwrap();
    let (core, a) = open(dir.path());
    insert_from(&core, &a, "m", &[("reservations@lorvan-hotels.example", 3), ("RESERVATIONS@Lorvan-Hotels.example", 1), ("billing@lorvan-hotels.example", 1)]);
    insert_from(&core, &a, "n", &[("news@mail.lorvan-hotels.example", 1), ("x@notlorvan-hotels.example", 2)]);
    insert_from(&core, &a, "o", &[("zoe@acme.test", 2), ("bob@acme.test", 1), ("al@beta.test", 3)]);
    insert_from(&core, &a, "t", &[("q@tie-b.test", 2), ("q@tie-a.test", 2)]);
    core.store().insert_messages(vec![mail(&a, "sent1", "Sent", USER, &["jean@client.test"], "Re", "Thanks.", 9_000)]).unwrap();
    wait_people(&core);
    assert_eq!(rows(&core, "lorvan"), vec![("lorvan-hotels.example".to_string(), 2, 5)], "reservations (any case) and billing: two people, five mails; the sub-domain and the look-alike do not start with lorvan");
    assert_eq!(rows(&core, "a"), vec![("acme.test".to_string(), 2, 3)], "domains that start with a: acme.test only (the owner's and client.test never)");
    assert_eq!(rows(&core, "b"), vec![("beta.test".to_string(), 1, 3)]);
    // equal mails: domain A to Z (broken copy: no tie-break, the order then follows the hash map)
    assert_eq!(rows(&core, "tie"), vec![("tie-a.test".to_string(), 1, 2), ("tie-b.test".to_string(), 1, 2)], "two mails each: A to Z");
    assert!(rows(&core, "").is_empty(), "an empty prefix answers nothing");
    assert!(rows(&core, "client").is_empty(), "a person the owner only wrote to is not a sender");
    assert!(rows(&core, "example.test").is_empty(), "the owner's own domain, from the owner's own mail, is not offered");
}

/// Prefix with and without "@", in any case; empty and "@"-only prefixes answer nothing; a prefix in the middle of a domain does not match.
/// Broken copy: the prefix is not trimmed of "@" (no row for "@lou"), or `contains` replaces `starts_with` ("hotels" matches), or the
/// empty prefix answers every domain.
#[test]
fn g_the_prefix_with_or_without_at_any_case_and_the_empty_prefix() {
    let dir = tempfile::tempdir().unwrap();
    let (core, a) = open(dir.path());
    insert_from(&core, &a, "m", &[("reservations@lorvan-hotels.example", 2), ("zoe@acme.test", 1)]);
    wait_people(&core);
    let want = vec![("lorvan-hotels.example".to_string(), 1, 2)];
    for p in ["lou", "@lou", "LOU", "@Lorvan-Hotels.Example", "lorvan-hotels.example", " @lou "] { assert_eq!(rows(&core, p), want, "prefix {p:?}"); }
    for p in ["hotels", "ouvre", "lorvan-hotels.examples", "@@"] { assert!(rows(&core, p).is_empty(), "prefix {p:?} is not the start of a domain (or is empty)"); }
    for p in ["", "@", "  ", " @ "] { assert!(rows(&core, p).is_empty(), "prefix {p:?} answers nothing"); }
}

/// At most 8 rows, the 8 with the most mails, in order.
/// Broken copy: SENDER_DOMAINS_MAX is dropped (12 rows) or the cut comes before the sort (a lower-count domain is kept).
#[test]
fn h_the_read_answers_at_most_eight_domains_the_busiest_first() {
    let dir = tempfile::tempdir().unwrap();
    let (core, a) = open(dir.path());
    // cap00.test has 1 mail ... cap11.test has 12
    let senders: Vec<(String, u32)> = (0..12u32).map(|i| (format!("s@cap{i:02}.test"), i + 1)).collect();
    let refs: Vec<(&str, u32)> = senders.iter().map(|(s, n)| (s.as_str(), *n)).collect();
    insert_from(&core, &a, "c", &refs);
    wait_people(&core);
    let got = rows(&core, "cap");
    assert_eq!(got.len(), 8, "at most 8 rows");
    let want: Vec<(String, u32, u32)> = (4..12u32).rev().map(|i| (format!("cap{i:02}.test"), 1, i + 1)).collect();
    assert_eq!(got, want, "the eight busiest, most mails first");
}

/// The read sees mail stored after the first build (the people list is kept up to date) and costs under 50 ms on 2,000 mails: the time
/// is PRINTED (`sender_domains_ms`).
/// Broken copy: the read builds its own table from the store each call (a full scan: time over the budget, or the new mail is missed
/// when the answer is cached at build time).
#[test]
fn i_the_read_is_fresh_and_under_fifty_ms_on_two_thousand_mails() {
    let dir = tempfile::tempdir().unwrap();
    let (core, a) = open(dir.path());
    // 2,000 mails from 500 addresses at 100 domains (dom000.test .. dom099.test), mail counts differing by domain
    let mut rows_in = Vec::new();
    for i in 0..2000u32 {
        let d = (i * 7) % 100;
        let u = i % 5;
        rows_in.push(mail(&a, &format!("bulk{i}"), "INBOX", &format!("u{u}@dom{d:03}.test"), &[USER], "Subject", "Body.", 1_000 + i64::from(i)));
    }
    core.store().insert_messages(rows_in).unwrap();
    wait_people(&core);
    let t0 = Instant::now();
    let got = core.sender_domains("dom".into());
    let first = t0.elapsed();
    assert_eq!(got.len(), 8, "the cap");
    assert!(got.windows(2).all(|w| w[0].mails >= w[1].mails), "most mails first");
    // new mail after the build: a new domain shows at once
    core.store().insert_messages(vec![mail(&a, "late", "INBOX", "late@zzz-new.test", &[USER], "Subject", "Body.", 9_999_000)]).unwrap();
    let t1 = Instant::now();
    let fresh = core.sender_domains("zzz".into());
    let second = t1.elapsed();
    assert_eq!(fresh.iter().map(row).collect::<Vec<_>>(), vec![("zzz-new.test".to_string(), 1, 1)], "mail stored after the first build is read");
    let t2 = Instant::now();
    let _ = core.sender_domains("d".into());
    let third = t2.elapsed();
    println!("sender_domains_ms: read 1 {:.2} ms, read after new mail {:.2} ms, wide prefix {:.2} ms, on 2000 mails (budget {} ms)",
        first.as_secs_f64() * 1000.0, second.as_secs_f64() * 1000.0, third.as_secs_f64() * 1000.0, READ_BUDGET.as_millis());
    for (what, t) in [("first read", first), ("read after new mail", second), ("wide prefix", third)] {
        assert!(t < READ_BUDGET, "{what} took {t:?}, over the {READ_BUDGET:?} budget");
    }
}
