//! Fake-adapter accounts for the engine tests. Nothing here touches a network: each account is an
//! in-memory mailbox whose messages are numbered 1 (oldest) to N (newest) per folder.
//! Fixture sizes (30 accounts, 3 folders, page 40, backoff 2..50 ms, 3 retries) are test inputs, not ledger numbers.
#![allow(dead_code)]
use async_trait::async_trait;
use contracts::{Account, Clock, CoreEvent, EventSink, MailProvider, Message, StreamKind, SyncCursor, SyncState};
use rusqlite::{params, Connection, OpenFlags};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use store::{NewMessage, Store};
use sync_engine::{AccountSource, Engine, EngineConfig, Outcome, Page, SourceError};

pub const ACCOUNTS: usize = 30;
pub const FOLDERS: [(&str, i64); 3] = [("INBOX", 180), ("Archive", 70), ("Sent", 30)];
pub const PER_ACCOUNT: i64 = 280;
pub const REFUSING: usize = 7;
pub const THROTTLED: usize = 13;
pub const REFUSAL_WORDS: &str = "[AUTHENTICATIONFAILED] Invalid credentials (Failure)";
pub const THROTTLE_WORDS: &str = "ApplicationThrottled: MailboxConcurrency limit exceeded";

pub fn config() -> EngineConfig {
    EngineConfig::from_ledger(40, 2, 50, 3)
}

pub fn account(i: usize) -> Account {
    let provider = if i == THROTTLED { MailProvider::Microsoft } else { [MailProvider::Google, MailProvider::Icloud, MailProvider::Imap][i % 3] };
    Account {
        id: format!("acct{i:02}"),
        address: format!("owner{i}@mail{i}.example"),
        provider,
        display_name: format!("Account {i}"),
        colour: "#336699".into(),
        order: i as u32,
        paused: false,
        group: None,
        icon_id: None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub account: String,
    pub folder: String,
    pub before: Option<i64>,
    pub backfill: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub messages_per_account: HashMap<String, i64>,
    pub inbox_events: usize,
}

#[derive(Default)]
pub struct Log {
    pub db_path: PathBuf,
    pub calls: Mutex<Vec<Call>>,
    pub fetched: Mutex<HashMap<String, u32>>,
    inflight: Mutex<HashMap<String, (usize, usize)>>,
    pub max_inflight: Mutex<HashMap<String, usize>>,
    pub max_backfilling_accounts: AtomicUsize,
    pub inbox_events: AtomicUsize,
    pub first_backfill: Mutex<Option<Snapshot>>,
    pub backfill_pages: AtomicUsize,
    pub kill_after: Mutex<Option<usize>>,
    pub killed: tokio::sync::Notify,
}

impl Log {
    fn snapshot_once(&self) {
        let mut slot = self.first_backfill.lock().unwrap();
        if slot.is_some() {
            return;
        }
        let c = Connection::open_with_flags(&self.db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let mut st = c.prepare("SELECT account_id, count(*) FROM message GROUP BY account_id").unwrap();
        let counts = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))).unwrap().map(Result::unwrap).collect();
        *slot = Some(Snapshot { messages_per_account: counts, inbox_events: self.inbox_events.load(SeqCst) });
    }

    fn enter(&self, account: &str, backfill: bool) {
        let mut map = self.inflight.lock().unwrap();
        let e = map.entry(account.to_string()).or_default();
        e.0 += 1;
        if backfill {
            e.1 += 1;
        }
        let n = e.0;
        let accounts_backfilling = map.values().filter(|v| v.1 > 0).count();
        drop(map);
        let mut max = self.max_inflight.lock().unwrap();
        let m = max.entry(account.to_string()).or_default();
        *m = (*m).max(n);
        self.max_backfilling_accounts.fetch_max(accounts_backfilling, SeqCst);
    }

    fn leave(&self, account: &str, backfill: bool) {
        let mut map = self.inflight.lock().unwrap();
        let e = map.get_mut(account).unwrap();
        e.0 -= 1;
        if backfill {
            e.1 -= 1;
        }
    }
}

pub struct FakeSource {
    pub account: Account,
    pub index: usize,
    pub refuse: Option<String>,
    pub throttle_left: AtomicUsize,
    pub log: Arc<Log>,
}

pub fn message(acct: &Account, folder: &str, folder_no: usize, k: i64) -> NewMessage {
    let id = format!("{}:{folder}:{k}", acct.id);
    NewMessage {
        message: Message { list_unsubscribe: vec![], list_unsubscribe_one_click: false,
            id: id.clone(),
            thread_id: id,
            account_id: acct.id.clone(),
            sender_name: format!("Sender {k}"),
            from_address: format!("sender{k}@other.example"),
            to: vec![acct.address.clone()],
            subject: format!("Message {k}"),
            preview: String::new(),
            received_at_ms: 1_700_000_000_000 + folder_no as i64 * 10_000_000 + k * 1_000,
            read: false,
            archived: false,
            folder: folder.to_string(),
            attachment_names: vec![],
            login_code: None,
            trackers_blocked: 0,
            reply_to_name: None,
            cc: vec![],
            bcc: vec![], is_outgoing: false,
        },
        remote_id: Some(k.to_string()),
        body_text: None,
        body_html: None,
        has_list_header: false,
        size_bytes: None,
        tab_id: None,
    }
}

#[async_trait]
impl AccountSource for FakeSource {
    async fn list_folders(&self) -> Result<Vec<String>, SourceError> {
        if let Some(words) = &self.refuse {
            return Err(SourceError::Failed { reason_words: words.clone(), retryable: false, code: None });
        }
        Ok(FOLDERS.iter().map(|(f, _)| f.to_string()).collect())
    }

