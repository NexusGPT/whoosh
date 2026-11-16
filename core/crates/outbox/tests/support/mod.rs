#![allow(dead_code)]
//! Test doubles for the outbox. The SMTP server and the Sent-folder stub are the send lane's own test
//! support, included by path (not copied). Everything else here is small and local.
//!
//! Scenario figures (LATE_AFTER_MS, STANDBY_AFTER_MS) are the test's own: the ledger has no rows for them yet
//! (see docs/lanes/core-outbox.md "Contract deltas"). The undo window comes from the ledger through the crate.
#[path = "../../../send/tests/support/mod.rs"]
pub mod smtp;

use contracts::{Clock, CloudSync, CoreError, CoreEvent, Draft, EventSink, OutboxState, Signature, SyncBatch, SyncChange};
use outbox::adapters::cloud::{CloudPublisher, CloudPuller};
use outbox::adapters::reply::StoreReplyContext;
use outbox::adapters::sender::SendCrateSender;
use outbox::adapters::store::StoreOutbox;
use outbox::ports::{GetEntryPort, ListOpenEntriesPort, OnlinePort, SendAccountPort, SendEntryPort};
use outbox::{Delivered, DeviceRole, Outbox, OutboxConfig, Ports, SyncPorts};
use send::{AppendSentPort, AppendSentRequest, LettreDelivery, ParentHeaders, ParentHeadersPort, ParentRef, ReplyContext, Security, SendAccount, SmtpSettings};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use store::Store;

pub use smtp::{FakeSmtp, Mode};
#[allow(unused_imports)]
pub use smtp::REFUSAL_TEXT;

pub const ADDRESS: &str = "assem@northlight-example.de";
pub const ACCOUNT_ID: &str = "acc-nordlicht";
/// 2026-11-09 08:00:00 UTC, the "08:00" of the schedule scenarios.
pub const EIGHT: i64 = 1_794_211_200_000;
pub const MINUTE: i64 = 60_000;
pub const LATE_AFTER_MS: i64 = 60_000;
pub const STANDBY_AFTER_MS: i64 = 120_000;

