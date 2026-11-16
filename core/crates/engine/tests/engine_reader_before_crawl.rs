//! fix-realcore round 2 (lead, main95): with IMAP_CONN_PER_ACCOUNT 2 the IDLE session holds one and the sync, the reader's body fetch, the crawl and
//! the acts share the other, so the body of the mail the owner opens must go BEFORE the crawl's next batch: after at most the batch already in
//! flight. A crawl of 60 bodies (INBOX, 6 batches) runs against a body source that logs each call with its start and end; while the first batch
//! is in flight the reader asks for the oldest mail (UID 1, the crawl's last batch). The log must show the reader's call as the next call after
//! the ones already started, and no background call starting while the reader's call runs (the one session would have had to serve it first).
//! Broken copies (one edit each, red first):
//!   crawl.rs `crawl_pass`: delete the line `self.after_readers().await;      // fix-realcore round 2: the mail the owner opened goes before the next batch`
//!       (the old code: the crawl sends its next batches while the reader's fetch runs: red on the overlap assertion)
//!   bodies.rs `request`: `self.readers.fetch_add(1, Ordering::SeqCst);` -> `self.readers.fetch_add(0, Ordering::SeqCst);` (no reader is ever seen: red)
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use contracts::*;
use engine::bodies::BodySource;
use engine::RustCore;
use store::NewMessage;

const USER: &str = "owner@example.test";
const WAIT: Duration = Duration::from_secs(60);
const MAILS: u32 = 60;
const READER_UID: u32 = 1;
/// A crawl batch's answer time and the reader's (longer, so a background call sent meanwhile is seen inside it).
const BATCH_MS: u64 = 80;
const READER_MS: u64 = 400;

/// One call of the source: the UIDs asked, when it started and when it answered.
#[derive(Clone, Debug)]
struct Call { uids: Vec<u32>, start: Instant, end: Option<Instant> }

struct LoggingSource { calls: Mutex<Vec<Call>> }
#[async_trait]
impl BodySource for LoggingSource {
    async fn fetch_bodies(&self, _folder: &str, uids: &[u32]) -> Result<Vec<(u32, Vec<u8>)>, CoreError> {
        let n = { let mut c = self.calls.lock().unwrap(); c.push(Call { uids: uids.to_vec(), start: Instant::now(), end: None }); c.len() - 1 };
        let reader = uids == [READER_UID];
        tokio::time::sleep(Duration::from_millis(if reader { READER_MS } else { BATCH_MS })).await;
        self.calls.lock().unwrap()[n].end = Some(Instant::now());
        Ok(uids.iter().map(|u| (*u, format!("From: s{u} <s{u}@example.test>\r\nTo: {USER}\r\nSubject: INBOX {u}\r\nMessage-ID: <r{u}@example.test>\r\n\r\nBody {u}.\r\n").into_bytes())).collect())
    }
}

fn wait_until(what: &str, mut ok: impl FnMut() -> bool) {
    let t0 = Instant::now();
    while t0.elapsed() < WAIT {
        if ok() { return; }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("never reached within {WAIT:?}: {what}");
}

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn row(account: &str, uid: u32) -> NewMessage {
    NewMessage {
        message: Message {
            id: format!("{account}:INBOX:1:{uid}"), thread_id: format!("{account}:<r{uid}@thread>"), account_id: account.into(), sender_name: format!("s{uid}"),
            from_address: format!("s{uid}@example.test"), to: vec![USER.into()], subject: format!("INBOX {uid}"), preview: format!("preview {uid}"),
            received_at_ms: 1_700_000_000_000 + i64::from(uid) * 1000, read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![],
            login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(uid.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(4000), tab_id: None,
    }
}

#[test]
fn the_body_the_owner_opens_goes_before_the_crawls_next_batch() {
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages((1..=MAILS).map(|u| row(&account, u)).collect()).unwrap();
    let source = Arc::new(LoggingSource { calls: Mutex::new(vec![]) });
    core.set_body_source(&account, source.clone());
    core.background().crawl_kick(&account);
    wait_until("the crawl's first batch is in flight", || !source.calls.lock().unwrap().is_empty());
    let started_before_ask = source.calls.lock().unwrap().len();
    let reader_id = format!("{account}:INBOX:1:{READER_UID}");
    let _ = core.body(reader_id.clone());
    wait_until("the opened mail's body is stored", || store.body_parts(&reader_id).unwrap().is_some_and(|p| p.body_text.is_some()));
    assert!(core.background().settle(WAIT), "the crawl ended");
    let calls = source.calls.lock().unwrap().clone();
    let at = calls.iter().position(|c| c.uids == [READER_UID]).expect("the reader's call reached the source");
    assert_eq!(at, started_before_ask, "the reader's call is the next one after the {started_before_ask} already in flight: {:?}", calls.iter().map(|c| c.uids.clone()).collect::<Vec<_>>());
    let reader = &calls[at];
    let reader_end = reader.end.expect("the reader's call answered");
    let during: Vec<Vec<u32>> = calls.iter().filter(|c| c.uids != [READER_UID] && c.start >= reader.start && c.start < reader_end).map(|c| c.uids.clone()).collect();
    assert!(during.is_empty(), "no crawl batch was sent while the reader's fetch was waiting or running: {during:?}");
    let crawled: usize = calls.iter().filter(|c| c.uids != [READER_UID]).map(|c| c.uids.len()).sum();
    assert_eq!(crawled, (MAILS - 1) as usize, "the crawl still stored every other body, the reader's mail not asked twice");
    println!("PASS reader-before-crawl: the reader's call was call {} of {}, after the {} in flight; no batch during its {} ms", at + 1, calls.len(), started_before_ask, READER_MS);
}
