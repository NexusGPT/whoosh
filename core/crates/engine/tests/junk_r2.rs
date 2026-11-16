//! junk-r2: the local spam filter wired into the real core. Through the real sync over the in-process IMAP fake (sync-imap's test
//! support): a live arrival on INBOX is scored and moved, suspected or kept by the ledger's cut-offs; the history, the first screen and a
//! rebuilt folder are never scored; an arrival already read or flagged is never scored; the first run trains once; MarkSpam's lesson is
//! learned once; the two headers reach the filter.
//!
//! MUTANTS (one edit each, run by _captures/junk-r2/mutants.sh; each must turn the test named red):
//!   J1 sync.rs Delta branch: `store_arrivals(.., |f| !held.contains(..))` -> `|_| false` (live arrivals never scored)
//!        -> an_idle_arrival_that_fails_its_headers_is_moved_to_junk_on_the_server
//!   J2 sync.rs Reset branch: the live test's `live_uv == Some(f.uid_validity) &&` removed (old mail of a rebuilt folder is scored)
//!        -> a_rebuilt_folder_is_never_mass_moved
//!   J3 junk_filter.rs `eligible`: the `\Seen` test dropped -> an_arrival_already_read_elsewhere_is_never_scored
//!   J4 junk_filter.rs `learn`: `Some(p) if p == lesson => continue` -> `Some(p) if p == lesson => f.learn(&msg, label(lesson))`
//!        -> mark_spam_learns_once_and_a_changed_mind_relearns
//!   J5 envelope.rs: `header_values(r.header, "authentication-results")` -> `Vec::new()` -> the_headers_reach_the_filter (and J1's test)
//!   J6 sync.rs run_account: the `crate::junk_filter::bootstrap(&d, &id);` line removed -> the_first_run_trains_once_from_junk_and_inbox
//!        and the_first_run_corpus_moves_a_phish_its_headers_alone_only_suspect
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::sync::Arc;
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::junk_filter::{decide, filter_for, learn, thresholds};
use engine::{RustCore, TestTransport};
use imap_fakes::{smtp::FakeSmtp, FakeImap};
use junk::{Context, Decision, Filter, Model};
use secrets::{MemoryKeychain, Namespace};
use store::{JunkLesson, JunkState, NewMessage, Store};
use sync_imap::FetchedMessage;

const USER: &str = "owner@example.test";
const PW: &str = "app-password";

fn detector() -> Box<dyn engine::Detect> { Box::new(autoconfig::Detector::new(FakeHttp::default(), FakeDns::default(), FakeProbe::default())) }
fn kc() -> Arc<MemoryKeychain> { Arc::new(MemoryKeychain::new(Namespace::rig())) }
fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(20) {
        if ok() { return; }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("never reached within 20 s: {what}");
}
fn config(imap_port: u16, smtp_port: u16) -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

/// The phish: SPF, DKIM and DMARC fail in the receiving server's own (topmost) header, the display name carries a bank's address while
/// the mail comes from another site, and Reply-To is a third site. Alone, the header rules give 1 - 0.7 * 0.7 * 0.5 * 0.45 * 0.75 = 0.917.
fn phish(n: u32) -> Vec<u8> {
    format!("Authentication-Results: mx.owner.example; spf=fail smtp.mailfrom=secure-login.example;\r\n dkim=fail header.d=secure-login.example; dmarc=fail header.from=secure-login.example\r\n\
             Authentication-Results: forged.example; spf=pass; dkim=pass; dmarc=pass\r\n\
             From: \"service@bank.example\" <alerts{n}@secure-login.example>\r\nReply-To: collect@payouts.example\r\nTo: {USER}\r\n\
             Subject: Your account is locked {n}\r\nMessage-ID: <phish-{n}@secure-login.example>\r\nDate: Sat, 07 Nov 2026 10:00:00 +0000\r\n\r\n\
             Verify your account now or it closes today.\r\n").into_bytes()
}
/// The same phish with no Reply-To: 1 - 0.7 * 0.7 * 0.5 * 0.45 = 0.890, between the two cut-offs.
fn phish_no_reply_to(n: u32) -> Vec<u8> {
    String::from_utf8(phish(n)).unwrap().replace("Reply-To: collect@payouts.example\r\n", "").into_bytes()
}
fn plain(n: u32) -> Vec<u8> {
    format!("From: Ann Lee <ann@friends.example>\r\nTo: {USER}\r\nSubject: Lunch on Friday {n}\r\nMessage-ID: <plain-{n}@friends.example>\r\nDate: Sat, 07 Nov 2026 10:00:00 +0000\r\n\r\nShall we meet at noon?\r\n").into_bytes()
}

