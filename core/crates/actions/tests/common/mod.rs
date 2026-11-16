#![allow(dead_code)]
//! Helpers shared by the actions tests. Numbers come from docs/CONSTANTS.md through actions::constants, never from here.
use actions::{Env, MailAdapter, Refusal, WriteBack};
use contracts::{Clock, CoreError, InboxQuery, Message};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use store::{NewMessage, Store};

pub struct TempDb(pub PathBuf);
impl TempDb {
    pub fn new(tag: &str) -> TempDb {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("whoosh-actions-{tag}-{}-{nanos}", std::process::id()));
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

/// A server that accepts everything, remembers what it was asked, and refuses for the accounts it is told to.
#[derive(Default)]
pub struct FakeServer {
    pub asked: Mutex<Vec<WriteBack>>,
    refuse: Mutex<HashSet<String>>,
}
impl FakeServer {
    pub fn refuse(&self, account: &str) {
        self.refuse.lock().unwrap().insert(account.to_string());
    }
    pub fn accept_all(&self) {
        self.refuse.lock().unwrap().clear();
    }
    pub fn asked(&self) -> Vec<WriteBack> {
        self.asked.lock().unwrap().clone()
    }
}
fn account_of(w: &WriteBack) -> &str {
    match w {
        WriteBack::SetArchived { account_id, .. } | WriteBack::SetRead { account_id, .. } | WriteBack::Snooze { account_id, .. } | WriteBack::Pin { account_id, .. }
        | WriteBack::Label { account_id, .. } | WriteBack::MoveToFolder { account_id, .. } | WriteBack::EmptyFolders { account_id, .. } | WriteBack::Unsubscribe { account_id, .. } => account_id,
        // The target server is the one that can refuse a move.
        WriteBack::Move { to_account_id, .. } => to_account_id,
    }
}
impl MailAdapter for FakeServer {
    fn execute(&self, request: WriteBack) -> Result<(), Refusal> {
        let account = account_of(&request).to_string();
        self.asked.lock().unwrap().push(request);
        if self.refuse.lock().unwrap().contains(&account) {
            return Err(Refusal { account_id: account, reason_words: "the server said the mailbox is read-only right now".into(), retryable: true });
        }
        Ok(())
    }
}

pub struct FakeClock(pub AtomicI64, pub i32);
impl FakeClock {
    pub fn set(&self, ms: i64) {
        self.0.store(ms, Ordering::SeqCst);
    }
}
impl Clock for FakeClock {
    fn now_ms(&self) -> i64 {
        self.0.fetch_add(1, Ordering::SeqCst)
    }
    fn utc_offset_s(&self) -> i32 {
        self.1
    }
}

/// 2023-11-14 (a Tuesday) 10:30:00 UTC, in the fake clock's own zone (UTC+1).
pub const NOW: i64 = 1_699_957_800_000;
pub const OWN_A: &str = "owner@alpha.test";

pub struct Rig {
    pub db: TempDb,
    pub env: Env,
    pub store: Arc<Store>,
    pub server: Arc<FakeServer>,
    pub clock: Arc<FakeClock>,
}

pub fn constant(name: &str) -> u64 {
    let cell = actions::constants::cell(name).unwrap_or_else(|| panic!("docs/CONSTANTS.md has no {name} row"));
    let digits: String = cell.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().unwrap()
}

pub fn msg(id: &str, thread: &str, account: &str, from: &str, at: i64) -> Message {
    Message { list_unsubscribe: vec![], list_unsubscribe_one_click: false,
        id: id.into(), thread_id: thread.into(), account_id: account.into(), sender_name: format!("Name of {from}"), from_address: from.into(),
        to: vec![OWN_A.into()], subject: format!("Subject {id}"), preview: format!("Preview {id}"), received_at_ms: at,
        read: false, archived: false, folder: "INBOX".into(), attachment_names: vec![], login_code: None, trackers_blocked: 0,
        reply_to_name: None,
        cc: vec![], bcc: vec![], is_outgoing: false,
    }
}

pub fn new_msg(m: Message) -> NewMessage {
    let remote = Some(format!("r-{}", m.id));
    NewMessage { message: m, remote_id: remote, body_text: Some("the body".into()), body_html: None, has_list_header: false, size_bytes: None, tab_id: Some("primary".into()) }
}

/// Two accounts ("Alpha" = acc-a, "Beta" = acc-b) and the given messages.
pub fn rig(tag: &str, messages: Vec<NewMessage>) -> Rig {
    let db = TempDb::new(tag);
    let store = Arc::new(Store::open(&db.0).unwrap());
    store
        .write(|ctx| {
            for (i, (id, address, name)) in [("acc-a", OWN_A, "Alpha"), ("acc-b", "owner@beta.test", "Beta")].iter().enumerate() {
                ctx.tx.execute(
                    "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) \
                     VALUES (?1, ?2, 'imap', ?3, '#336699', ?4, 'manual', 'svc' || char(31) || ?1, 0)",
                    rusqlite::params![id, address, name, i as i64],
                )?;
            }
            Ok(())
        })
        .unwrap();
    store.insert_messages(messages).unwrap();
    let server = Arc::new(FakeServer::default());
    let clock = Arc::new(FakeClock(AtomicI64::new(NOW), 3600));
    let env = Env::new(store.clone(), server.clone(), clock.clone());
    Rig { db, env, store, server, clock }
}

/// Four inbound messages in the Primary tab, newest first by number: m4 is newest. m1 and m3 sit in acc-a, m2 and m4 in acc-b.
pub fn four() -> Vec<NewMessage> {
    (1..=4)
        .map(|i| {
            let account = if i % 2 == 1 { "acc-a" } else { "acc-b" };
            new_msg(msg(&format!("m{i}"), &format!("t{i}"), account, &format!("sender{i}@ext.test"), NOW - 100_000 + i * 1000))
        })
        .collect()
}

pub fn primary() -> InboxQuery {
    InboxQuery { tab_id: "primary".into(), account_id: None, label_id: None, mailbox: None, account_ids: vec![], filters: vec![], smart_folder_id: None }
}

pub fn inbox_ids(rig: &Rig) -> Vec<String> {
    rig.store.inbox(&primary()).unwrap().into_iter().map(|r| r.message_id).collect()
}

/// A scalar from the database, read through the writer so the test sees what the app would.
pub fn scalar(rig: &Rig, sql: &'static str) -> i64 {
    rig.store.write(move |ctx| Ok(ctx.tx.query_row(sql, [], |r| r.get::<_, i64>(0))?)).unwrap()
}

pub fn text(rig: &Rig, sql: &'static str) -> String {
    rig.store.write(move |ctx| Ok(ctx.tx.query_row(sql, [], |r| r.get::<_, String>(0))?)).unwrap()
}

pub fn assert_action_failed(e: CoreError, account_name: &str) {
    match e {
        CoreError::Action { failure } => {
            assert_eq!(failure.account_name, account_name, "the failure must name the account that refused");
            assert!(!failure.reason_words.is_empty(), "the failure must say why in words");
        }
        other => panic!("expected ActionFailed naming {account_name}, got {other:?}"),
    }
}

/// The store's counts must always equal a count from scratch.
pub fn assert_counts_match(rig: &Rig) {
    assert_eq!(rig.store.counts().unwrap(), rig.store.recount().unwrap(), "the tally drifted from a recount");
}
