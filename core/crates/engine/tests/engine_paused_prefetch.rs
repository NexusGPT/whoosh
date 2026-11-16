//! fix-realcore round 13 (main112 `--slice-crawl --paused`: "9 whole-message fetches in 20s, 6 on a folder other than INBOX"; 85 bodies stored = the
//! newest 50 of each folder, the crawl's own 15 left): with the crawl held, the PREFETCH still fetched the newest bodies of Archive and Sent. While the
//! background is held by battery, heat or a hold, only INBOX's newest bodies are fetched (what the list shows first); another folder waits for the
//! crawl after the pause. `background/pause` names the engine's own pause.
//! Broken copy (one edit, red first):
//!   bodies.rs `prefetch_run`: `if !folder.eq_ignore_ascii_case("INBOX") && self.held_now() { break; }` -> deleted (Archive is fetched while held: red)
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use contracts::*;
use engine::bodies::BodySource;
use engine::RustCore;
use store::NewMessage;

const USER: &str = "owner@example.test";

/// A server that answers every body and records which folder each ask was for.
struct Recording { asked: Mutex<Vec<(String, Vec<u32>)>> }
#[async_trait]
impl BodySource for Recording {
    async fn fetch_bodies(&self, folder: &str, uids: &[u32]) -> Result<Vec<(u32, Vec<u8>)>, CoreError> {
        self.asked.lock().unwrap().push((folder.to_string(), uids.to_vec()));
        Ok(uids.iter().map(|u| (*u, format!("From: s <s@example.test>\r\nTo: {USER}\r\nSubject: {folder} {u}\r\nMessage-ID: <p{folder}{u}@example.test>\r\n\r\nBody {u}.\r\n").into_bytes())).collect())
    }
}

fn config() -> AccountConfig {
    AccountConfig {
        imap_host: "127.0.0.1".into(), imap_port: 1, imap_security: ConnectionSecurity::Ssl, smtp_host: "127.0.0.1".into(), smtp_port: 1,
        smtp_security: ConnectionSecurity::Starttls, username: USER.into(), smtp_username: USER.into(), auth_method: AuthMethod::Password, source: DetectionSource::Manual,
    }
}

fn row(account: &str, folder: &str, uid: u32) -> NewMessage {
    NewMessage {
        message: Message {
            id: format!("{account}:{folder}:1:{uid}"), thread_id: format!("{account}:<p{folder}{uid}@thread>"), account_id: account.into(), sender_name: "Sender".into(),
            from_address: "s@example.test".into(), to: vec![USER.into()], subject: format!("{folder} {uid}"), preview: "preview".into(),
            received_at_ms: 1_700_000_000_000 + i64::from(uid) * 1000, read: false, archived: false, folder: folder.into(), attachment_names: vec![],
            login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false, reply_to_name: None, cc: vec![], bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(uid.to_string()), body_text: None, body_html: None, has_list_header: false, size_bytes: Some(4000), tab_id: None,
    }
}

#[test]
fn while_held_on_battery_the_prefetch_fetches_inbox_only() {
    std::env::set_var("WHOOSH_CORE_CHECK_CONDITIONS", "battery:15");
    let dir = tempfile::tempdir().unwrap();
    let core = RustCore::open_production(dir.path().to_str().unwrap(), true).expect("the rig core opens");
    std::env::remove_var("WHOOSH_CORE_CHECK_CONDITIONS");
    let store = core.store();
    let account = store.add_account(USER, "Owner", MailProvider::Microsoft, "svc\u{1f}acct", false, config(), 1).unwrap();
    store.insert_messages((1..=3).flat_map(|u| [row(&account, "INBOX", u), row(&account, "Archive", u), row(&account, "Sent", u)]).collect()).unwrap();
    let source = Arc::new(Recording { asked: Mutex::new(vec![]) });
    core.set_body_source(&account, source.clone());
    assert_eq!(core.state("background/pause".into()).as_deref(), Some("\"battery\""), "the engine says why it waits");
    // the sync's passes over the three folders, then the crawl (held: it waits for the pause to end)
    for folder in ["INBOX", "Archive", "Sent"] { core.background().folder_synced(&account, folder); }
    core.background().kick(&account);
    core.background().crawl_kick(&account);
    std::thread::sleep(Duration::from_secs(3));
    let asked = source.asked.lock().unwrap().clone();
    let inbox = asked.iter().filter(|(f, _)| f == "INBOX").count();
    let other: Vec<_> = asked.iter().filter(|(f, _)| f != "INBOX").cloned().collect();
    assert!(inbox >= 1, "INBOX's newest bodies are fetched while held: {asked:?}");
    assert!(other.is_empty(), "no body of another folder while held: {other:?}");
    println!("PASS paused-prefetch-inbox-only: {inbox} INBOX fetch(es), none elsewhere, background/pause \"battery\"");
}