/// Puts one raw message in a mailbox of the fake and wakes an IDLE on it (the fake announces a changed mailbox).
fn deliver(imap: &FakeImap, mbox: &str, raw: Vec<u8>, flags: &[&str]) -> u32 {
    let uid = imap.with(|s| {
        let b = s.boxes.iter_mut().find(|b| b.name == mbox).unwrap();
        b.highest_modseq += 1;
        let uid = b.next_uid;
        b.next_uid += 1;
        b.msgs.push(imap_fakes::Msg { uid, flags: flags.iter().map(|f| f.to_string()).collect(), modseq: b.highest_modseq, raw, gm: None });
        b.version += 1;
        uid
    });
    imap.change_flags(mbox, &[], "\\Seen");
    uid
}

struct Synced { core: RustCore, account: String, imap: FakeImap, _smtp: FakeSmtp, _rt: tokio::runtime::Runtime, store_path: std::path::PathBuf, _dir: tempfile::TempDir }

/// An account added and synced to its history over the fake, listening by IDLE. `prepare` fills the server before the account is added.
fn synced(condstore: bool, prepare: impl FnOnce(&FakeImap)) -> Synced {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let imap = rt.block_on(FakeImap::start());
    imap.with(|s| { s.qresync = condstore; s.cap_condstore = condstore; });
    prepare(&imap);
    let smtp = rt.block_on(FakeSmtp::start(USER, PW));
    let dir = tempfile::tempdir().unwrap();
    let store_path = dir.path().join("whoosh.sqlite");
    let core = RustCore::with_parts(Store::open(&store_path).unwrap(), kc(), detector(), TestTransport::PlainLoopback);
    let account = core.run(Command::AddAccountConfigured { address: USER.into(), display_name: Some("Owner".into()), config: config(imap.port, smtp.port), password: SecretText { value: PW.into() } }).expect("added").created_id.unwrap();
    wait_until("listening", || core.sync_phase(&account) == Some(engine::SyncPhase::Listening));
    wait_until("the IDLE connection", || imap.with(|s| s.idle_starts >= 1));
    // the fake counts an IDLE as started before it waits for a change, and drops a change announced in between: let it settle
    std::thread::sleep(Duration::from_millis(300));
    Synced { core, account, imap, _smtp: smtp, _rt: rt, store_path, _dir: dir }
}

/// A second read-only view of the core's store (its own connection; the WAL lets it read while the core writes).
fn peek(s: &Synced) -> Store { Store::open(&s.store_path).unwrap() }

/// The id of the stored message with this subject, and the folder the store holds it in.
fn id_by_subject(store: &Store, subject: &str) -> Option<(String, String)> {
    let id = store.junk_texts(&all_ids(store)).unwrap().into_iter().find(|m| m.subject == subject)?.message_id;
    let folder = store.message(&id).unwrap()?.folder;
    Some((id, folder))
}
fn all_ids(store: &Store) -> Vec<String> {
    store.write(|ctx| {
        let mut st = ctx.tx.prepare("SELECT id FROM message")?;
        let rows = st.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }).unwrap()
}

// ---------------------------------------------------------------- the decision, without the sync

fn fetched(id: &str, uid: u32, raw: &[u8], flags: &[&str]) -> FetchedMessage {
    // the same header reading the sync uses: the fake hands the raw block to sync-imap's builder through a real fetch in the other
    // tests; here the two fields are filled the way envelope.rs fills them
    let text = String::from_utf8_lossy(raw).to_string();
    let head = text.split("\r\n\r\n").next().unwrap().to_string();
    let m = junk::Message::from_header_block(&head, "", None);
    FetchedMessage {
        uid, uid_validity: 1, modseq: None, flags: flags.iter().map(|f| f.to_string()).collect(), message_id_header: None, size: None, gmail: None,
        // lane reply-headers: the chain field the sync-imap helper adds to FetchedMessage (none here: the junk rules do not read it)
        references: vec![],
        message: Message { id: id.into(), thread_id: id.into(), account_id: "acc".into(), sender_name: m.from_name.clone().unwrap_or_default(), from_address: m.from_email.clone(),
            to: vec![USER.into()], subject: m.subject.clone(), preview: "Verify your account now or it closes today.".into(), received_at_ms: 1, read: false, archived: false,
            folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false },
        authentication_results: m.authentication_results.clone(),
        reply_to: m.reply_to.clone(),
        reply_to_named: vec![],
        html_head: None,
    }
}

