//! engine-act: the outbox of the real core. `outbox::Outbox` over the store's own adapters, a sender over the send crate (lettre) with
//! each account's SMTP settings (as the sign-in check builds them: a password from the Keychain, or an OAuth2 access token sent by
//! XOAUTH2), the Sent copy appended over IMAP on the account's live adapter. A timer thread ticks the outbox every OUTBOX_TICK_MS while
//! an entry is open (the undo window and send-later are the outbox crate's, from the ledger); `SendNow` sends at once. A message that
//! left is stored as `sent-<outbox id>` in its thread (outgoing, read, out of the inbox: delta 15), and the reply reminder its draft
//! or its entry held follows it (`reminders::follow_sent`).
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use contracts::{
    AppendSentPort, AppendSentRequest, Clock, CoreError, CoreEvent, Draft, EventSink, Message, OutboxEntry, OutboxState, StreamKind,
};
use outbox::adapters::reply::StoreReplyContext;
use outbox::adapters::sender::SendCrateSender;
use outbox::adapters::store::StoreOutbox;
use outbox::{DeviceRole, Outbox, OutboxConfig, Ports, SendAccountPort};
use send::{ParentHeaders, ParentHeadersPort, ParentRef, SendAccount, SmtpCredentials, SmtpSettings};
use store::{NewMessage, Store};
use sync_imap::ImapMailAdapter;

use crate::rust_core::{Keyring, TestTransport};
use crate::sync::{emit, is_loopback, Sinks, SyncDeps};

/// How often the outbox is ticked while an entry is open: the ledger's OUTBOX_TICK_MS (docs/CONSTANTS.md), generated into crate::consts.
pub const OUTBOX_TICK_MS: u64 = crate::consts::OUTBOX_TICK_MS;
/// The name this device gives its entries: one Mac, the primary sender (docs/CONSTANTS.md OUTBOX_STANDBY_AFTER_MS is the iPhone's).
const DEVICE_ID: &str = "mac";

fn unexpected(words: impl Into<String>) -> CoreError { CoreError::Unexpected { reason_words: words.into() } }

fn block<T: Send>(rt: &tokio::runtime::Handle, f: impl std::future::Future<Output = T> + Send) -> Result<T, CoreError> {
    std::thread::scope(|s| s.spawn(|| rt.block_on(f)).join()).map_err(|_| unexpected("the mail server call stopped unexpectedly"))
}

/// The account's live IMAP adapter (the one its sync opened), or a retryable refusal in words.
fn live(deps: &SyncDeps, account_id: &str) -> Result<Arc<sync_imap::ImapAdapter>, CoreError> {
    deps.adapters.lock().unwrap_or_else(|e| e.into_inner()).get(account_id).cloned().ok_or_else(|| CoreError::Action {
        failure: contracts::ActionFailed { account_name: account_id.to_string(), reason_words: "this account is not connected to its mail server yet".into(), retryable: true, code: None },
    })
}

/// The Sent copy over IMAP (sync-imap's `append_sent`: the `\Sent` folder, nothing for Gmail).
pub struct LiveSentAppender { pub deps: Arc<SyncDeps>, pub runtime: tokio::runtime::Handle }
impl AppendSentPort for LiveSentAppender {
    fn execute(&self, request: &AppendSentRequest) -> Result<(), CoreError> {
        let a = live(&self.deps, &request.account_id)?;
        block(&self.runtime, async { a.append_sent(&request.rfc822).await })?
    }
}

/// The Message-ID and References of the message a reply answers, read from its server (sync-imap's `message_headers`).
pub struct LiveParentHeaders { pub deps: Arc<SyncDeps>, pub runtime: tokio::runtime::Handle }
impl ParentHeadersPort for LiveParentHeaders {
    fn execute(&self, parent: &ParentRef) -> Result<ParentHeaders, CoreError> {
        let uid: u32 = parent.remote_id.parse().map_err(|_| CoreError::Invalid { field: "remote_id".into(), reason_words: format!("'{}' is not an IMAP UID", parent.remote_id) })?;
        let a = live(&self.deps, &parent.account_id)?;
        let block_bytes = block(&self.runtime, async { a.message_headers(&parent.folder, uid).await })??;
        sync_imap::parent_headers_from(&block_bytes)
    }
}

