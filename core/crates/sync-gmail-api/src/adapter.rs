use crate::error::GmailError;
use crate::limits::*;
use crate::model::*;
use crate::quota::QuotaGate;
use crate::transport::{FeatureFlags, Method, Pacer, Request, Response, TokenSource, Transport};
use crate::wire;
use base64::Engine;
use contracts::SyncCursor;
use serde::de::DeserializeOwned;
use std::sync::Mutex;

/// The flag that switches this adapter on (Google's restricted-scope verification passed).
pub const FLAG_NAME: &str = "gmail_api_verified";

const WINDOW_MS: i64 = 60_000;
const HOUR_MS: i64 = 3_600_000;

/// Everything the adapter is built from. Made only when the flag is on.
pub struct Parts {
    pub transport: Box<dyn Transport>,
    pub tokens: Box<dyn TokenSource>,
    pub pacer: Box<dyn Pacer>,
    pub account_name: String,
}

pub struct GmailApi {
    parts: Parts,
    gate: Mutex<QuotaGate>,
}

impl GmailApi {
    /// Builds the adapter only when `gmail_api_verified` is on. With the flag off, `build` is never called,
    /// so no transport, token source or pacer is made and no request can be sent.
    pub fn open(flags: &dyn FeatureFlags, build: impl FnOnce() -> Parts) -> Result<GmailApi, GmailError> {
        if !flags.is_on(FLAG_NAME) {
            return Err(GmailError::FlagOff);
        }
        Ok(GmailApi { parts: build(), gate: Mutex::new(QuotaGate::new(GMAIL_UNITS_PER_MIN, WINDOW_MS)) })
    }

    pub fn account_name(&self) -> &str {
        &self.parts.account_name
    }

    // ---- one call: pace, send, classify ----