#[test]
fn a_scored_arrival_moves_suspects_or_keeps_by_the_ledger_cut_offs() {
    let filter = Filter::new(Model::new(), thresholds());
    assert_eq!((filter.thresholds.move_at, filter.thresholds.review_at), (junk::tuning::JUNK_MOVE_THRESHOLD, junk::tuning::JUNK_REVIEW_THRESHOLD), "the ledger's numbers");
    let (a, b, c) = (fetched("m1", 1, &phish(1), &[]), fetched("m2", 2, &phish_no_reply_to(2), &[]), fetched("m3", 3, &plain(3), &[]));
    let out = decide(&filter, &Context::default(), &[&a, &b, &c], true, false);
    let got: Vec<(&str, Decision, Option<JunkState>)> = out.iter().map(|s| (s.message_id.as_str(), s.verdict.decision, s.state)).collect();
    assert_eq!(got, vec![("m1", Decision::Move, Some(JunkState::Moved)), ("m2", Decision::Suspect, Some(JunkState::Suspect)), ("m3", Decision::Keep, None)], "{out:?}");
    assert!((out[0].verdict.score - (1.0 - 0.7 * 0.7 * 0.5 * 0.45 * 0.75)).abs() < 1e-9, "the header rules as independent chances: {}", out[0].verdict.score);
    // an account with no Junk folder: what would move is only suspected; the hidden check's mutant keeps it
    assert_eq!(decide(&filter, &Context::default(), &[&a], false, false)[0].state, Some(JunkState::Suspect));
    assert_eq!(decide(&filter, &Context::default(), &[&a], true, true)[0].state, None, "junk-never-moves keeps what it would move");
    // read or flagged on another client before it arrived here: never scored
    let (seen, flagged) = (fetched("m4", 4, &phish(4), &["\\Seen"]), fetched("m5", 5, &phish(5), &["\\Flagged"]));
    assert!(decide(&filter, &Context::default(), &[&seen, &flagged], true, false).is_empty());
}

// ---------------------------------------------------------------- through the real sync

fn idle_move(condstore: bool) {
    let s = synced(condstore, |imap| imap.seed("INBOX", 3));
    let bad = deliver(&s.imap, "INBOX", phish(7), &[]);
    let good = deliver(&s.imap, "INBOX", plain(8), &[]);
    wait_until("the phish reaches the server's Junk folder", || s.imap.uids("Junk").len() == 1);
    let inbox = s.imap.uids("INBOX");
    assert!(!inbox.contains(&bad) && inbox.contains(&good), "the phish left INBOX on the server, the plain one stayed: {inbox:?}");
    assert!(s.imap.log().iter().any(|l| l.contains(&format!("UID MOVE {bad} Junk")) || l.contains(&format!("UID MOVE {bad} \"Junk\""))), "a MOVE to the Junk-role folder: {:?}", s.imap.log().iter().filter(|l| l.contains("MOVE")).collect::<Vec<_>>());
    let st = peek(&s);
    wait_until("both arrivals stored", || id_by_subject(&st, "Lunch on Friday 8").is_some() && id_by_subject(&st, "Your account is locked 7").is_some());
    let (pid, pfolder) = id_by_subject(&st, "Your account is locked 7").unwrap();
    assert_eq!(pfolder, "Spam", "the local row left the inbox at once");
    let (score, state) = st.junk_of(&pid).unwrap().unwrap();
    assert_eq!(state, Some(JunkState::Moved));
    assert!(score.unwrap() >= junk::tuning::JUNK_MOVE_THRESHOLD, "{score:?}");
    let (gid, gfolder) = id_by_subject(&st, "Lunch on Friday 8").unwrap();
    assert_eq!(gfolder, "INBOX");
    assert_eq!(st.junk_of(&gid).unwrap().unwrap(), (Some(0.0), None), "the plain arrival keeps its score and no mark");
    wait_until("the ledger row the move made is gone", || st.pending_actions().unwrap().is_empty());
    let _ = &s.core;
}

