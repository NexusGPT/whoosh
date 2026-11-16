//! The JMAP adapter: open-source start is `jmap-client` (Stalwart Labs, Apache-2.0 OR MIT), which
//! speaks the protocol. This file adds what the crate does not have: the capability probe with the
//! fallback answer, the first-screen-before-history order, the Email-state cursor, and the
//! batch sizes derived from the server's own declared limit.
use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use contracts::{CoreError, SyncCursor};
use jmap_client::client::{Client, Credentials};
use jmap_client::core::error::MethodErrorType;
use jmap_client::email::query::{Comparator, Filter};
use jmap_client::mailbox::{Property as MailboxProperty, Role};
use jmap_client::Error;
use tokio::sync::Mutex;

use crate::config::{JmapAuth, JmapConfig};
use crate::errors::{failed, is_auth_refused, is_cannot_calculate_changes, map_error};
use crate::map::{properties, to_fetched};
use crate::port::*;

pub(crate) const MAIL_CAPABILITY: &str = "urn:ietf:params:jmap:mail";

pub struct JmapAdapter {
    pub(crate) client: Client,
    pub(crate) cfg: JmapConfig,
    /// The server-side id of the account that holds the mail.
    pub(crate) server_account: String,
    folders: Mutex<Option<HashMap<String, Folder>>>,
}

impl std::fmt::Debug for JmapAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JmapAdapter {{ account: {:?}, base_url: {:?}, server_account: {:?} }}", self.cfg.account_name, self.cfg.base_url, self.server_account)
    }
}

fn role_word(role: Role) -> Option<String> {
    match role {
        Role::Archive => Some("archive".into()),
        Role::Drafts => Some("drafts".into()),
        Role::Important => Some("important".into()),
        Role::Inbox => Some("inbox".into()),
        Role::Junk => Some("junk".into()),
        Role::Sent => Some("sent".into()),
        Role::Trash => Some("trash".into()),
        Role::Other(s) => Some(s),
        Role::None => None,
    }
}

