//! junk-r3: the spam filter as the owner meets it, through the real sync over the in-process IMAP fake (127.0.0.1 only). Every test ends
//! with `nothing_left_this_mac`: the process is closed to this Mac (netguard, as a rig core closes it), a resolver stub counts every name
//! lookup that is not a loopback address, and the guard's own refusal counter must still read 0.
//!
//! MUTANTS (one edit each, run by body-r1-out/mutants.sh; each must turn the test named red; the edit and the red line are in its log):
//!   E1 junk_filter.rs `decide`: `Decision::Move | Decision::Suspect => Some(Suspect)` -> `Decision::Move | Decision::Suspect => Some(Moved)`
//!        (the review band moves too) -> the_owners_threshold_says_when_the_filter_moves
//!   E2 junk_filter.rs `spare_correspondents`: body emptied -> a_sender_the_owner_wrote_to_is_never_moved_by_score_alone
//!   E3 junk_filter.rs `context`: `hard_rules` forced to `vec![]` -> a_blocked_sender_is_always_junk
//!   E4 junk_filter.rs `learn`: the model call (`f.learn(&msg, label(lesson))`) removed -> three_junk_marks_move_the_fourth_alike
//!   E5 rust_core.rs MarkNotSpam arm: `JunkLesson::NotJunk` -> `JunkLesson::Junk` -> not_junk_trains_and_a_look_alike_stays
//!   E6 junk_filter.rs `store_arrivals`: a lookup of a name outside this Mac added -> every test (the guard counts it)
//!   E7 junk_filter.rs `from_fetched`: `html: f.html_head.clone()` -> `html: None` -> the_link_rule_reads_the_html_the_sync_fetched
#![allow(dead_code)]
#[path = "../../autoconfig/tests/support/mod.rs"]
mod detect_fakes;
#[allow(dead_code)]
#[path = "../../sync-imap/tests/support/mod.rs"]
mod imap_fakes;

use std::sync::Arc;
use std::time::{Duration, Instant};

use contracts::*;
use detect_fakes::{FakeDns, FakeHttp, FakeProbe};
use engine::{RustCore, TestTransport};
use imap_fakes::{smtp::FakeSmtp, FakeImap};
use secrets::{MemoryKeychain, Namespace};
use store::{JunkLesson, JunkState, NewMessage, Store};

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


use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use contracts::BlockTarget;

const MOVE: f64 = junk::tuning::JUNK_MOVE_THRESHOLD;
const REVIEW: f64 = junk::tuning::JUNK_REVIEW_THRESHOLD;

// ---------------------------------------------------------------- nothing leaves this Mac

static NAMES_ASKED: AtomicUsize = AtomicUsize::new(0);

/// The guard's resolver stub: an address literal or `localhost` resolves as itself; any other name is counted and refused.
struct CountingResolver;
impl netguard::Resolve for CountingResolver {
    fn lookup(&self, host: &str, port: u16) -> std::io::Result<Vec<std::net::SocketAddr>> {
        if netguard::is_this_mac(host) {
            use std::net::ToSocketAddrs;
            return Ok((host, port).to_socket_addrs()?.collect());
        }
        NAMES_ASKED.fetch_add(1, Ordering::SeqCst);
        Err(std::io::Error::new(std::io::ErrorKind::Other, "the test refuses every name outside this Mac"))
    }
}

fn close_to_this_mac() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| { netguard::set_resolver(std::sync::Arc::new(CountingResolver)); netguard::close_to_this_mac(); });
}

fn nothing_left_this_mac() {
    assert!(netguard::is_closed(), "the process was closed to this Mac");
    assert_eq!(NAMES_ASKED.load(Ordering::SeqCst), 0, "no name outside this Mac was looked up");
    assert_eq!((netguard::refused_count(), netguard::last_refused()), (0, None), "the guard refused nothing: nothing tried to leave");
}

// ---------------------------------------------------------------- the fixtures