/// The sending account: its SMTP settings from the store, its secret read for this call only.
pub struct StoreSendAccounts {
    pub store: Arc<Store>,
    pub keychain: Arc<dyn Keyring>,
    pub endpoint: Arc<dyn oauth::TokenEndpoint>,
    pub transport: TestTransport,
}
impl SendAccountPort for StoreSendAccounts {
    fn execute(&self, account_id: &str) -> Result<(SendAccount, Option<contracts::Signature>), CoreError> {
        let a = self.store.sync_accounts().map_err(|e| unexpected(format!("the accounts could not be read: {e}")))?
            .into_iter().find(|a| a.account.id == account_id).ok_or_else(|| CoreError::NotFound { kind: "account".into(), id: account_id.to_string() })?;
        // real-accounts: a paused account sends nothing; the entry stays in the Outbox with these words and leaves when the account is resumed
        if a.account.paused { return Err(CoreError::Action { failure: crate::rust_core::account_cmds::held_failure(&a.account.display_name) }); }
        let config = a.config.clone().ok_or_else(|| CoreError::Invalid { field: "account".into(), reason_words: "this account has no sending settings".into() })?;
        let r = a.keychain_ref.split_once('\u{1f}').map(|(s, acc)| contracts::KeychainRef { service: s.to_string(), account: acc.to_string() })
            .ok_or_else(|| unexpected("the stored credential reference has no separator"))?;
        let oauth = a.oauth || config.auth_method == contracts::AuthMethod::Oauth2;
        let secret = if oauth {
            let token = crate::sign_in::token::at_launch(self.endpoint.clone(), self.keychain.clone(), r).map_err(unexpected)?;
            token.access_token().map_err(|e| e)?
        } else {
            let s = secrets::ReadSecret::execute(&*self.keychain, &r).map_err(|e| unexpected(e.to_string()))?;
            String::from_utf8(s.expose().to_vec()).map_err(|e| unexpected(e.to_string()))?
        };
        let plain = self.transport == TestTransport::PlainLoopback && is_loopback(&config.smtp_host);
        let security = if plain { send::Security::Plaintext } else {
            match config.smtp_security { contracts::ConnectionSecurity::Ssl => send::Security::Implicit, contracts::ConnectionSecurity::Starttls => send::Security::StartTls }
        };
        let username = if config.smtp_username.trim().is_empty() { config.username.clone() } else { config.smtp_username.clone() };
        let smtp = SmtpSettings { host: config.smtp_host.clone(), port: config.smtp_port, security, credentials: Some(SmtpCredentials { username, secret, bearer: oauth }) };
        Ok((SendAccount { id: a.account.id.clone(), name: a.account.display_name.clone(), address: a.account.address.clone(), smtp }, self.store.signature(&a.account.address).map_err(|e| unexpected(format!("the signature could not be read: {e}")))?))   // real-compose: the saved signature leaves with the mail (the send crate adds it once)
    }
}

/// Whether there is a network path. The real core has no path monitor of its own yet: a send that finds none fails in the
/// server's words and is retried by the owner (the outbox crate's waiting state needs the OS monitor, owned by Swift).
struct AlwaysOnline;
impl contracts::OnlinePort for AlwaysOnline { fn execute(&self) -> bool { true } }

/// The outbox stream, forwarded to the core's own sinks (StreamKind::Outbox).
struct Forward(Sinks);
impl EventSink for Forward { fn on_event(&self, event: CoreEvent) { emit(&self.0, StreamKind::Outbox, event) } }