#[test]
fn an_idle_arrival_that_fails_its_headers_is_moved_to_junk_on_the_server() {
    idle_move(true);
}

#[test]
fn an_idle_arrival_on_a_server_without_condstore_is_moved_too() {
    idle_move(false);
}

#[test]
fn the_headers_reach_the_filter() {
    // the same phish without its Authentication-Results and Reply-To scores only the display name (0.55): kept. With them it moves.
    let s = synced(true, |imap| imap.seed("INBOX", 1));
    let stripped = String::from_utf8(phish(9)).unwrap().lines().filter(|l| !l.starts_with("Authentication-Results") && !l.starts_with(" dkim=") && !l.starts_with("Reply-To")).collect::<Vec<_>>().join("\r\n").into_bytes();
    deliver(&s.imap, "INBOX", stripped, &[]);
    deliver(&s.imap, "INBOX", phish(10), &[]);
    let st = peek(&s);
    wait_until("both stored", || id_by_subject(&st, "Your account is locked 9").is_some() && id_by_subject(&st, "Your account is locked 10").is_some());
    let (with, _) = id_by_subject(&st, "Your account is locked 10").unwrap();
    let (without, _) = id_by_subject(&st, "Your account is locked 9").unwrap();
    let (w_score, w_state) = st.junk_of(&with).unwrap().unwrap();
    let (n_score, n_state) = st.junk_of(&without).unwrap().unwrap();
    assert_eq!(w_state, Some(JunkState::Moved), "the headers fetched from the server decided: {w_score:?}");
    assert!((n_score.unwrap() - junk::tuning::RULE_DISPLAY_NAME_IMPERSONATION).abs() < 1e-9 && n_state.is_none(), "only the name rule: {n_score:?} {n_state:?}");
}

#[test]
fn the_history_and_the_first_screen_are_never_scored() {
    // the phish is old mail: it is on the server before the account is added
    let s = synced(true, |imap| { imap.seed("INBOX", 2); deliver(imap, "INBOX", phish(3), &[]); });
    let st = peek(&s);
    let (id, folder) = id_by_subject(&st, "Your account is locked 3").expect("stored by the first screen");
    assert_eq!(folder, "INBOX");
    assert_eq!(st.junk_of(&id).unwrap().unwrap(), (None, None), "never scored");
    assert!(s.imap.uids("Junk").is_empty() && !s.imap.log().iter().any(|l| l.contains("MOVE")), "nothing was moved on the server");
}

#[test]
fn a_rebuilt_folder_is_never_mass_moved() {
    // a server without CONDSTORE re-reads the newest page on every change; when the folder is rebuilt (UIDVALIDITY changes) every old
    // message comes back under a new id and must not be read as new mail
    let s = synced(false, |imap| { imap.seed("INBOX", 2); deliver(imap, "INBOX", phish(3), &[]); });
    // the rebuilt folder numbers its old messages afresh, above every UID the store held: only the UIDVALIDITY tells them from new mail
    s.imap.with(|st| {
        let b = st.boxes.iter_mut().find(|b| b.name == "INBOX").unwrap();
        b.uid_validity = 2000;
        for (i, m) in b.msgs.iter_mut().enumerate() { m.uid = 10 + i as u32; }
        b.next_uid = 20;
        b.version += 1;
    });
    s.imap.change_flags("INBOX", &[], "\\Seen");
    let st = peek(&s);
    wait_until("the rebuilt folder re-read", || all_ids(&st).iter().any(|i| i.contains(":2000:")));
    std::thread::sleep(Duration::from_millis(300));
    assert!(s.imap.uids("Junk").is_empty() && !s.imap.log().iter().any(|l| l.contains("MOVE")), "nothing old was moved");
    for id in all_ids(&st) { assert_eq!(st.junk_of(&id).unwrap().unwrap().1, None, "{id}"); }
}