fn raw(from: &str, subject: &str, text: &str, mid: &str, extra: &str) -> Vec<u8> {
    format!("{extra}From: {from}\r\nTo: {USER}\r\nSubject: {subject}\r\nMessage-ID: <{mid}@mail.example>\r\nDate: Mon, 09 Nov 2026 12:00:00 +0000\r\n\r\n{text}\r\n").into_bytes()
}
/// A message of the invented vocabulary no support corpus holds: the model has no opinion of it until the owner teaches one.
fn zorbix(n: u32) -> Vec<u8> {
    raw("Zorbix <promo@zorbix.example>", &format!("Zorbix gizmo {n}"), "Zorbix gizmo quartz voucher plinth", &format!("z{n}"), "")
}
fn subject_of_zorbix(n: u32) -> String { format!("Zorbix gizmo {n}") }
fn clean_from(address: &str, n: u32) -> Vec<u8> {
    raw(&format!("<{address}>"), &format!("Quarterly figures {n}"), "The figures are attached for your review.", &format!("c{n}"), "")
}

const JUNK_WORDS: [&str; 8] = ["verify your account now", "your account is locked", "confirm your password today", "urgent security alert",
    "claim your refund", "unusual sign-in activity", "update your payment details", "final notice before closure"];
const GOOD_WORDS: [&str; 8] = ["lunch on friday", "notes from the planning meeting", "the slides for tuesday", "photos from the weekend",
    "the draft for review", "agenda for the team call", "dinner at our place", "the invoice you asked about"];
fn cap(w: &str) -> String { let mut c = w.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }
fn corpus_junk(n: usize) -> Vec<u8> {
    let w = JUNK_WORDS[n % JUNK_WORDS.len()];
    raw(&format!("Security Team <alert{n}@secure-login.example>"), &format!("{} {n}", cap(w)), &format!("{}: click the link to verify your account and confirm your password now.", cap(w)), &format!("junk-{n}"), "")
}
fn corpus_good(n: usize) -> Vec<u8> {
    let w = GOOD_WORDS[n % GOOD_WORDS.len()];
    raw(&format!("Ann Lee <ann{n}@friends.example>"), &format!("{} {n}", cap(w)), &format!("Hello, about {w}: shall we meet at noon with the team?"), &format!("good-{n}"), "")
}

/// An account whose first run learned 24 junk (its Junk folder) and 24 not-junk (its inbox): the model is ready, and knows nothing of Zorbix.
fn trained() -> Synced {
    close_to_this_mac();
    let s = synced(true, |imap| for n in 1..=24 { deliver(imap, "INBOX", corpus_good(n), &[]); deliver(imap, "Junk", corpus_junk(n), &[]); });
    let st = peek(&s);
    wait_until("the first run is recorded", || st.junk_bootstrap_report(&s.account).unwrap().is_some());
    assert_eq!(st.junk_bootstrap_report(&s.account).unwrap(), Some((24, 24)));
    s
}

/// What the store holds of one arrival once it is scored: (id, folder, score, state).
fn scored(st: &Store, subject: &str) -> (String, String, f64, Option<JunkState>) {
    let mut out = None;
    wait_until(&format!("{subject:?} stored and scored"), || {
        if let Some((id, folder)) = id_by_subject(st, subject) {
            if let Some((Some(score), state)) = st.junk_of(&id).unwrap() { out = Some((id, folder, score, state)); return true; }
        }
        false
    });
    out.unwrap()
}

/// True once the server no longer holds this INBOX message and a MOVE was asked (a batch is one command: "UID MOVE 2,3 Junk").
fn moved_on_server(s: &Synced, uid: u32) -> bool {
    !s.imap.uids("INBOX").contains(&uid) && s.imap.log().iter().any(|l| l.contains("UID MOVE") && l.contains("Junk"))
}
fn quiet(s: &Synced) { let st = peek(s); wait_until("the write-back ledger is empty", || st.pending_actions().unwrap().is_empty()); }
fn why(core: &RustCore, id: &str) -> serde_json::Value { serde_json::from_str(&core.state(format!("message/{id}/junk")).expect("message/<id>/junk is served")).unwrap() }

// ---------------------------------------------------------------- (2) training