/// Z inside the undo window: the actions crate's `SendCanceller`, over the outbox.
pub struct CancelViaOutbox(pub Arc<Outbox>);
impl actions::SendCanceller for CancelViaOutbox {
    fn execute(&self, outbox_id: &str) -> Result<(), actions::Refusal> {
        self.0.cancel(outbox_id).map(|_| ()).map_err(|e| actions::Refusal { account_id: String::new(), reason_words: e.to_string(), retryable: false })
    }
}

/// The outbox, its timer, and what a sent message leaves in the store.
pub struct MailOut {
    pub outbox: Arc<Outbox>,
    store: Arc<Store>,
    deps: Arc<SyncDeps>,
    clock: Arc<dyn Clock>,
    kicked: Mutex<bool>,
    wake: Condvar,
    stop: AtomicBool,
}

/// fix-realcore round 15b: every try's SMTP steps go to `<data_dir>/core.log` (send/src/smtp.rs `LoggedDelivery`: codes and server words only).
fn smtp_log(store: &Store) -> send::StepLog {
    let path = crate::log::path_beside(store.path());
    Arc::new(move |line: &str| crate::log::log_line(&path, line))
}

impl MailOut {
    pub fn open(store: Arc<Store>, deps: Arc<SyncDeps>, runtime: tokio::runtime::Handle, keychain: Arc<dyn Keyring>, endpoint: Arc<dyn oauth::TokenEndpoint>, transport: TestTransport, clock: Arc<dyn Clock>) -> Result<Arc<MailOut>, CoreError> {
        let rows = Arc::new(StoreOutbox::new(store.clone()));
        let append: Arc<dyn AppendSentPort> = Arc::new(LiveSentAppender { deps: deps.clone(), runtime: runtime.clone() });
        let headers: Arc<dyn ParentHeadersPort> = Arc::new(LiveParentHeaders { deps: deps.clone(), runtime: runtime.clone() });
        // delta 22: a draft's files are read when its message is built (draft_files.rs), the same bytes to SMTP and to the Sent copy
        let files: Arc<dyn outbox::AttachmentFilesPort> = Arc::new(crate::draft_files::DraftFiles { store: store.clone(), deps: deps.clone(), runtime });
        let accounts: Arc<dyn SendAccountPort> = Arc::new(StoreSendAccounts { store: store.clone(), keychain, endpoint, transport });
        let ports = Ports {
            save: rows.clone(), get: rows.clone(), list_open: rows.clone(), find_by_key: rows.clone(), pending_copies: rows.clone(), load_draft: rows.clone(), account_names: rows,
            reply_context: Arc::new(StoreReplyContext::new(store.clone(), headers)),
            sender: Arc::new(SendCrateSender::new(accounts, Arc::new(send::LoggedDelivery { log: smtp_log(&store) }), append.clone(), clock.clone()).with_files(files)),
            online: Arc::new(AlwaysOnline), append_sent: append, clock: clock.clone(), sync: None,
        };
        let cfg = OutboxConfig::new(DEVICE_ID, DeviceRole::Primary, contracts::constants::OUTBOX_LATE_AFTER_MS as i64)?;
        let outbox = Arc::new(Outbox::open(ports, cfg)?);
        outbox.subscribe(Arc::new(Forward(deps.sinks.clone())))?;
        Ok(Arc::new(MailOut { outbox, store, deps, clock, kicked: Mutex::new(false), wake: Condvar::new(), stop: AtomicBool::new(false) }))
    }

    /// One tick: what is due leaves, and every message that left joins its thread.
    pub fn tick(&self) {
        match self.outbox.tick() {
            Ok(report) => for id in report.delivered { self.join_sent(&id) },
            Err(e) => eprintln!("outbox: the tick failed: {e}"),
        }
    }

