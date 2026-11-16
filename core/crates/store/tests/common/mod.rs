#![allow(dead_code)]
//! Helpers shared by the store's tests. Numbers come from docs/CONSTANTS.md, never from here.
use contracts::{CoreEvent, EventSink, Message};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use store::{NewMessage, Store};

pub struct TempDb(pub PathBuf);
impl TempDb {
    pub fn new(tag: &str) -> TempDb {
        let dir = std::env::temp_dir().join(format!("whoosh-store-{tag}-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        TempDb(dir.join("whoosh.db"))
    }
}
impl Drop for TempDb {
    fn drop(&mut self) {
        if let Some(dir) = self.0.parent() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// A value from the docs/CONSTANTS.md ledger: the first integer in the Value cell of the row named `name`.
pub fn constant(name: &str) -> Option<u64> {
    let ledger = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/CONSTANTS.md");
    let text = std::fs::read_to_string(&ledger).unwrap_or_else(|e| panic!("cannot read {}: {e}", ledger.display()));
    let line = text.lines().find(|l| l.trim_start().starts_with(&format!("| {name} |")) || l.trim_start().starts_with(&format!("| {name} ")))?;
    let cell = line.split('|').nth(2)?;
    let digits: String = cell.trim().chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit() || *c == ',').filter(|c| *c != ',').collect();
    digits.parse().ok()
}

#[derive(Default)]
pub struct Collector(pub Mutex<Vec<CoreEvent>>);
impl EventSink for Collector {
    fn on_event(&self, event: CoreEvent) {
        self.0.lock().unwrap().push(event);
    }
}
impl Collector {
    pub fn take(&self) -> Vec<CoreEvent> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

pub fn msg(id: &str, thread: &str, account: &str, from: &str, at: i64) -> Message {
    Message { list_unsubscribe: vec![], list_unsubscribe_one_click: false,
        id: id.into(), thread_id: thread.into(), account_id: account.into(), sender_name: from.into(), from_address: from.into(),
        to: vec!["owner@example.test".into()], subject: format!("Subject {id}"), preview: format!("Preview {id}"), received_at_ms: at,
        read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0,
        reply_to_name: None,
        cc: vec![], bcc: vec![], is_outgoing: false,
    }
}

pub fn new_msg(m: Message, tab: Option<&str>) -> NewMessage {
    NewMessage { message: m, remote_id: None, body_text: None, body_html: None, has_list_header: false, size_bytes: None, tab_id: tab.map(String::from) }
}

/// Two accounts, then `n` inbound messages spread over them, one thread per two messages, tabs by index.
pub fn small_store(db: &TempDb, n: usize) -> Store {
    let store = Store::open(&db.0).unwrap();
    store
        .write(|ctx| {
            for (i, id) in ["acc-a", "acc-b"].iter().enumerate() {
                ctx.tx.execute(
                    "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) \
                     VALUES (?1, ?2, 'imap', ?1, '#336699', ?3, 'manual', 'svc' || char(31) || ?1, 0)",
                    rusqlite::params![id, format!("owner{i}@example.test"), i as i64],
                )?;
            }
            Ok(())
        })
        .unwrap();
    let tabs = ["primary", "needs-reply", "updates", "promotions", "social", "vips"];
    let batch: Vec<NewMessage> = (0..n)
        .map(|i| {
            let account = if i % 2 == 0 { "acc-a" } else { "acc-b" };
            new_msg(msg(&format!("m{i:03}"), &format!("t{:03}", i / 2), account, &format!("sender{}@ext.test", i % 7), 1_700_000_000_000 + i as i64 * 1000), Some(tabs[i % 6]))
        })
        .collect();
    store.insert_messages(batch).unwrap();
    store
}

/// The 1M-message generator the budget tests share (30 accounts, 4 messages per thread, six tabs, snoozes, pins).
pub const MESSAGES: u64 = 1_000_000;
pub const ACCOUNTS: u64 = 30;

pub fn build_fixture(path: &std::path::Path) {
    let mut conn = rusqlite::Connection::open(path).unwrap();
    conn.pragma_update(None, "journal_mode", "OFF").unwrap();
    conn.pragma_update(None, "synchronous", "OFF").unwrap();
    store::migrations::migrate(&mut conn).unwrap();
    let tx = conn.transaction().unwrap();
    tx.execute_batch(&format!(
        "WITH RECURSIVE s(n) AS (SELECT 0 UNION ALL SELECT n + 1 FROM s WHERE n < {a} - 1)
         INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms)
         SELECT 'acc-' || n, 'owner' || n || '@example.test', 'imap', 'Account ' || n, '#336699', n, 'manual', 'svc' || char(31) || n, 0 FROM s;
         WITH RECURSIVE s(n) AS (SELECT 0 UNION ALL SELECT n + 1 FROM s WHERE n < {t} - 1)
         INSERT INTO thread (id, account_id, subject, participants_json, last_message_at_ms)
         SELECT 't' || n, 'acc-' || (n % {a}), 'Thread ' || n, '[]', 1700000000000 + n * 4000 FROM s;
         WITH RECURSIVE s(i) AS (SELECT 0 UNION ALL SELECT i + 1 FROM s WHERE i < {m} - 1)
         INSERT INTO message (id, thread_id, account_id, sender_name, from_address, subject, preview, received_at_ms, read, archived, folder)
         SELECT 'm' || i, 't' || (i / 4), 'acc-' || ((i / 4) % {a}), 'Sender ' || (i % 5000),
                CASE WHEN i % 10 = 9 THEN 'owner' || ((i / 4) % {a}) || '@example.test' ELSE 'sender' || (i % 5000) || '@ext.test' END,
                'Subject ' || i, 'Preview of message ' || i, 1700000000000 + i * 1000,
                CASE WHEN i % 5 = 0 THEN 0 ELSE 1 END, CASE WHEN i % 11 = 0 THEN 1 ELSE 0 END,
                CASE WHEN i % 10 = 9 THEN 'Sent' ELSE 'INBOX' END FROM s;
         WITH RECURSIVE s(i) AS (SELECT 0 UNION ALL SELECT i + 1 FROM s WHERE i < {m} - 1)
         INSERT INTO message_category (message_id, tab_id, decided_by, rule_id, probability, decided_at_ms, updated_at_ms)
         SELECT 'm' || i,
                CASE WHEN h < 40 THEN 'primary' WHEN h < 45 THEN 'needs-reply' WHEN h < 70 THEN 'updates'
                     WHEN h < 90 THEN 'promotions' WHEN h < 98 THEN 'social' ELSE 'vips' END,
                'default', NULL, NULL, 0, 0 FROM (SELECT i, (i * 2654435761 % 4294967296) % 100 AS h FROM s);
         WITH RECURSIVE s(i) AS (SELECT 0 UNION ALL SELECT i + 97 FROM s WHERE i < {m} - 97)
         INSERT INTO snooze (message_id, return_at_ms, updated_at_ms) SELECT 'm' || i, 9000000000000, 0 FROM s;
         WITH RECURSIVE s(i) AS (SELECT 5 UNION ALL SELECT i + 1009 FROM s WHERE i < {m} - 1009)
         INSERT INTO pin (message_id, pinned_at_ms) SELECT 'm' || i, i FROM s;",
        a = ACCOUNTS, t = MESSAGES / 4, m = MESSAGES
    ))
    .unwrap();
    tx.commit().unwrap();
}