#[test]
fn an_arrival_already_read_elsewhere_is_never_scored() {
    let s = synced(true, |imap| imap.seed("INBOX", 1));
    deliver(&s.imap, "INBOX", phish(5), &["\\Seen"]);
    deliver(&s.imap, "INBOX", plain(6), &[]);
    let st = peek(&s);
    wait_until("both stored", || id_by_subject(&st, "Your account is locked 5").is_some() && id_by_subject(&st, "Lunch on Friday 6").is_some());
    let (id, folder) = id_by_subject(&st, "Your account is locked 5").unwrap();
    assert_eq!((folder.as_str(), st.junk_of(&id).unwrap().unwrap()), ("INBOX", (None, None)), "the owner read it on another client: it is theirs");
    assert!(s.imap.uids("Junk").is_empty());
}

#[test]
fn the_first_run_trains_once_from_junk_and_inbox() {
    let s = synced(true, |imap| {
        for n in 0..4 { deliver(imap, "Junk", phish(100 + n), &[]); }
        for n in 0..3 { deliver(imap, "INBOX", plain(200 + n), &[]); }
    });
    let st = peek(&s);
    wait_until("the first run is recorded", || st.junk_bootstrap_report(&s.account).unwrap().is_some());
    assert_eq!(st.junk_bootstrap_report(&s.account).unwrap(), Some((4, 3)), "the Junk folder as junk, the inbox as not junk");
    let model = st.junk_model().unwrap().expect("the learned table is saved");
    assert_eq!((model.trained_junk, model.trained_good), (4, 3));
    let again = engine::junk_filter::filter_for(&st);
    assert_eq!(again.lock().unwrap().model.trained(), (4, 3), "the one filter of this store holds the same table");
}

#[test]
fn mark_spam_learns_once_and_a_changed_mind_relearns() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("whoosh.sqlite")).unwrap();
    let account = store.add_account(USER, "Owner", MailProvider::Imap, "svc\u{1f}acct", false, config(1, 1), 1).unwrap();
    let msg = |key: &str, subject: &str| NewMessage {
        message: Message { id: format!("{account}:{key}"), thread_id: key.into(), account_id: account.clone(), sender_name: "Promo".into(), from_address: "deals@promo.example".into(),
            to: vec![USER.into()], subject: subject.into(), preview: "Cheap pills and a free cruise".into(), received_at_ms: 1, read: false, archived: false, folder: "INBOX".into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false },
        remote_id: Some("1".into()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(10), tab_id: None,
    };
    store.insert_messages(vec![msg("k1", "Win a cruise today")]).unwrap();
    let id = format!("{account}:k1");
    assert_eq!(learn(&store, &[id.clone()], JunkLesson::Junk).unwrap(), 1);
    assert_eq!(learn(&store, &[id.clone()], JunkLesson::Junk).unwrap(), 0, "a repeated MarkSpam never trains twice");
    let trained = |s: &Store| { let m = s.junk_model().unwrap().unwrap(); (m.trained_junk, m.trained_good) };
    assert_eq!(trained(&store), (1, 0));
    assert_eq!(store.junk_lesson(&id).unwrap(), Some(JunkLesson::Junk), "the lesson is written");
    assert_eq!(learn(&store, &[id.clone()], JunkLesson::NotJunk).unwrap(), 1);
    assert_eq!(trained(&store), (0, 1), "a changed mind takes the old lesson out and puts the new one in");
    assert_eq!(filter_for(&store).lock().unwrap().model.trained(), (0, 1), "the held filter and the saved table agree");
    assert_eq!(learn(&store, &["nobody".into()], JunkLesson::Junk).unwrap(), 0, "an unknown id teaches nothing");
}

// ---------------------------------------------------------------- slice 6's mail, in process (checks/realcore_mailfake.py, the same text)

const JUNK_WORDS: [&str; 8] = ["verify your account now", "your account is locked", "confirm your password today", "urgent security alert",
    "claim your refund", "unusual sign-in activity", "update your payment details", "final notice before closure"];
const GOOD_WORDS: [&str; 8] = ["lunch on friday", "notes from the planning meeting", "the slides for tuesday", "photos from the weekend",
    "the draft for review", "agenda for the team call", "dinner at our place", "the invoice you asked about"];