    /// `SendNow`: an open entry leaves at once (the outbox's retry is its one door to deliver an entry by id).
    pub fn send_now(&self, outbox_id: &str) -> Result<(), CoreError> {
        let view = self.outbox.get(outbox_id)?.ok_or_else(|| CoreError::NotFound { kind: "outbox entry".into(), id: outbox_id.to_string() })?;
        if matches!(view.entry.state, OutboxState::Sent { .. } | OutboxState::Cancelled) {
            return Err(CoreError::Invalid { field: "outbox".into(), reason_words: "this message is already sent or cancelled".into() });
        }
        self.outbox.retry(outbox_id)?;
        self.join_sent(outbox_id);
        Ok(())
    }

    /// The stored copy of a sent message, in its thread: `sent-<outbox id>`, from the account's own address (so `is_outgoing`), read,
    /// out of the inbox, in the Sent folder, dated the send. Idempotent (the same id is kept once). Its reply reminder follows it.
    pub fn join_sent(&self, outbox_id: &str) {
        let Ok(Some(view)) = self.outbox.get(outbox_id) else { return };
        let OutboxState::Sent { at_ms } = view.entry.state else { return };
        let draft = view.entry.draft.clone();
        let sent_id = format!("sent-{outbox_id}");
        let Some(account) = self.store.account_summaries().ok().and_then(|all| all.into_iter().find(|a| a.id == draft.from_account_id)) else { return };
        let thread = draft.reply_to_thread_id.clone().unwrap_or_else(|| sent_id.clone());
        // delta 22: the files the message carried (the rows its draft handed to the entry) are the sent copy's attachments
        let files = self.store.outbox_files(outbox_id).unwrap_or_default();
        let sent_folder = crate::writeback::role_folder(&self.deps, &account.id, sync_imap::FolderRole::Sent).unwrap_or_else(|| "Sent".to_string());
        let copy = NewMessage {
            message: Message {
                id: sent_id.clone(), thread_id: thread, account_id: account.id.clone(), sender_name: account.name.clone(), from_address: account.address.clone(),
                to: draft.to.clone(), subject: draft.subject.clone(), preview: draft.body.chars().take(160).collect(), received_at_ms: at_ms, read: true, archived: true,
                folder: sent_folder, attachment_names: files.iter().map(|f| f.file_name.clone()).collect(), login_code: None, trackers_blocked: 0, list_unsubscribe: vec![], list_unsubscribe_one_click: false,
                reply_to_name: None, cc: draft.cc.clone(), bcc: draft.bcc.clone(), is_outgoing: true,
            },
            remote_id: None, body_text: Some(draft.body.clone()), body_html: None, has_list_header: false, size_bytes: Some(draft.body.len() as i64), tab_id: None,
        };
        if let Err(e) = self.store.insert_messages(vec![copy]) { eprintln!("outbox {outbox_id}: the sent copy was not stored: {e}"); return; }
        crate::bodies::crawl::enqueue_for_store(&self.store, &[sent_id.clone()]); // index-queue: the copy's text is cut and embedded now, not at the next launch
        let kept: Vec<contracts::AttachmentFile> = files.iter().enumerate().map(|(i, f)| contracts::AttachmentFile {
            id: format!("{sent_id}/{}", i + 1), message_id: sent_id.clone(), file_name: f.file_name.clone(), mime_type: f.media_type.clone(), size_bytes: f.size_bytes, local_path: None, inline: false, content_id: None,
        }).collect();
        if let Err(e) = self.store.put_message_attachments(&sent_id, kept) { eprintln!("outbox {outbox_id}: the sent copy's files were not listed: {e}"); }
        if let Err(e) = crate::reminders::follow_sent(&self.store, &draft.id, &sent_id, self.clock.now_ms()) { eprintln!("outbox {outbox_id}: {e}"); }
        // a reply flips "unanswered" on the older messages of its thread: the label flow re-reads them
        self.deps.filing.expect(vec![sent_id]);
        self.deps.filing.kick();
    }

