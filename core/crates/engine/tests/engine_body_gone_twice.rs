//! fix-realcore round 12 (main111 `--slice-bodygone`: "UID SEARCH UID 2 lines: 2; whole-body asks of it: 5"; "whole-body asks after the check: 2"):
//! the prefetch and the crawl each read their list of bodies to fetch, then took turns; the first found the mail gone and removed its row, and the
//! second, from its older list, asked the server for the same UID again. Each now checks, in its turn and before it asks, that the row still exists.
//! engine_body_gone.rs did not see it: it ran one download at a time. Here both run together over a source that answers slowly and says the UID is gone.
//! Broken copy (one edit, red first):
//!   crawl.rs `drop_stored`: `Ok(None) => true,` -> `Ok(None) => false,` (the second runner asks again: red)
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use contracts::*;
use engine::bodies::BodySource;
use engine::RustCore;
use store::NewMessage;

const USER: &str = "owner@example.test";
const WAIT: Duration = Duration::from_secs(40);
const GONE: u32 = 2;

/// A server that no longer holds UID 2: no body, and the existence check says gone. It answers after a pause, so both runners have read their lists.
struct GoneSource { calls: AtomicUsize, asked: Mutex<Vec<Vec<u32>>> }
#[async_trait]
impl BodySource for GoneSource {
    async fn fetch_bodies(&self, _folder: &str, uids: &[u32]) -> Result<Vec<(u32, Vec<u8>)>, CoreError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.asked.lock().unwrap().push(uids.to_vec());
        tokio::time::sleep(Duration::from_millis(400)).await;
        Ok(Vec::new())
    }
    fn gone(&self, _folder: &str, uid: u32) -> bool { uid == GONE }
}

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn row(account: &str, id: &str, uid: u32) -> NewMessage {
    NewMessage {
        message: Message {
            id: id.into(), thread_id: format!("{account}:<twice{uid}@thread>"), account_id: account.into(), sender_name: "Sender".into(),
            from_address: "s@example.test".into(), to: vec![USER.into()], subject: format!("Gone {uid}"), preview: "preview".into(),
            received_at_ms: 1_700_000_000_000 + i64::from(uid) * 1000, read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![],
            login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(uid.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(4000), tab_id: None,
    }
}

#[test]
fn the_prefetch_and_the_crawl_together_ask_a_gone_uid_once() {
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens");
    let store = core.store();
    let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
    let id = format!("{account}:INBOX:1:{GONE}");
    store.insert_messages(vec![row(&account, &id, GONE)]).unwrap();
    let source = Arc::new(GoneSource { calls: AtomicUsize::new(0), asked: Mutex::new(vec![]) });
    core.set_body_source(&account, source.clone());
    // the sync's pass: the prefetch and the crawl start together, each from its own list
    core.background().folder_synced(&account, "INBOX");
    core.background().kick(&account);
    core.background().crawl_kick(&account);
    assert!(core.background().settle(WAIT), "the background ended");
    let asked: Vec<Vec<u32>> = source.asked.lock().unwrap().clone();
    let times = asked.iter().filter(|u| u.contains(&GONE)).count();
    assert!(store.message(&id).unwrap().is_none(), "the gone mail's row is removed");
    assert_eq!(times, 1, "the gone UID is asked once, not by both runners: {asked:?}");
    println!("PASS gone-asked-once: prefetch and crawl together asked UID {GONE} {times} time");
}