#[test]
fn three_junk_marks_move_the_fourth_alike() {
    let s = trained();
    let st = peek(&s);
    for n in 1..=3 { deliver(&s.imap, "INBOX", zorbix(n), &[]); }
    let firsts: Vec<_> = (1..=3).map(|n| scored(&st, &subject_of_zorbix(n))).collect();
    for (_, folder, score, state) in &firsts {
        assert_eq!((folder.as_str(), *state), ("INBOX", None), "before the owner teaches anything the new vocabulary stays: {score}");
        assert!(*score < REVIEW, "no opinion of Zorbix yet: {score}");
    }
    let ids: Vec<String> = firsts.iter().map(|f| f.0.clone()).collect();
    s.core.run(Command::MarkSpam { message_ids: ids.clone() }).expect("the owner marks three messages junk");
    for id in &ids { assert_eq!(st.junk_lesson(id).unwrap(), Some(JunkLesson::Junk), "the act is a lesson"); }
    let (j, g) = { let m = st.junk_model().unwrap().unwrap(); (m.trained_junk, m.trained_good) };
    assert_eq!((j, g), (27, 24), "24 from the first run plus the owner's three");
    quiet(&s);
    let uid = deliver(&s.imap, "INBOX", zorbix(4), &[]);
    let (id, folder, score, state) = scored(&st, &subject_of_zorbix(4));
    assert!(score >= MOVE, "a fourth alike scores above the move threshold: {score}");
    assert_eq!((folder.as_str(), state), ("Spam", Some(JunkState::Moved)));
    wait_until("the server moves it to Junk", || moved_on_server(&s, uid));
    let w = why(&s.core, &id);
    assert_eq!((w["folder"].as_str(), w["state"].as_str(), w["blocked"].as_bool(), w["correspondent"].as_bool()), (Some("Spam"), Some("moved"), Some(false), Some(false)), "{w}");
    assert!(w["score"].as_f64().unwrap() >= MOVE, "{w}");
    nothing_left_this_mac();
}

#[test]
fn not_junk_trains_and_a_look_alike_stays() {
    let s = trained();
    let st = peek(&s);
    deliver(&s.imap, "INBOX", zorbix(1), &[]);
    let (first, ..) = scored(&st, &subject_of_zorbix(1));
    s.core.run(Command::MarkSpam { message_ids: vec![first.clone()] }).expect("the key !");
    quiet(&s);
    // the lesson alone is enough: a look-alike now moves
    let uid2 = deliver(&s.imap, "INBOX", zorbix(2), &[]);
    let (second, folder2, score2, state2) = scored(&st, &subject_of_zorbix(2));
    assert!((folder2.as_str(), state2) == ("Spam", Some(JunkState::Moved)) && score2 >= MOVE, "{folder2} {score2} {state2:?}");
    wait_until("moved on the server", || moved_on_server(&s, uid2));
    quiet(&s);
    // the owner changes their mind about the first one: "Not spam" in the Junk view
    s.core.run(Command::MarkNotSpam { message_ids: vec![first.clone()] }).expect("Not spam");
    assert_eq!(st.junk_lesson(&first).unwrap(), Some(JunkLesson::NotJunk), "the act is a lesson");
    assert_eq!(st.message(&first).unwrap().unwrap().folder, "INBOX", "it is back in the inbox");
    let w = why(&s.core, &first);
    assert_eq!((w["folder"].as_str(), w["lesson"].as_str(), w["state"].as_str()), (Some("INBOX"), Some("not_junk"), None), "{w}");
    quiet(&s);
    // a look-alike now stays
    deliver(&s.imap, "INBOX", zorbix(3), &[]);
    let (third, folder3, score3, state3) = scored(&st, &subject_of_zorbix(3));
    assert_eq!((folder3.as_str(), state3), ("INBOX", None), "the look-alike stays in the inbox: {score3}");
    assert!(score3 < REVIEW, "{score3}");
    assert_eq!(why(&s.core, &third)["state"], serde_json::Value::Null);
    assert_eq!(st.message(&second).unwrap().unwrap().folder, "Spam", "the one already moved stays where it was moved");
    nothing_left_this_mac();
}

// ---------------------------------------------------------------- (3) safety