    pub fn open_entries(&self) -> Vec<OutboxEntry> { self.outbox.open_entries().unwrap_or_default() }

    /// fix-realcore round 15: `outbox/<id>/send_state`, what the reader's card and the Outbox row show for one entry, as identifiers and numbers
    /// (the shell words them): `{"state", "attempts", "next_try_ms", "stage", "words", "sent_at_ms"}`. `state` is queued (the undo window),
    /// scheduled, sending, retrying (a broken connection; `next_try_ms` the next automatic try), waiting (no network), failed (`stage` the step
    /// it broke at, or null when the server answered a code; `words` the failure in the server's words), sent or cancelled. None for an unknown id.
    pub fn send_state(&self, outbox_id: &str) -> Option<String> {
        let view = self.outbox.get(outbox_id).ok().flatten()?;
        let q = |s: &str| crate::reads::json_text(s);
        let opt_i = |v: Option<i64>| v.map_or("null".to_string(), |n| n.to_string());
        let (state, stage, words, sent_at) = match &view.entry.state {
            OutboxState::Undoable { .. } => ("queued", None, None, None),
            OutboxState::Scheduled { .. } | OutboxState::Late { .. } => ("scheduled", None, None, None),
            OutboxState::Sending => ("sending", None, None, None),
            OutboxState::Sent { at_ms } => ("sent", None, None, Some(*at_ms)),
            OutboxState::Cancelled => ("cancelled", None, None, None),
            OutboxState::WaitingForNetwork { .. } => ("waiting", None, None, None),
            OutboxState::Failed { failure, .. } => {
                let stage = failure.code.as_deref().and_then(|c| c.strip_prefix(send::smtp::NETWORK_CODE_PREFIX)).map(str::to_string);
                let state = if view.retry_at_ms.is_some() { "retrying" } else if failure.reason_words.contains("no network") { "waiting" } else { "failed" };
                (state, stage, Some(failure.reason_words.clone()), None)
            }
        };
        Some(format!(r#"{{"state":{},"attempts":{},"next_try_ms":{},"stage":{},"words":{},"sent_at_ms":{}}}"#,
            q(state), view.attempts, opt_i(view.retry_at_ms), stage.as_deref().map_or("null".to_string(), q), words.as_deref().map_or("null".to_string(), q), opt_i(sent_at)))
    }

    /// fix-realcore round 15: `outbox/open`, the open entries (not yet sent or cancelled), oldest first: `[{"id", "subject", "send_state"}]`, the last
    /// as `send_state` serves it. What the Outbox row lists and what a check finds an entry by.
    pub fn open_states(&self) -> String {
        let mut entries = self.open_entries();
        entries.sort_by_key(|e| e.created_at_ms);
        let rows: Vec<String> = entries.iter().map(|e| format!(r#"{{"id":{},"subject":{},"send_state":{}}}"#,
            crate::reads::json_text(&e.id), crate::reads::json_text(&e.draft.subject), self.send_state(&e.id).unwrap_or_else(|| "null".to_string()))).collect();
        format!("[{}]", rows.join(","))
    }

    pub fn kick(&self) {
        *self.kicked.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.wake.notify_all();
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        self.kick();
    }

    /// The timer: a tick every OUTBOX_TICK_MS while an entry is open, asleep otherwise until a send kicks it.
    pub fn start(self: &Arc<Self>) {
        let me = self.clone();
        let spawned = std::thread::Builder::new().name("whoosh-outbox".into()).spawn(move || {
            while !me.stop.load(Ordering::SeqCst) {
                let open = !me.open_entries().is_empty();
                if open { me.tick(); }
                let mut k = me.kicked.lock().unwrap_or_else(|e| e.into_inner());
                if !*k {
                    k = if open || !me.open_entries().is_empty() {
                        me.wake.wait_timeout(k, Duration::from_millis(OUTBOX_TICK_MS)).map(|(g, _)| g).unwrap_or_else(|e| e.into_inner().0)
                    } else {
                        me.wake.wait(k).unwrap_or_else(|e| e.into_inner())
                    };
                }
                *k = false;
            }
        });
        if let Err(e) = spawned { eprintln!("outbox: the timer thread did not start ({e}); queued mail leaves on SendNow only"); }
    }
}