    fn pace(&self, cost: u64) {
        let mut gate = self.gate.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            let now = self.parts.pacer.now_ms();
            let wait = gate.wait_needed(now, cost);
            if wait == 0 {
                gate.record(now, cost);
                return;
            }
            self.parts.pacer.sleep_ms(wait);
        }
    }

    fn call(&self, cost: u64, method: Method, path: &str, query: Vec<(String, String)>, body: Option<String>) -> Result<Response, GmailError> {
        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            self.pace(cost);
            let bearer = self.parts.tokens.bearer().map_err(|m| GmailError::AuthExpired { message: m })?;
            let request = Request { method, path: path.to_string(), query: query.clone(), bearer, json_body: body.clone() };
            let response = self.parts.transport.send(&request).map_err(|m| GmailError::Network { message: m })?;
            match response.status {
                200..=299 => return Ok(response),
                401 => return Err(GmailError::AuthExpired { message: error_message(&response.body) }),
                429 | 403 if is_rate_limited(&response) => {
                    if attempt as u64 >= GMAIL_RETRY_MAX {
                        return Err(GmailError::RateLimited { message: error_message(&response.body), attempts: attempt });
                    }
                    let backoff = response
                        .retry_after_ms
                        .unwrap_or((GMAIL_BACKOFF_BASE_MS << (attempt - 1)) as i64);
                    self.parts.pacer.sleep_ms(backoff);
                }
                status => return Err(GmailError::Http { status, message: error_message(&response.body) }),
            }
        }
    }

    fn get<T: DeserializeOwned>(&self, cost: u64, path: &str, query: Vec<(String, String)>, what: &str) -> Result<T, GmailError> {
        let response = self.call(cost, Method::Get, path, query, None)?;
        decode(&response.body, what)
    }

    // ---- labels ----

    pub fn labels(&self) -> Result<Vec<Label>, GmailError> {
        let list: wire::LabelList = self.get(GMAIL_COST_LABELS_LIST, "labels", vec![], "the label list")?;
        Ok(list
            .labels
            .into_iter()
            .map(|l| Label {
                kind: if l.kind.as_deref() == Some("system") { LabelKind::System } else { LabelKind::User },
                id: l.id,
                name: l.name,
            })
            .collect())
    }

    // ---- watch ----

    /// users.watch on the caller's Pub/Sub topic. Call again when `watch_renew_due` says so.
    pub fn start_watch(&self, topic_name: &str, label_ids: &[String]) -> Result<Watch, GmailError> {
        let body = serde_json::json!({ "topicName": topic_name, "labelIds": label_ids, "labelFilterBehavior": "INCLUDE" });
        let started = self.parts.pacer.now_ms();
        let response = self.call(GMAIL_COST_WATCH, Method::Post, "watch", vec![], Some(body.to_string()))?;
        let reply: wire::WatchReply = decode(&response.body, "the watch reply")?;
        let expires_at_ms: i64 = reply
            .expiration
            .parse()
            .map_err(|_| GmailError::Decode { what: "the watch expiration".into(), reason: format!("not a number: {}", reply.expiration) })?;
        let renew_at_ms = (started + GMAIL_WATCH_RENEW_H as i64 * HOUR_MS).min(expires_at_ms);
        Ok(Watch { history_id: reply.history_id, expires_at_ms, renew_at_ms })
    }

    pub fn watch_renew_due(&self, watch: &Watch) -> bool {
        self.parts.pacer.now_ms() >= watch.renew_at_ms
    }

    // ---- sync ----

    /// Brings the local mirror up to date. `None` or an expired start point does a full resync.
    /// Batches go to `sink` as they are read; the returned cursor is stored by the caller only after this returns Ok.
    pub fn sync(&self, from: Option<&SyncCursor>, sink: &mut dyn ChangeSink) -> Result<SyncSummary, GmailError> {
        match from {
            None => self.full_resync(ResyncReason::NoCursor, sink),
            Some(SyncCursor::GmailHistory { history_id }) => match self.incremental(history_id, sink) {
                Err(GmailError::HistoryExpired) => self.full_resync(ResyncReason::HistoryExpired, sink),
                other => other,
            },
            Some(_) => Err(GmailError::Invalid { field: "cursor".into(), reason: "this is not a Gmail history cursor".into() }),
        }
    }

    fn incremental(&self, start_history_id: &str, sink: &mut dyn ChangeSink) -> Result<SyncSummary, GmailError> {
        // 1. Read every history page first (ids only). A 404 can only happen here, before anything reached the sink.
        let mut records: Vec<wire::HistoryRecord> = Vec::new();
        let mut latest = start_history_id.to_string();
        let mut page_token: Option<String> = None;
        loop {
            let mut query = vec![
                ("startHistoryId".to_string(), start_history_id.to_string()),
                ("maxResults".to_string(), GMAIL_PAGE_SIZE.to_string()),
            ];
            for kind in ["messageAdded", "messageDeleted", "labelAdded", "labelRemoved"] {
                query.push(("historyTypes".to_string(), kind.to_string()));
            }
            if let Some(token) = &page_token {
                query.push(("pageToken".to_string(), token.clone()));
            }
            let page: wire::HistoryList = match self.get(GMAIL_COST_HISTORY_LIST, "history", query, "the history page") {
                Err(GmailError::Http { status: 404, .. }) => return Err(GmailError::HistoryExpired),
                other => other?,
            };
            records.extend(page.history);
            if let Some(id) = page.history_id {
                latest = id;
            }
            match page.next_page_token {
                Some(t) => page_token = Some(t),
                None => break,
            }
        }

        // 2. Net the records into changes in order.
        let changes = collapse(records);

        // 3. Fetch the bodies of added messages only, then store: added batches first, other changes last.
        let added: Vec<(String, String, Vec<String>)> = changes
            .iter()
            .filter_map(|c| match c {
                Change::Added { message_id, thread_id, label_ids } => Some((message_id.clone(), thread_id.clone(), label_ids.clone())),
                _ => None,
            })
            .collect();
        let mut summary = SyncSummary {
            mode: SyncMode::Incremental,
            reason: None,
            cursor: SyncCursor::GmailHistory { history_id: latest },
            changes: 0,
            messages: 0,
            gone: 0,
        };
        for chunk in added.chunks(GMAIL_PAGE_SIZE as usize) {
            let ids: Vec<&str> = chunk.iter().map(|(id, _, _)| id.as_str()).collect();
            self.emit_added(&ids, sink, &mut summary)?;
        }
        let rest: Vec<Change> = changes.into_iter().filter(|c| !matches!(c, Change::Added { .. })).collect();
        if !rest.is_empty() {
            summary.changes += rest.len() as u64;
            sink.accept(Batch { changes: rest, messages: vec![] }).map_err(|m| GmailError::Sink { message: m })?;
        }
        Ok(summary)
    }

    fn full_resync(&self, reason: ResyncReason, sink: &mut dyn ChangeSink) -> Result<SyncSummary, GmailError> {
        // The mailbox position is read BEFORE the listing, so a message that arrives while we list is replayed by the next history call.
        let profile: wire::Profile = self.get(GMAIL_COST_PROFILE, "profile", vec![], "the profile")?;
        let mut summary = SyncSummary {
            mode: SyncMode::FullResync,
            reason: Some(reason),
            cursor: SyncCursor::GmailHistory { history_id: profile.history_id },
            changes: 0,
            messages: 0,
            gone: 0,
        };
        let mut page_token: Option<String> = None;
        loop {
            let mut query = vec![("maxResults".to_string(), GMAIL_PAGE_SIZE.to_string())];
            if let Some(token) = &page_token {
                query.push(("pageToken".to_string(), token.clone()));
            }
            let page: wire::MessageList = self.get(GMAIL_COST_MESSAGES_LIST, "messages", query, "the message list")?;
            let ids: Vec<&str> = page.messages.iter().map(|m| m.id.as_str()).collect();
            self.emit_added(&ids, sink, &mut summary)?;
            match page.next_page_token {
                Some(t) => page_token = Some(t),
                None => break,
            }
        }
        Ok(summary)
    }

    /// Fetches each id raw and hands one batch to the sink.
    fn emit_added(&self, ids: &[&str], sink: &mut dyn ChangeSink, summary: &mut SyncSummary) -> Result<(), GmailError> {
        let mut batch = Batch::default();
        for id in ids {
            let path = format!("messages/{id}");
            let query = vec![("format".to_string(), "raw".to_string())];
            let response = match self.call(GMAIL_COST_MESSAGES_GET_RAW, Method::Get, &path, query, None) {
                Err(GmailError::Http { status: 404, .. }) => {
                    summary.gone += 1;
                    continue;
                }
                other => other?,
            };
            let message: wire::WireMessage = decode(&response.body, "a message")?;
            let raw = message.raw.as_deref().ok_or_else(|| GmailError::Decode { what: "a message".into(), reason: format!("message {id} has no raw body") })?;
            let rfc822 = decode_base64url(raw)?;
            let thread_id = message.thread_id.unwrap_or_else(|| message.id.clone());
            let internal_date_ms = match message.internal_date.as_deref() {
                Some(s) => s.parse().map_err(|_| GmailError::Decode { what: "a message date".into(), reason: format!("not a number: {s}") })?,
                None => 0,
            };
            batch.changes.push(Change::Added { message_id: message.id.clone(), thread_id: thread_id.clone(), label_ids: message.label_ids.clone() });
            batch.messages.push(RawMessage {
                id: message.id,
                thread_id,
                label_ids: message.label_ids,
                history_id: message.history_id.unwrap_or_default(),
                internal_date_ms,
                size_estimate: message.size_estimate.unwrap_or(0),
                rfc822,
            });
        }
        if batch.messages.is_empty() {
            return Ok(());
        }
        summary.changes += batch.changes.len() as u64;
        summary.messages += batch.messages.len() as u64;
        sink.accept(batch).map_err(|m| GmailError::Sink { message: m })
    }
}

