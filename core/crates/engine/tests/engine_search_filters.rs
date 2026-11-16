//! search-forms part 2: the exact filters of the owner's search (`from:` `to:` `subject:` `attachment:` `label:` `before:` `after:`
//! `has:attachment` inside the query of `Command::Search`). The engine keeps only the mails that pass every filter, then ranks the
//! kept mails by the words left. Each test runs over the rig's production core with no model (the filters are exact; the meaning
//! order is part 1's) and reads the list sent on the Search stream. On c83fce8c each fails: the filter text was searched as words
//! and the whole mail was listed.
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

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}
/// Noon UTC of a day: any local offset keeps it inside that local day's neighbours, far from the filters' month boundaries.
fn noon(y: i64, m: i64, d: i64) -> i64 { days_from_civil(y, m, d) * 86_400_000 + 43_200_000 }

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

#[allow(clippy::too_many_arguments)]
fn mail(account: &str, id: &str, folder: &str, sender: (&str, &str), to: &[&str], cc: &[&str], subject: &str, body: &str, at: i64) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("{id}:thread"), account_id: account.into(), sender_name: sender.0.into(), from_address: sender.1.into(),
            to: to.iter().map(|s| s.to_string()).collect(), subject: subject.into(), preview: body.into(), received_at_ms: at, read: false, archived: false,
            folder: folder.into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: cc.iter().map(|s| s.to_string()).collect(), bcc: vec![], is_outgoing: false,
        },
        remote_id: None, body_text: Some(body.into()), body_html: None, has_list_header: false, size_bytes: None, tab_id: None,
    }
}

// A: from Zoë Martin, Jan 15, "Quarterly report", the file report-q3.pdf, label Work.
// B: from Bob Stone, Feb 15, Ann Lee in Cc, "Lunch plans".
// C: the owner's own mail in Sent, Mar 15, to Jean Dupont, "Re: report".
// D: from zoe@acme.test signed "Z. Martin", Apr 15, "Report draft".
const A: &str = "a-zoe-report";
const B: &str = "b-bob-lunch";
const C: &str = "c-sent-to-jean";
const D: &str = "d-zoe-draft";

fn core_with_mail(dir: &std::path::Path) -> RustCore {
    let core = RustCore::open_production(dir.to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    let a = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages(vec![
        mail(&a, A, "INBOX", ("Zoë Martin", "zoe@acme.test"), &[USER], &[], "Quarterly report", "The report numbers are in.", noon(2026, 1, 15)),
        mail(&a, B, "INBOX", ("Bob Stone", "bob@other.test"), &[USER], &["Ann Lee <ann@acme.test>"], "Lunch plans", "Salmon at noon.", noon(2026, 2, 15)),
        mail(&a, C, "Sent", ("Owner", USER), &["Jean Dupont <jean.dupont@client.test>"], &[], "Re: report", "The report is attached.", noon(2026, 3, 15)),
        mail(&a, D, "INBOX", ("Z. Martin", "zoe@acme.test"), &[USER], &[], "Report draft", "A draft report for you.", noon(2026, 4, 15)),
    ]).unwrap();
    store.put_message_attachments(A, vec![AttachmentFile { id: format!("{A}:att:000"), message_id: A.into(), file_name: "report-q3.pdf".into(), mime_type: "application/pdf".into(), size_bytes: 10, local_path: None, inline: false, content_id: None }]).unwrap();
    let work = store.label_create(store::NewLabel { id: None, name: "Work".into(), parent_id: None, colour: None, account_ids: vec![], now_ms: 1 }).unwrap();
    store.label_assign(work.id, vec![A.to_string()], 1).unwrap();
    let bg = core.background();
    bg.reindex(&[A, B, C, D].iter().map(|s| s.to_string()).collect::<Vec<_>>());
    assert!(bg.settle_index(WAIT), "the indexer finished");
    core
}

/// The one list a search sends with no model (it asserts the command is answered).
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

/// Each filter alone keeps exactly the mails that pass it; accents and case are folded; a value with a space is quoted.
#[test]
fn a_each_filter_alone_keeps_exactly_its_mails() {
    let dir = tempfile::tempdir().unwrap();
    let core = core_with_mail(dir.path());
    let cases: [(&str, &[&str]); 13] = [
        ("from:zoe", &[A, D]),
        ("from:\"zoë martin\"", &[A]),
        ("FROM:\"ZOE MARTIN\"", &[A]),
        ("to:ann", &[B]),
        ("subject:report", &[A, C, D]),
        ("attachment:report", &[A]),
        ("label:work", &[A]),
        ("before:2026-02-01", &[A]),
        ("after:2026-03-31", &[D]),
        ("after:2026-03-15", &[C, D]),
        ("after:2026-01-01 before:2026-02-01", &[A]),
        ("has:attachment", &[A]),
        ("subject:\"quarterly report\"", &[A]),
    ];
    for (query, want) in cases {
        let hits = list(&core, query);
        assert_eq!(set(&hits), of(want), "{query}");
    }
}

/// `to:` finds a mail the owner sent (the recipient of the Sent folder's mail).
#[test]
fn b_to_finds_a_mail_the_owner_sent() {
    let dir = tempfile::tempdir().unwrap();
    let core = core_with_mail(dir.path());
    assert_eq!(set(&list(&core, "to:jean")), of(&[C]));
    assert_eq!(set(&list(&core, "to:\"jean dupont\"")), of(&[C]), "a value with a space, in quotes");
}

/// Two filters together keep the mails that pass both.
#[test]
fn c_two_filters_keep_the_mails_that_pass_both() {
    let dir = tempfile::tempdir().unwrap();
    let core = core_with_mail(dir.path());
    assert_eq!(set(&list(&core, "from:zoe subject:draft")), of(&[D]));
    assert_eq!(set(&list(&core, "from:zoe after:2026-01-31")), of(&[D]));
    assert_eq!(set(&list(&core, "subject:report has:attachment")), of(&[A]));
}

/// A filter plus words: a mail that holds the words but fails the filter (C, the owner's "report") is absent from the WHOLE list.
#[test]
fn d_a_filter_plus_words_leaves_out_a_mail_that_fails_the_filter() {
    let dir = tempfile::tempdir().unwrap();
    let core = core_with_mail(dir.path());
    let hits = list(&core, "from:zoe report");
    assert_eq!(set(&hits), of(&[A, D]), "only the kept mails, ranked by the words: {:?}", hits.iter().map(|h| &h.message_id).collect::<Vec<_>>());
    assert!(hits.iter().all(|h| h.matched_in != MatchedIn::MeaningOnly), "both hold the word: the exact group");
}

/// Filters with no words: the kept mails by date, newest first, all in the exact group; state stage "filtered".
#[test]
fn e_filters_with_no_words_list_the_kept_mails_by_date_as_exact_matches() {
    let dir = tempfile::tempdir().unwrap();
    let core = core_with_mail(dir.path());
    let hits = list(&core, "subject:report");
    assert_eq!(hits.iter().map(|h| h.message_id.as_str()).collect::<Vec<_>>(), vec![D, C, A], "date order, newest first");
    assert!(hits.iter().all(|h| h.matched_in != MatchedIn::MeaningOnly), "all in the exact group");
    assert_eq!(core.state("search/last/stage".into()).as_deref(), Some("\"filtered\""));
    assert_eq!(core.state("search/last/reason".into()).as_deref(), Some("null"), "nothing to say above a filtered list");
}