/// The reply a quick reply makes: to the newest message of the thread that the account did not send (else the newest one's first
/// recipient), "Re: " before the thread's subject unless it already starts so.
pub fn quick_reply_draft(store: &Store, thread_or_message: &str, from: Option<&str>, body: String, draft_id: String) -> Result<Draft, CoreError> {
    let key = thread_or_message.to_string();
    let found: Option<(String, String)> = store.write(move |ctx| {
        Ok(ctx.tx.query_row("SELECT id, account_id FROM thread WHERE id = ?1", [&key], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).ok()
            .or_else(|| ctx.tx.query_row("SELECT thread_id, account_id FROM message WHERE id = ?1", [&key], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).ok()))
    }).map_err(|e| unexpected(e.to_string()))?;
    let (thread, owner) = found.ok_or_else(|| CoreError::NotFound { kind: "thread".into(), id: thread_or_message.to_string() })?;
    let account = from.map(str::to_string).unwrap_or(owner);
    let accounts = store.account_summaries().map_err(|e| unexpected(e.to_string()))?;
    let me = accounts.iter().find(|a| a.id == account).ok_or_else(|| CoreError::NotFound { kind: "account".into(), id: account.clone() })?.address.to_ascii_lowercase();
    let t = thread.clone();
    let (to, subject, answered): (Option<String>, String, Option<(String, String)>) = store.write(move |ctx| {
        let mut st = ctx.tx.prepare("SELECT from_address, to_json, subject, id FROM message WHERE thread_id = ?1 ORDER BY received_at_ms DESC, id DESC")?;
        let rows: Vec<(String, String, String, String)> = st.query_map([&t], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<Result<_, _>>()?;
        let subject = rows.first().map(|r| r.2.clone()).unwrap_or_default();
        let incoming = rows.iter().find(|r| r.0.to_ascii_lowercase() != me);
        let answered = incoming.map(|r| (r.3.clone(), r.0.clone()));
        let to = incoming.map(|r| r.0.clone())
            .or_else(|| rows.first().and_then(|r| r.1.trim_start_matches('[').split(',').next().map(|s| s.trim().trim_matches(|c| c == '"' || c == ']').to_string())).filter(|s| !s.is_empty()));
        Ok((to, subject, answered))
    }).map_err(|e| unexpected(e.to_string()))?;
    let to = to.ok_or_else(|| CoreError::Invalid { field: "thread_id".into(), reason_words: "nobody in this thread can be answered".into() })?;
    // fix-replydoors: a mail with a Reply-To is answered there (the same rule as Reply, compose_cmds.rs); none stored: the sender, as before.
    // Two local store reads, no network. A read that fails leaves today's behaviour.
    let to: Vec<String> = match answered {
        Some((id, from)) => {
            let reply_to = store.reply_to(&id).unwrap_or_default();
            if reply_to.is_empty() { vec![to] } else {
                let own = store.own_addresses().unwrap_or_default();
                let r = crate::compose_cmds::recipients_for_reply(&from, &reply_to, &[], &own);
                if r.is_empty() { vec![to] } else { r }
            }
        }
        None => vec![to],
    };
    let subject = if subject.to_ascii_lowercase().starts_with("re:") { subject } else { format!("Re: {subject}") };
    Ok(Draft { id: draft_id, from_account_id: account, to, cc: vec![], subject, body, attachment_ids: vec![], ai_marks: vec![], reply_to_thread_id: Some(thread), bcc: vec![] })
}
