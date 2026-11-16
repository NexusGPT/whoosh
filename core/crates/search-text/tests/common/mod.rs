#![allow(dead_code)]
//! Helpers for the search-text tests. Numbers come from docs/CONSTANTS.md, never from here.
use contracts::Message;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use store::{NewMessage, Store};

pub struct TempDir(pub PathBuf);
impl TempDir {
    pub fn new(tag: &str) -> TempDir {
        let dir = std::env::temp_dir().join(format!("whoosh-text-{tag}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }
    pub fn db(&self) -> PathBuf {
        self.0.join("whoosh.db")
    }
    pub fn index(&self) -> PathBuf {
        self.0.join("text-index")
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A value from the docs/CONSTANTS.md ledger: the first integer in the Value cell of the row named `name`.
pub fn constant(name: &str) -> u64 {
    let ledger = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/CONSTANTS.md");
    let text = std::fs::read_to_string(&ledger).unwrap_or_else(|e| panic!("cannot read {}: {e}", ledger.display()));
    let line = text.lines().find(|l| l.trim_start().starts_with(&format!("| {name} |"))).unwrap_or_else(|| panic!("{name} is not in docs/CONSTANTS.md"));
    let cell = line.split('|').nth(2).unwrap();
    let digits: String = cell.trim().chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit() || *c == ',').filter(|c| *c != ',').collect();
    digits.parse().unwrap_or_else(|_| panic!("{name} has no integer in its Value cell"))
}

pub fn msg(id: &str, subject: &str, from: &str, to: &str, at: i64) -> Message {
    Message { list_unsubscribe: vec![], list_unsubscribe_one_click: false,
        id: id.into(), thread_id: format!("t-{id}"), account_id: "acc-a".into(), sender_name: from.into(), from_address: format!("{}@ext.test", from.to_lowercase().replace(' ', ".")),
        to: vec![to.into()], subject: subject.into(), preview: format!("Preview of {id}"), received_at_ms: at, read: false, archived: false, folder: "INBOX".into(),
        attachment_names: vec![], login_code: None, trackers_blocked: 0,
        reply_to_name: None,
        cc: vec![], bcc: vec![], is_outgoing: false,
    }
}

pub fn new_msg(m: Message, body_text: Option<&str>, body_html: Option<&str>) -> NewMessage {
    NewMessage { message: m, remote_id: None, body_text: body_text.map(String::from), body_html: body_html.map(String::from), has_list_header: false, size_bytes: None, tab_id: Some("primary".into()) }
}

pub fn open_store(dir: &TempDir) -> Arc<Store> {
    let store = Arc::new(Store::open(&dir.db()).unwrap());
    store
        .write(|ctx| {
            ctx.tx.execute(
                "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) \
                 VALUES ('acc-a', 'owner@example.test', 'imap', 'A', '#336699', 0, 'manual', 'svc' || char(31) || 'a', 0)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
    store
}

pub fn insert(store: &Store, batch: Vec<NewMessage>) {
    store.insert_messages(batch).unwrap();
}

pub fn add_attachment(store: &Store, id: &str, message_id: &str, file_name: &str, extracted: Option<&str>) {
    let (id, message_id, file_name, extracted) = (id.to_string(), message_id.to_string(), file_name.to_string(), extracted.map(String::from));
    store
        .write(move |ctx| {
            ctx.tx.execute(
                "INSERT INTO attachment (id, message_id, file_name, mime_type, size_bytes, extracted_text) VALUES (?1, ?2, ?3, 'application/pdf', 10, ?4)",
                rusqlite::params![id, message_id, file_name, extracted],
            )?;
            Ok(())
        })
        .unwrap();
}

/// Poll `check` until it returns true; the elapsed time is the answer, None when `limit` passes first.
pub fn within(limit: Duration, mut check: impl FnMut() -> bool) -> Option<Duration> {
    let start = Instant::now();
    loop {
        if check() {
            return Some(start.elapsed());
        }
        if start.elapsed() > limit {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub fn ids(hits: &[contracts::SearchHit]) -> Vec<String> {
    hits.iter().map(|h| h.message_id.clone()).collect()
}