fn cap(w: &str) -> String { let mut c = w.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }
fn corpus_junk(n: usize) -> Vec<u8> {
    let w = JUNK_WORDS[n % JUNK_WORDS.len()];
    format!("From: Security Team <alert{n}@secure-login.example>\r\nTo: {USER}\r\nSubject: {} {n}\r\nMessage-ID: <junk-{n}@secure-login.example>\r\nDate: Sun, 08 Nov 2026 09:{:02}:00 +0000\r\n\r\n{}: click the link to verify your account and confirm your password now.\r\n", cap(w), n % 60, cap(w)).into_bytes()
}
fn corpus_good(n: usize) -> Vec<u8> {
    let w = GOOD_WORDS[n % GOOD_WORDS.len()];
    format!("From: Ann Lee <ann{n}@friends.example>\r\nTo: {USER}\r\nSubject: {} {n}\r\nMessage-ID: <good-{n}@friends.example>\r\nDate: Sun, 08 Nov 2026 08:{:02}:00 +0000\r\n\r\nHello, about {w}: shall we meet at noon with the team?\r\n", cap(w), n % 60).into_bytes()
}
/// Slice 6's phish: no Reply-To, so its header rules alone give 0.890, between the cut-offs.
fn slice6_phish() -> Vec<u8> {
    format!("Authentication-Results: mx.whoosh-check.invalid; spf=fail smtp.mailfrom=secure-login.example;\r\n dkim=fail header.d=secure-login.example; dmarc=fail header.from=secure-login.example\r\n\
             Authentication-Results: forged.example; spf=pass; dkim=pass; dmarc=pass\r\nFrom: \"service@bank.example\" <alerts@secure-login.example>\r\nTo: {USER}\r\n\
             Subject: Your account is locked: verify your account now\r\nMessage-ID: <phish-1@secure-login.example>\r\nDate: Mon, 09 Nov 2026 11:00:00 +0000\r\n\r\n\
             Urgent security alert: verify your account now and confirm your password today.\r\n").into_bytes()
}
fn slice6_plain() -> Vec<u8> {
    format!("From: Ann Lee <ann@friends.example>\r\nTo: {USER}\r\nSubject: Lunch on Friday with the team\r\nMessage-ID: <plain-1@friends.example>\r\nDate: Mon, 09 Nov 2026 11:01:00 +0000\r\n\r\nHello, shall we meet at noon on friday with the team?\r\n").into_bytes()
}

#[test]
fn the_first_run_corpus_moves_a_phish_its_headers_alone_only_suspect() {
    // as slice 6's local server: no CONDSTORE (every change re-reads the newest page), 24 ordinary messages in INBOX, 24 junk in Junk
    let s = synced(false, |imap| for n in 1..=24 { deliver(imap, "INBOX", corpus_good(n), &[]); deliver(imap, "Junk", corpus_junk(n), &[]); });
    let st = peek(&s);
    wait_until("the first run is recorded", || st.junk_bootstrap_report(&s.account).unwrap().is_some());
    assert_eq!(st.junk_bootstrap_report(&s.account).unwrap(), Some((24, 24)));
    let bad = deliver(&s.imap, "INBOX", slice6_phish(), &[]);
    let good = deliver(&s.imap, "INBOX", slice6_plain(), &[]);
    wait_until("the phish reaches the server's Junk folder", || s.imap.uids("Junk").len() == 25);
    let inbox = s.imap.uids("INBOX");
    assert!(!inbox.contains(&bad) && inbox.contains(&good), "{inbox:?}");
    wait_until("both stored", || id_by_subject(&st, "Lunch on Friday with the team").is_some());
    let (pid, _) = id_by_subject(&st, "Your account is locked: verify your account now").unwrap();
    let (gid, gfolder) = id_by_subject(&st, "Lunch on Friday with the team").unwrap();
    let (p_score, p_state) = st.junk_of(&pid).unwrap().unwrap();
    let (g_score, g_state) = st.junk_of(&gid).unwrap().unwrap();
    assert_eq!(p_state, Some(JunkState::Moved), "{p_score:?}");
    assert!(p_score.unwrap() > 1.0 - 0.7 * 0.7 * 0.5 * 0.45, "the learned words pushed it past what its headers alone give: {p_score:?}");
    assert_eq!((gfolder.as_str(), g_state), ("INBOX", None), "the plain one stays: {g_score:?}");
    assert!(g_score.unwrap() < junk::tuning::JUNK_REVIEW_THRESHOLD, "{g_score:?}");
}