/// History records into ordered changes. A message added and then deleted inside the same range is never fetched.
fn collapse(records: Vec<wire::HistoryRecord>) -> Vec<Change> {
    let mut changes: Vec<Change> = Vec::new();
    for record in records {
        for e in record.messages_added {
            let m = e.message;
            let thread_id = m.thread_id.unwrap_or_else(|| m.id.clone());
            changes.push(Change::Added { message_id: m.id, thread_id, label_ids: m.label_ids });
        }
        for e in record.messages_deleted {
            let id = e.message.id;
            changes.retain(|c| !matches!(c, Change::Added { message_id, .. } if *message_id == id));
            changes.push(Change::Deleted { message_id: id });
        }
        for e in record.labels_added {
            changes.push(Change::LabelsChanged { message_id: e.message.id, added: e.label_ids, removed: vec![] });
        }
        for e in record.labels_removed {
            changes.push(Change::LabelsChanged { message_id: e.message.id, added: vec![], removed: e.label_ids });
        }
    }
    changes
}

fn decode<T: DeserializeOwned>(body: &str, what: &str) -> Result<T, GmailError> {
    serde_json::from_str(body).map_err(|e| GmailError::Decode { what: what.to_string(), reason: e.to_string() })
}

fn decode_base64url(raw: &str) -> Result<Vec<u8>, GmailError> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(raw.trim_end_matches('='))
        .map_err(|e| GmailError::Decode { what: "a raw message".into(), reason: e.to_string() })
}

/// The real message Google sent, never only the status.
fn error_message(body: &str) -> String {
    match serde_json::from_str::<wire::ApiErrorEnvelope>(body) {
        Ok(e) if !e.error.message.is_empty() => e.error.message,
        _ => body.chars().take(300).collect(),
    }
}

fn is_rate_limited(response: &Response) -> bool {
    if response.status == 429 {
        return true;
    }
    serde_json::from_str::<wire::ApiErrorEnvelope>(&response.body)
        .map(|e| e.error.errors.iter().any(|d| d.reason == "rateLimitExceeded" || d.reason == "userRateLimitExceeded"))
        .unwrap_or(false)
}