pub struct TestClock(AtomicI64);
impl TestClock {
    pub fn at(ms: i64) -> Arc<TestClock> {
        Arc::new(TestClock(AtomicI64::new(ms)))
    }
    pub fn set(&self, ms: i64) {
        self.0.store(ms, Ordering::SeqCst);
    }
    pub fn advance(&self, ms: i64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}
impl Clock for TestClock {
    fn now_ms(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
    fn utc_offset_s(&self) -> i32 {
        0
    }
}

pub struct Net(AtomicBool);
impl Net {
    pub fn up() -> Arc<Net> {
        Arc::new(Net(AtomicBool::new(true)))
    }
    pub fn set(&self, online: bool) {
        self.0.store(online, Ordering::SeqCst);
    }
}
impl OnlinePort for Net {
    fn execute(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// The account the sender uses; the SMTP port can be switched between servers.
pub struct Accounts(pub AtomicU16);
impl Accounts {
    pub fn on(port: u16) -> Arc<Accounts> {
        Arc::new(Accounts(AtomicU16::new(port)))
    }
}
impl SendAccountPort for Accounts {
    fn execute(&self, account_id: &str) -> Result<(SendAccount, Option<Signature>), CoreError> {
        assert_eq!(account_id, ACCOUNT_ID);
        let account = SendAccount {
            id: ACCOUNT_ID.into(),
            name: "Nordlicht".into(),
            address: ADDRESS.into(),
            smtp: SmtpSettings { host: "127.0.0.1".into(), port: self.0.load(Ordering::SeqCst), security: Security::Plaintext, credentials: None },
        };
        Ok((account, Some(Signature { address: ADDRESS.into(), text: "Assem Chammah | Nordlicht".into() })))
    }
}

/// The Sent folder: records every APPEND; can be told to refuse.
pub struct SentFolder {
    pub appended: Mutex<Vec<AppendSentRequest>>,
    pub refuse: AtomicBool,
}
impl SentFolder {
    pub fn new() -> Arc<SentFolder> {
        Arc::new(SentFolder { appended: Mutex::new(Vec::new()), refuse: AtomicBool::new(false) })
    }
    pub fn count(&self) -> usize {
        self.appended.lock().unwrap().len()
    }
}
impl AppendSentPort for SentFolder {
    fn execute(&self, request: &AppendSentRequest) -> Result<(), CoreError> {
        if self.refuse.load(Ordering::SeqCst) {
            return Err(CoreError::Unexpected { reason_words: "the Sent folder is full".into() });
        }
        self.appended.lock().unwrap().push(AppendSentRequest { account_id: request.account_id.clone(), rfc822: request.rfc822.clone() });
        Ok(())
    }
}

/// Wraps the real sender and records the stored state of the entry at the moment of delivery.
pub struct LedgerProbe {
    pub inner: Arc<dyn SendEntryPort>,
    pub list: Arc<dyn ListOpenEntriesPort>,
    pub seen: Mutex<Vec<String>>,
}
impl SendEntryPort for LedgerProbe {
    fn execute(&self, draft: &Draft, reply: Option<&ReplyContext>) -> Result<Delivered, CoreError> {
        let records = self.list.execute()?;
        let record = records.iter().find(|r| r.entry.draft.id == draft.id).expect("the entry is stored when delivery starts");
        self.seen.lock().unwrap().push(format!("{:?}", record.entry.state));
        self.inner.execute(draft, reply)
    }
}

/// The mail server's answer to "what are the headers of this message?": configured by the test, every ask recorded.
#[derive(Default)]
pub struct Parents {
    pub answer: Mutex<Option<Result<ParentHeaders, CoreError>>>,
    pub asked: Mutex<Vec<ParentRef>>,
}
impl ParentHeadersPort for Parents {
    fn execute(&self, parent: &ParentRef) -> Result<ParentHeaders, CoreError> {
        self.asked.lock().unwrap().push(parent.clone());
        self.answer.lock().unwrap().clone().unwrap_or_else(|| Err(CoreError::Unexpected { reason_words: "the test gave no parent headers".into() }))
    }
}

#[derive(Default)]
pub struct Collector(pub Mutex<Vec<CoreEvent>>);
impl EventSink for Collector {
    fn on_event(&self, event: CoreEvent) {
        self.0.lock().unwrap().push(event);
    }
}
impl Collector {
    pub fn states(&self, outbox_id: &str) -> Vec<OutboxState> {
        self.0.lock().unwrap().iter().filter_map(|e| match e { CoreEvent::OutboxTick { entry } if entry.id == outbox_id => Some(entry.state.clone()), _ => None }).collect()
    }
    pub fn completed(&self) -> usize {
        self.0.lock().unwrap().iter().filter(|e| matches!(e, CoreEvent::SendCompleted { .. })).count()
    }
    pub fn len(&self) -> usize {
        self.0.lock().unwrap().len()
    }
}

/// The shared sync store of the two-device tests: an append-only log; a token is the log length.
#[derive(Default)]
pub struct FakeCloud {
    pub log: Mutex<Vec<SyncChange>>,
    pub down: AtomicBool,
}
impl FakeCloud {
    pub fn new() -> Arc<FakeCloud> {
        Arc::new(FakeCloud::default())
    }
    pub fn len(&self) -> usize {
        self.log.lock().unwrap().len()
    }
}
impl CloudSync for FakeCloud {
    fn push(&self, change: SyncChange) -> Result<(), CoreError> {
        if self.down.load(Ordering::SeqCst) {
            return Err(CoreError::Unexpected { reason_words: "the sync store is unreachable".into() });
        }
        self.log.lock().unwrap().push(change);
        Ok(())
    }
    fn pull(&self, since_token: Option<String>) -> Result<SyncBatch, CoreError> {
        if self.down.load(Ordering::SeqCst) {
            return Err(CoreError::Unexpected { reason_words: "the sync store is unreachable".into() });
        }
        let log = self.log.lock().unwrap();
        let from: usize = since_token.map(|t| t.parse().expect("token")).unwrap_or(0);
        Ok(SyncBatch { changes: log[from..].to_vec(), next_token: Some(log.len().to_string()) })
    }
}

pub struct TempDb(pub PathBuf);
impl TempDb {
    pub fn new(tag: &str) -> TempDb {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = SERIAL.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("whoosh-outbox-{tag}-{}-{n}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
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

/// One device: its own store, clock, network, Sent folder and outbox; the SMTP port and the cloud are shared.
pub struct Device {
    pub db: TempDb,
    pub store: Arc<Store>,
    pub rows: Arc<StoreOutbox>,
    pub clock: Arc<TestClock>,
    pub net: Arc<Net>,
    pub sent: Arc<SentFolder>,
    pub accounts: Arc<Accounts>,
    pub probe: Arc<LedgerProbe>,
    pub parents: Arc<Parents>,
    pub events: Arc<Collector>,
    pub outbox: Outbox,
    cloud: Option<Arc<FakeCloud>>,
    role: DeviceRole,
    name: String,
}

impl Device {
    pub fn new(name: &str, role: DeviceRole, start_ms: i64, smtp_port: u16, cloud: Option<Arc<FakeCloud>>) -> Device {
        let db = TempDb::new(name);
        let store = Arc::new(Store::open(&db.0).expect("open store"));
        let rows = Arc::new(StoreOutbox::new(store.clone()));
        let clock = TestClock::at(start_ms);
        let net = Net::up();
        let sent = SentFolder::new();
        let accounts = Accounts::on(smtp_port);
        let sender: Arc<dyn SendEntryPort> = Arc::new(SendCrateSender::new(accounts.clone(), Arc::new(LettreDelivery), sent.clone(), clock.clone()));
        let probe = Arc::new(LedgerProbe { inner: sender, list: rows.clone(), seen: Mutex::new(Vec::new()) });
        let events = Arc::new(Collector::default());
        let parents = Arc::new(Parents::default());
        insert_account(&store);
        let outbox = build_outbox(&store, &rows, &probe, &parents, &net, &sent, &clock, &events, &cloud, role, name);
        Device { db, store, rows, clock, net, sent, accounts, probe, parents, events, outbox, cloud, role, name: name.to_string() }
    }

    /// The app restarting: a new outbox (and a new sync token) over the same store.
    pub fn restart(&mut self) {
        self.outbox = build_outbox(&self.store, &self.rows, &self.probe, &self.parents, &self.net, &self.sent, &self.clock, &self.events, &self.cloud, self.role, &self.name);
    }

    /// A draft in the draft table, as the composer leaves it.
    pub fn put_draft(&self, id: &str, body: &str) -> Draft {
        self.put_draft_in(id, body, None)
    }

    /// A draft that answers (or, sent with `send_forward`, forwards) the thread `thread`.
    pub fn put_draft_in(&self, id: &str, body: &str, thread: Option<&str>) -> Draft {
        let draft = Draft {
            id: id.into(),
            from_account_id: ACCOUNT_ID.into(),
            to: vec!["Linea Supply <orders@linea-example.com>".into()],
            cc: vec![],
            subject: "Re: Quote for the Q1 print run".into(),
            body: body.into(),
            attachment_ids: vec![],
            ai_marks: vec![],
            reply_to_thread_id: thread.map(str::to_string),
            bcc: vec![],
        };
        let d = draft.clone();
        self.store
            .write(move |ctx| {
                ctx.tx.execute(
                    "INSERT INTO draft (id, from_account_id, to_json, cc_json, subject, body, attachment_ids_json, ai_marks_json, reply_to_thread_id, updated_at_ms) VALUES (?1,?2,?3,'[]',?4,?5,'[]','[]',?6,1)",
                    rusqlite::params![d.id, d.from_account_id, serde_json::to_string(&d.to).unwrap(), d.subject, d.body, d.reply_to_thread_id],
                )?;
                Ok(())
            })
            .expect("insert draft");
        draft
    }

    /// A thread with one message the mail server holds at `folder`/`uid`; the server will answer `headers` for it.
    pub fn put_parent(&self, thread: &str, folder: &str, uid: u32, headers: ParentHeaders) {
        let (t, f) = (thread.to_string(), folder.to_string());
        self.store
            .write(move |ctx| {
                ctx.tx.execute("INSERT INTO thread (id, account_id, subject) VALUES (?1, ?2, 'Quote for the Q1 print run')", rusqlite::params![t, ACCOUNT_ID])?;
                ctx.tx.execute(
                    "INSERT INTO message (id, thread_id, account_id, remote_id, sender_name, from_address, subject, received_at_ms, folder) \
                     VALUES (?1, ?2, ?3, ?4, 'Linea Supply', 'orders@linea-example.com', 'Quote for the Q1 print run', 1000, ?5)",
                    rusqlite::params![format!("{t}-m1"), t, ACCOUNT_ID, uid.to_string(), f],
                )?;
                Ok(())
            })
            .expect("insert parent");
        *self.parents.answer.lock().unwrap() = Some(Ok(headers));
    }

    pub fn draft_rows(&self) -> i64 {
        self.store.write(|ctx| Ok(ctx.tx.query_row("SELECT COUNT(*) FROM draft", [], |r| r.get(0))?)).expect("count drafts")
    }

    pub fn state(&self, outbox_id: &str) -> OutboxState {
        GetEntryPort::execute(&*self.rows, outbox_id).expect("read").expect("entry exists").entry.state
    }

    pub fn save_raw(&self, record: &outbox::Record) {
        outbox::ports::SaveEntryPort::execute(&*self.rows, &outbox::SaveRequest { record: record.clone(), draft: outbox::DraftEffect::Keep }).expect("save");
    }
}

fn insert_account(store: &Store) {
    store
        .write(|ctx| {
            ctx.tx.execute(
                "INSERT INTO account (id, address, provider, display_name, colour, sort_order, credential_kind, keychain_ref, updated_at_ms) \
                 VALUES (?1, ?2, 'imap', 'Nordlicht', '#9C36B5', 0, 'manual', 'whoosh' || char(31) || 'nordlicht', 1)",
                [ACCOUNT_ID, ADDRESS],
            )?;
            Ok(())
        })
        .expect("insert account");
}

#[allow(clippy::too_many_arguments)]
fn build_outbox(store: &Arc<Store>, rows: &Arc<StoreOutbox>, probe: &Arc<LedgerProbe>, parents: &Arc<Parents>, net: &Arc<Net>, sent: &Arc<SentFolder>, clock: &Arc<TestClock>, events: &Arc<Collector>, cloud: &Option<Arc<FakeCloud>>, role: DeviceRole, name: &str) -> Outbox {
    let sync = cloud.as_ref().map(|c| SyncPorts { publish: Arc::new(CloudPublisher::new(c.clone())), pull: Arc::new(CloudPuller::new(c.clone())) });
    let ports = Ports {
        save: rows.clone(),
        get: rows.clone(),
        list_open: rows.clone(),
        find_by_key: rows.clone(),
        pending_copies: rows.clone(),
        reply_context: Arc::new(StoreReplyContext::new(store.clone(), parents.clone())),
        load_draft: rows.clone(),
        account_names: rows.clone(),
        sender: probe.clone(),
        online: net.clone(),
        append_sent: sent.clone(),
        clock: clock.clone(),
        sync,
    };
    let outbox = Outbox::open(ports, OutboxConfig::new(name, role, LATE_AFTER_MS).expect("config")).expect("open outbox");
    outbox.subscribe(events.clone()).expect("subscribe");
    outbox
}