    async fn page(&self, folder: &str, before: Option<i64>, limit: u32) -> Result<Page, SourceError> {
        let backfill = folder != "INBOX" || before.is_some();
        if backfill {
            self.log.snapshot_once();
            let kill = *self.log.kill_after.lock().unwrap();
            if kill.is_some_and(|k| self.log.backfill_pages.load(SeqCst) >= k) {
                self.log.killed.notify_one();
                std::future::pending::<()>().await;
            }
            if self.throttle_left.load(SeqCst) > 0 {
                self.throttle_left.fetch_sub(1, SeqCst);
                return Err(SourceError::Throttled { retry_after_ms: Some(5), reason_words: THROTTLE_WORDS.into() });
            }
        }
        self.log.enter(&self.account.id, backfill);
        // The tests run on tokio's paused clock: this sleep costs no wall time. Time moves only when every task
        // waits and no store write is running, so a later account's first screen ends after earlier accounts'
        // writes, and a backfill that does not wait for every first screen overlaps them.
        let delay = if backfill { 1 } else { 2 + 20 * self.index as u64 };
        tokio::time::sleep(Duration::from_millis(delay)).await;
        let (folder_no, n) = FOLDERS.iter().enumerate().find(|(_, (f, _))| *f == folder).map(|(i, (_, n))| (i, *n)).unwrap();
        let top = before.map(|b| b - 1).unwrap_or(n);
        let low = (top - limit as i64 + 1).max(1);
        let messages: Vec<NewMessage> = (low..=top).rev().map(|k| message(&self.account, folder, folder_no, k)).collect();
        let oldest = if messages.is_empty() { None } else { Some(low) };
        {
            let mut fetched = self.log.fetched.lock().unwrap();
            for m in &messages {
                *fetched.entry(m.message.id.clone()).or_default() += 1;
            }
        }
        self.log.calls.lock().unwrap().push(Call { account: self.account.id.clone(), folder: folder.into(), before, backfill });
        if backfill {
            self.log.backfill_pages.fetch_add(1, SeqCst);
        }
        self.log.leave(&self.account.id, backfill);
        let cursor = before.is_none().then_some(SyncCursor::Imap { uid_validity: 1, highest_mod_seq: n as u64 });
        Ok(Page { messages, oldest, done: oldest.is_none_or(|o| o <= 1), cursor })
    }
}

pub struct Events {
    pub states: Mutex<Vec<SyncState>>,
    log: Arc<Log>,
}

impl EventSink for Events {
    fn on_event(&self, event: CoreEvent) {
        match event {
            CoreEvent::SyncStateChanged { state } => self.states.lock().unwrap().push(state),
            CoreEvent::InboxChanged { .. } => {
                self.log.inbox_events.fetch_add(1, SeqCst);
            }
            _ => {}
        }
    }
}

pub struct TestClock;
impl Clock for TestClock {
    fn now_ms(&self) -> i64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64
    }
    fn utc_offset_s(&self) -> i32 {
        0
    }
}

pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub log: Arc<Log>,
    pub events: Arc<Events>,
    pub accounts: Vec<Account>,
    pub throttle: bool,
}

pub fn fixture(throttle: bool) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let log = Arc::new(Log { db_path: dir.path().join("whoosh.sqlite"), ..Default::default() });
    let events = Arc::new(Events { states: Mutex::new(vec![]), log: log.clone() });
    Fixture { dir, log, events, accounts: (0..ACCOUNTS).map(account).collect(), throttle }
}

pub fn open_store(f: &Fixture) -> Arc<Store> {
    let store = Arc::new(Store::open(&f.log.db_path).unwrap());
    let accounts = f.accounts.clone();
    store
        .write(move |ctx| {
            for a in &accounts {
                let provider = match a.provider {
                    MailProvider::Google => "google",
                    MailProvider::Microsoft => "microsoft",
                    MailProvider::Icloud => "icloud",
                    MailProvider::Fastmail => "fastmail",
                    MailProvider::Imap => "imap",
                };
                ctx.tx.execute(
                    "INSERT OR IGNORE INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'manual', ?7, 0)",
                    params![a.id, a.address, provider, a.display_name, a.colour, a.order, format!("whoosh\u{1f}{}", a.id)],
                )?;
            }
            Ok(())
        })
        .unwrap();
    store.subscribe(StreamKind::Inbox, f.events.clone());
    store
}

pub fn sources(f: &Fixture) -> Vec<(Account, Arc<dyn AccountSource>)> {
    f.accounts
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let src = FakeSource {
                account: a.clone(),
                index: i,
                refuse: (i == REFUSING).then(|| REFUSAL_WORDS.to_string()),
                throttle_left: AtomicUsize::new(if f.throttle && i == THROTTLED { 3 } else { 0 }),
                log: f.log.clone(),
            };
            (a.clone(), Arc::new(src) as Arc<dyn AccountSource>)
        })
        .collect()
}

/// One full run, bounded: a stall (one account holding the others) is a failure, not a hang. On the paused
/// clock the 60 s pass in no wall time once every task is stuck.
pub async fn run(f: &Fixture, store: Arc<Store>) -> HashMap<String, Outcome> {
    let engine = Engine::new(store, f.events.clone(), Arc::new(TestClock), config());
    let out = tokio::time::timeout(Duration::from_secs(60), engine.run(sources(f))).await.expect("the engine stalled: 60 s without every account ending");
    out.into_iter().collect()
}

pub fn db(path: &Path) -> Connection {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}