fn refused(e: &(dyn std::error::Error + 'static)) -> bool {
    let mut cur: Option<&(dyn std::error::Error + 'static)> = Some(e);
    while let Some(c) = cur {
        if let Some(io) = c.downcast_ref::<std::io::Error>() {
            if io.kind() == std::io::ErrorKind::ConnectionRefused {
                return true;
            }
        }
        cur = c.source();
    }
    false
}

fn fallback<A>(detail: String) -> Discovery<A> {
    Discovery::FallbackToImap { reason_words: format!("{FALLBACK_WORDS} ({detail})") }
}

impl JmapAdapter {
    /// Ask the host whether it offers JMAP, and sign in when it does.
    ///
    /// * JMAP session read and it holds mail: `Discovery::Jmap`.
    /// * The host has no JMAP (nothing listens, a 4xx other than a refused sign-in, an answer that
    ///   is not a JMAP session, a session without mail, a redirect to a host not trusted):
    ///   `Discovery::FallbackToImap`.
    /// * The sign-in is refused, or the host is unreachable or broken right now: `Err`, because an
    ///   IMAP fallback would not fix it and the owner must read the real reason.
    pub async fn connect(cfg: JmapConfig) -> Result<Discovery<JmapAdapter>, CoreError> {
        netguard::outbound_allowed_url(&cfg.base_url)?;   // jmap-guard: a rig copy dials this Mac only, refused before any lookup or connection
        let credentials = match &cfg.auth {
            JmapAuth::Bearer(t) => Credentials::bearer(t.clone()),
            JmapAuth::Basic { user, password } => Credentials::basic(user, password),
        };
        let connected = Client::new()
            .credentials(credentials)
            .timeout(cfg.request_timeout)
            .follow_redirects(cfg.redirect_hosts.iter().cloned())
            .connect(&cfg.base_url)
            .await;
        let mut client = match connected {
            Ok(c) => c,
            Err(e) if is_auth_refused(&e) => return Err(map_error(&cfg.account_name, e)),
            Err(Error::Transport(t)) if t.is_redirect() || refused(&t) => {
                return Ok(fallback(format!("{} did not answer a JMAP session request: {t}", cfg.base_url)));
            }
            Err(Error::Server(s)) if s.starts_with('4') => {
                return Ok(fallback(format!("{} answered {s} to the JMAP session request", cfg.base_url)));
            }
            Err(Error::Problem(p)) if matches!(p.status, Some(s) if (400..500).contains(&s)) => {
                return Ok(fallback(format!("{} refused the JMAP session request: {p}", cfg.base_url)));
            }
            Err(Error::Parse(p)) => {
                return Ok(fallback(format!("{} answered something that is not a JMAP session: {p}", cfg.base_url)));
            }
            Err(e) => return Err(map_error(&cfg.account_name, e)),
        };
        let session = client.session();
        let mail_account = session
            .primary_accounts()
            .find(|(capability, _)| capability.as_str() == MAIL_CAPABILITY)
            .map(|(_, account)| account.clone());
        let Some(server_account) = mail_account.filter(|_| session.has_capability(MAIL_CAPABILITY)) else {
            return Ok(fallback(format!("{} offers JMAP but no mail account", cfg.base_url)));
        };
        client.set_default_account_id(server_account.clone());
        Ok(Discovery::Jmap(JmapAdapter { client, cfg, server_account, folders: Mutex::new(None) }))
    }

    pub(crate) fn err(&self, e: Error) -> CoreError {
        map_error(&self.cfg.account_name, e)
    }

    pub(crate) fn words(&self, reason_words: String, retryable: bool) -> CoreError {
        failed(&self.cfg.account_name, reason_words, retryable)
    }

    /// The most objects one Email/get may ask for, from the server's own session document.
    fn get_cap(&self) -> usize {
        self.client.session().core_capabilities().map(|c| c.max_objects_in_get()).filter(|n| *n > 0).unwrap_or(usize::MAX)
    }

    fn state_of(&self, cursor: &SyncCursor) -> Result<String, CoreError> {
        match cursor {
            SyncCursor::Jmap { state } => Ok(state.clone()),
            other => Err(CoreError::Invalid { field: "cursor".into(), reason_words: format!("a JMAP account needs a JMAP cursor, got {other:?}") }),
        }
    }

    async fn folder_map(&self, refresh: bool) -> Result<HashMap<String, Folder>, CoreError> {
        if !refresh {
            if let Some(m) = self.folders.lock().await.as_ref() {
                return Ok(m.clone());
            }
        }
        let list = self.list_folders().await?;
        let map: HashMap<String, Folder> = list.into_iter().map(|f| (f.id.clone(), f)).collect();
        *self.folders.lock().await = Some(map.clone());
        Ok(map)
    }

    /// Fetch full records for `ids`, in chunks no bigger than the server's declared get limit.
    pub(crate) async fn fetch_by_ids(&self, ids: &[String], folders: &HashMap<String, Folder>, opts: FetchOptions) -> Result<(Vec<FetchedMessage>, Option<String>), CoreError> {
        let mut out = Vec::with_capacity(ids.len());
        let mut state = None;
        for chunk in ids.chunks(self.get_cap().max(1)) {
            let mut request = self.client.build();
            request.get_email().ids(chunk.iter().map(String::as_str)).properties(properties());
            let mut got = request.send_get_email().await.map_err(|e| self.err(e))?;
            state = Some(got.take_state());
            let mut by_id: HashMap<String, FetchedMessage> = got
                .take_list()
                .into_iter()
                .filter_map(|e| to_fetched(&self.cfg.account_id, folders, e, opts))
                .map(|m| (m.server_id.clone(), m))
                .collect();
            // The server may answer in any order; the caller's order is the order of `chunk`.
            out.extend(chunk.iter().filter_map(|id| by_id.remove(id)));
        }
        Ok((out, state))
    }

    /// One Email/query in the mailbox (newest first) plus the Email/get of its ids, in one request.
    async fn query_page(&self, mailbox_id: &str, anchor: Option<&str>, limit: usize, opts: FetchOptions) -> Result<(Vec<FetchedMessage>, Option<String>, bool), CoreError> {
        let folders = self.folder_map(false).await?;
        let limit = limit.min(self.get_cap()).max(1);
        let mut request = self.client.build();
        let query = request.query_email();
        query
            .filter(Filter::in_mailbox(mailbox_id))
            .sort([Comparator::received_at().descending()])
            .limit(limit)
            .calculate_total(true);
        if let Some(a) = anchor {
            query.anchor(a).anchor_offset(1);
        }
        let reference = query.result_reference();
        request.get_email().ids_ref(reference).properties(properties());
        let mut responses = request.send().await.map_err(|e| self.err(e))?.unwrap_method_responses();
        let mut got = responses.pop().ok_or_else(|| self.words("the server sent no answer to the message request".into(), true))?.unwrap_get_email().map_err(|e| self.err(e))?;
        let mut queried = responses.pop().ok_or_else(|| self.words("the server sent no answer to the list request".into(), true))?.unwrap_query_email().map_err(|e| self.err(e))?;
        let state = got.take_state();
        let ids = queried.take_ids();
        let reached_end = match queried.total() {
            Some(total) => (queried.position().max(0) as usize).saturating_add(ids.len()) >= total,
            None => ids.is_empty(),
        };
        let mut by_id: HashMap<String, FetchedMessage> = got
            .take_list()
            .into_iter()
            .filter_map(|e| to_fetched(&self.cfg.account_id, &folders, e, opts))
            .map(|m| (m.server_id.clone(), m))
            .collect();
        let ordered: Vec<FetchedMessage> = ids.iter().filter_map(|id| by_id.remove(id)).collect();
        Ok((ordered, Some(state), reached_end))
    }
}

#[async_trait]
impl JmapMailAdapter for JmapAdapter {
    async fn list_folders(&self) -> Result<Vec<Folder>, CoreError> {
        let mut request = self.client.build();
        request.get_mailbox().properties([
            MailboxProperty::Id,
            MailboxProperty::Name,
            MailboxProperty::ParentId,
            MailboxProperty::Role,
            MailboxProperty::TotalEmails,
            MailboxProperty::UnreadEmails,
        ]);
        let mut got = request.send_get_mailbox().await.map_err(|e| self.err(e))?;
        let folders = got
            .take_list()
            .into_iter()
            .map(|mut m| Folder {
                id: m.take_id(),
                name: m.name().unwrap_or_default().to_string(),
                parent_id: m.parent_id().map(str::to_string),
                role: role_word(m.role()),
                total: m.total_emails() as u64,
                unread: m.unread_emails() as u64,
            })
            .collect::<Vec<_>>();
        *self.folders.lock().await = Some(folders.iter().map(|f| (f.id.clone(), f.clone())).collect());
        Ok(folders)
    }

    async fn first_screen(&self, mailbox_id: &str, first_screen: usize, opts: FetchOptions) -> Result<Delivery, CoreError> {
        if first_screen == 0 {
            return Err(CoreError::Invalid { field: "first_screen".into(), reason_words: "the first screen must hold at least one message".into() });
        }
        self.folder_map(true).await?;
        let (messages, state, _) = self.query_page(mailbox_id, None, first_screen, opts).await?;
        let cursor = state.map(|state| SyncCursor::Jmap { state });
        Ok(Delivery { changed: messages, vanished: vec![], cursor })
    }

    async fn history_page(&self, mailbox_id: &str, after_id: Option<&str>, limit: usize, opts: FetchOptions) -> Result<HistoryPage, CoreError> {
        if limit == 0 {
            return Err(CoreError::Invalid { field: "limit".into(), reason_words: "a history page must hold at least one message".into() });
        }
        let (messages, _, done) = self.query_page(mailbox_id, after_id, limit, opts).await.map_err(|e| match e {
            // The anchor message was destroyed between two pages.
            CoreError::Action { failure } if failure.reason_words.contains("anchorNotFound") || failure.reason_words.contains("Anchor not found") => {
                self.words("the history position moved on the server; start the history again from the newest message".into(), true)
            }
            other => other,
        })?;
        let last_id = messages.last().map(|m| m.server_id.clone());
        Ok(HistoryPage { done: done || messages.is_empty(), last_id, messages })
    }

    async fn initial_sync(&self, mailbox_id: &str, first_screen: usize, history_page: usize, opts: FetchOptions, sink: &mut dyn MessageSink) -> Result<SyncCursor, CoreError> {
        let first = self.first_screen(mailbox_id, first_screen, opts).await?;
        let cursor = first.cursor.clone().ok_or_else(|| self.words("the server gave no state for the first screen".into(), true))?;
        let mut after = first.changed.last().map(|m| m.server_id.clone());
        let nothing_more = first.changed.is_empty();
        // The first screen reaches the core BEFORE any older message is requested.
        sink.deliver(first).await?;
        if nothing_more {
            return Ok(cursor);
        }
        loop {
            let page = self.history_page(mailbox_id, after.as_deref(), history_page, opts).await?;
            if page.messages.is_empty() {
                break;
            }
            after = page.last_id.clone();
            let done = page.done;
            sink.deliver(Delivery { changed: page.messages, vanished: vec![], cursor: None }).await?;
            if done {
                break;
            }
        }
        Ok(cursor)
    }

    async fn resync(&self, cursor: &SyncCursor, opts: FetchOptions) -> Result<Resync, CoreError> {
        let mut since = self.state_of(cursor)?;
        let folders = self.folder_map(true).await?;
        let mut changed_ids: Vec<String> = Vec::new();
        let mut destroyed: Vec<String> = Vec::new();
        loop {
            let mut changes = match self.client.email_changes(since.clone(), None).await {
                Ok(c) => c,
                Err(e) if is_cannot_calculate_changes(&e) => {
                    return Ok(Resync::Reset { reason_words: "the server can no longer list what changed since the stored position".into() });
                }
                Err(e) => return Err(self.err(e)),
            };
            changed_ids.extend(changes.take_created());
            changed_ids.extend(changes.take_updated());
            destroyed.extend(changes.take_destroyed());
            let more = changes.has_more_changes();
            since = changes.take_new_state();
            if !more {
                break;
            }
        }
        let gone: HashSet<&String> = destroyed.iter().collect();
        let mut seen = HashSet::new();
        let ids: Vec<String> = changed_ids.into_iter().filter(|id| !gone.contains(id) && seen.insert(id.clone())).collect();
        let (changed, _) = self.fetch_by_ids(&ids, &folders, opts).await?;
        Ok(Resync::Delta(Delivery { changed, vanished: destroyed, cursor: Some(SyncCursor::Jmap { state: since }) }))
    }
}

#[allow(dead_code)]
fn _anchor_not_found_exists(t: MethodErrorType) -> bool {
    matches!(t, MethodErrorType::AnchorNotFound)
}