#[test]
fn a_blocked_sender_is_always_junk() {
    close_to_this_mac();
    let s = synced(true, |imap| imap.seed("INBOX", 1));      // no first-run corpus: the word model has no opinion at all
    let st = peek(&s);
    st.block_add(BlockTarget::Sender { address: "pest@blocked.example".into() }, 1).unwrap();
    st.block_add(BlockTarget::Domain { domain: "spamdom.example".into() }, 1).unwrap();
    let uids = [deliver(&s.imap, "INBOX", clean_from("pest@blocked.example", 1), &[]),
                deliver(&s.imap, "INBOX", clean_from("anyone@news.spamdom.example", 2), &[]),
                deliver(&s.imap, "INBOX", clean_from("ann@friends.example", 3), &[])];
    for n in 1..=2 {
        let (id, folder, score, state) = scored(&st, &format!("Quarterly figures {n}"));
        assert_eq!((folder.as_str(), state, score), ("Spam", Some(JunkState::Moved), 1.0), "a block is certain, whatever the words");
        assert_eq!(why(&s.core, &id)["blocked"], true);
    }
    let (id, folder, score, state) = scored(&st, "Quarterly figures 3");
    assert_eq!((folder.as_str(), state, score), ("INBOX", None, 0.0), "an unblocked sender with the same words stays");
    assert_eq!(why(&s.core, &id)["blocked"], false);
    wait_until("the two blocked ones are moved on the server", || moved_on_server(&s, uids[0]) && moved_on_server(&s, uids[1]));
    assert!(!moved_on_server(&s, uids[2]));
    // a blocked sender the owner has written to is still blocked: the guard spares the score, never the owner's own block
    st.block_add(BlockTarget::Sender { address: "old@friend.example".into() }, 2).unwrap();
    sent_to(&st, &s.account, "old@friend.example", "sent-old");
    deliver(&s.imap, "INBOX", clean_from("old@friend.example", 4), &[]);
    let (_, folder, score, state) = scored(&st, "Quarterly figures 4");
    assert_eq!((folder.as_str(), state, score), ("Spam", Some(JunkState::Moved), 1.0));
    nothing_left_this_mac();
}

/// The owner's mail to `to`, as the sync stores a copy of the server's Sent folder.
fn sent_to(st: &Store, account: &str, to: &str, key: &str) {
    st.insert_messages(vec![NewMessage {
        message: Message { id: format!("{account}:{key}"), thread_id: key.into(), account_id: account.into(), sender_name: "Owner".into(), from_address: USER.into(),
            to: vec![to.into()], subject: format!("Hello {key}"), preview: "Hello".into(), received_at_ms: 1, read: true, archived: false, folder: "Sent".into(),
            attachment_names: vec![], login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: true },
        remote_id: None, body_text: None, body_html: None, has_list_header: false, size_bytes: Some(10), tab_id: None,
    }]).unwrap();
}

#[test]
fn a_sender_the_owner_wrote_to_is_never_moved_by_score_alone() {
    close_to_this_mac();
    let s = synced(true, |imap| imap.seed("INBOX", 1));
    let st = peek(&s);
    // the owner has written to alerts11@secure-login.example (a reply to a real correspondent whose mail fails its headers: a forwarder)
    sent_to(&st, &s.account, "alerts11@secure-login.example", "sent-1");
    let kept = deliver(&s.imap, "INBOX", phish(11), &[]);
    let stranger = deliver(&s.imap, "INBOX", phish(12), &[]);
    let (kid, kfolder, kscore, kstate) = scored(&st, "Your account is locked 11");
    assert!(kscore >= MOVE, "its score alone says move: {kscore}");
    assert_eq!((kfolder.as_str(), kstate), ("INBOX", Some(JunkState::Suspect)), "but a correspondent's mail only gets the suspect mark");
    let w = why(&s.core, &kid);
    assert_eq!((w["correspondent"].as_bool(), w["state"].as_str(), w["folder"].as_str()), (Some(true), Some("suspect"), Some("INBOX")), "{w}");
    let (sid, sfolder, _, sstate) = scored(&st, "Your account is locked 12");
    assert_eq!((sfolder.as_str(), sstate), ("Spam", Some(JunkState::Moved)), "the same mail from a stranger moves");
    assert_eq!(why(&s.core, &sid)["correspondent"], false);
    wait_until("the stranger's mail is moved", || moved_on_server(&s, stranger));
    assert!(!moved_on_server(&s, kept), "the correspondent's mail was never sent to Junk on the server");
    assert!(s.imap.uids("INBOX").contains(&kept));
    nothing_left_this_mac();
}

// ---------------------------------------------------------------- (1) the threshold and the signals

#[test]
fn the_owners_threshold_says_when_the_filter_moves() {
    close_to_this_mac();
    let s = synced(true, |imap| imap.seed("INBOX", 1));
    let st = peek(&s);
    // a cut-off the person sets above the phish's score (0.917): it is suspected, not moved
    engine::junk_filter::set_thresholds(&st, 0.95, 0.70).expect("a valid pair");
    let held = deliver(&s.imap, "INBOX", phish(21), &[]);
    let (_, folder, score, state) = scored(&st, "Your account is locked 21");
    assert!(score > MOVE && score < 0.95, "{score}");
    assert_eq!((folder.as_str(), state), ("INBOX", Some(JunkState::Suspect)), "below the person's cut-off: kept and marked");
    assert!(engine::junk_filter::set_thresholds(&st, 0.5, 0.7).is_err(), "a review cut-off above the move cut-off is refused");
    // the person lowers it: the same kind of mail moves
    engine::junk_filter::set_thresholds(&st, 0.90, 0.70).unwrap();
    let moved = deliver(&s.imap, "INBOX", phish(22), &[]);
    let (_, folder, _, state) = scored(&st, "Your account is locked 22");
    assert_eq!((folder.as_str(), state), ("Spam", Some(JunkState::Moved)));
    wait_until("moved on the server", || moved_on_server(&s, moved));
    assert!(!moved_on_server(&s, held));
    nothing_left_this_mac();
}

#[test]
fn the_link_rule_reads_the_html_the_sync_fetched() {
    close_to_this_mac();
    let s = synced(true, |imap| imap.seed("INBOX", 1));
    let st = peek(&s);
    let html = raw("Bank <service@bank.example>", "Your statement", "<html><body><p>Open <a href=\"http://collect.payouts.example/login\">www.bank.example</a> now</p></body></html>", "link-1",
        "MIME-Version: 1.0\r\nContent-Type: text/html; charset=utf-8\r\n");
    deliver(&s.imap, "INBOX", html, &[]);
    let (_, folder, score, state) = scored(&st, "Your statement");
    assert!((score - junk::tuning::RULE_LINK_MISMATCH).abs() < 1e-9, "the link rule alone: {score}");
    assert_eq!((folder.as_str(), state), ("INBOX", None), "a link mismatch alone moves nothing");
    nothing_left_this_mac();
}

// ---------------------------------------------------------------- (2) the first run's cap

#[test]
fn the_first_run_learns_at_most_the_ledgers_cap_from_the_servers_junk_folder() {
    close_to_this_mac();
    let cap = junk::tuning::BOOTSTRAP_MAX_PER_CLASS;
    // the server's Junk folder holds more than the cap (the person's own years of junk), the inbox a few
    let s = synced(true, |imap| {
        imap.with(|st| {
            let b = st.boxes.iter_mut().find(|b| b.name == "Junk").unwrap();
            for n in 0..(cap + 3) as u32 {
                b.highest_modseq += 1;
                let uid = b.next_uid;
                b.next_uid += 1;
                b.msgs.push(imap_fakes::Msg { uid, flags: Default::default(), modseq: b.highest_modseq, raw: corpus_junk(n as usize), gm: None });
            }
            b.version += 1;
        });
        for n in 1..=5 { deliver(imap, "INBOX", corpus_good(n), &[]); }
    });
    let st = peek(&s);
    let mut report = None;
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(120) && report.is_none() { report = st.junk_bootstrap_report(&s.account).unwrap(); std::thread::sleep(Duration::from_millis(100)); }
    let (junk_learned, good_learned) = report.expect("the first run is recorded");
    assert_eq!((junk_learned as usize, good_learned as usize), (cap, 5), "the Junk folder is read up to the ledger's cap, the inbox in full");
    nothing_left_this_mac();
}
