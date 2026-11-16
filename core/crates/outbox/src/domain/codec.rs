//! One shape for a record outside memory: the `outbox_entry` columns. The store adapter writes them as
//! columns, the cloud adapter sends the same fields as JSON. The draft column holds the envelope
//! `{"scheduled": bool, "draft": {...}}`, the failure column holds `{..ActionFailed, "waiting": bool}`.
use super::{CopyPending, Record};
use contracts::{ActionFailed, AiMark, CoreError, Draft, OutboxEntry, OutboxState};
use send::{ReplyContext, ThreadKind};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct RowData {
    pub id: String,
    pub draft_json: String,
    pub state: String,
    pub due_at_ms: Option<i64>,
    pub sent_at_ms: Option<i64>,
    pub failure_json: Option<String>,
    pub attempts: i64,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

fn unreadable(what: &str, why: impl std::fmt::Display) -> CoreError {
    CoreError::Unexpected { reason_words: format!("an outbox record is unreadable ({what}): {why}") }
}

pub(crate) fn draft_to_value(d: &Draft) -> Value {
    json!({
        "id": d.id, "from_account_id": d.from_account_id, "to": d.to, "cc": d.cc, "bcc": d.bcc, "subject": d.subject, "body": d.body,
        "attachment_ids": d.attachment_ids,
        "ai_marks": d.ai_marks.iter().map(|m| json!({"start": m.start, "end": m.end, "words": m.words})).collect::<Vec<_>>(),
        "reply_to_thread_id": d.reply_to_thread_id,
    })
}

fn strings(v: &Value, key: &str) -> Result<Vec<String>, CoreError> {
    v.get(key).and_then(Value::as_array).ok_or_else(|| unreadable("draft", format!("{key} is not a list")))?
        .iter().map(|s| s.as_str().map(str::to_string).ok_or_else(|| unreadable("draft", format!("{key} holds a non-text item")))).collect()
}

fn text(v: &Value, key: &str) -> Result<String, CoreError> {
    v.get(key).and_then(Value::as_str).map(str::to_string).ok_or_else(|| unreadable("draft", format!("{key} is missing")))
}

pub(crate) fn draft_from_value(v: &Value) -> Result<Draft, CoreError> {
    let marks = v.get("ai_marks").and_then(Value::as_array).ok_or_else(|| unreadable("draft", "ai_marks is not a list"))?
        .iter().map(|m| Ok(AiMark {
            start: m.get("start").and_then(Value::as_u64).ok_or_else(|| unreadable("draft", "mark start"))? as u32,
            end: m.get("end").and_then(Value::as_u64).ok_or_else(|| unreadable("draft", "mark end"))? as u32,
            words: text(m, "words")?,
        })).collect::<Result<Vec<_>, CoreError>>()?;
    Ok(Draft {
        id: text(v, "id")?, from_account_id: text(v, "from_account_id")?, to: strings(v, "to")?, cc: strings(v, "cc")?,
        subject: text(v, "subject")?, body: text(v, "body")?, attachment_ids: strings(v, "attachment_ids")?, ai_marks: marks,
        reply_to_thread_id: v.get("reply_to_thread_id").and_then(Value::as_str).map(str::to_string),
        // delta 12; a record written before it has no bcc: none
        bcc: if v.get("bcc").is_some() { strings(v, "bcc")? } else { vec![] },
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Result<Vec<u8>, CoreError> {
    if s.len() % 2 != 0 || !s.is_ascii() {
        return Err(unreadable("copy_pending", "the saved copy is not hexadecimal text"));
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| unreadable("copy_pending", e))).collect()
}

fn threading_to_value(t: &Option<ReplyContext>) -> Value {
    match t {
        None => Value::Null,
        Some(c) => json!({
            "kind": match c.kind { ThreadKind::Reply => "reply", ThreadKind::Forward => "forward" },
            "parent_message_id": c.parent_message_id,
            "parent_references": c.parent_references,
        }),
    }
}

fn threading_from_value(v: Option<&Value>) -> Result<Option<ReplyContext>, CoreError> {
    let Some(v) = v.filter(|v| !v.is_null()) else { return Ok(None) };
    let kind = match v.get("kind").and_then(Value::as_str) {
        Some("reply") => ThreadKind::Reply,
        Some("forward") => ThreadKind::Forward,
        other => return Err(unreadable("threading", format!("unknown kind {other:?}"))),
    };
    Ok(Some(ReplyContext { kind, parent_message_id: text(v, "parent_message_id")?, parent_references: strings(v, "parent_references")? }))
}

pub fn to_row(r: &Record) -> RowData {
    let (state, sent_at_ms, failure_json) = match &r.entry.state {
        OutboxState::Undoable { .. } => ("undoable", None, None),
        OutboxState::Scheduled { .. } => ("scheduled", None, None),
        OutboxState::Sending => ("sending", None, None),
        OutboxState::Sent { at_ms } => ("sent", Some(*at_ms), None),
        OutboxState::Failed { failure, .. } => (
            "failed",
            None,
            Some(json!({"account_name": failure.account_name, "reason_words": failure.reason_words, "retryable": failure.retryable, "code": failure.code, "waiting": r.waiting, "retry_at_ms": r.retry_at_ms}).to_string()),
        ),
        OutboxState::Cancelled => ("cancelled", None, None),
        // root delta 4: Late is DERIVED at read (now - due > OUTBOX_LATE_AFTER_MS), so it is stored as the
        // scheduled row it is; WaitingForNetwork is stored as today's waiting failure. The table's CHECK
        // allows no other state, and a new stored value would be a migration.
        OutboxState::Late { .. } => ("scheduled", None, None),
        OutboxState::WaitingForNetwork { .. } => (
            "failed",
            None,
            Some(json!({"account_name": "", "reason_words": "waiting for the network", "retryable": true, "waiting": true}).to_string()),
        ),
    };
    RowData {
        id: r.entry.id.clone(),
        draft_json: json!({
            "scheduled": r.scheduled,
            "draft": draft_to_value(&r.entry.draft),
            "idempotency_key": r.idempotency_key,
            "threading": threading_to_value(&r.threading),
            "copy_pending": r.copy_pending.as_ref().map(|c| json!({"words": c.words, "rfc822_hex": hex(&c.rfc822)})),
        })
        .to_string(),
        state: state.to_string(),
        due_at_ms: Some(r.due_at_ms),
        sent_at_ms,
        failure_json,
        attempts: i64::from(r.attempts),
        created_at_ms: r.entry.created_at_ms,
        updated_at_ms: r.updated_at_ms,
    }
}

pub fn from_row(row: RowData) -> Result<Record, CoreError> {
    let env: Value = serde_json::from_str(&row.draft_json).map_err(|e| unreadable("draft", e))?;
    let scheduled = env.get("scheduled").and_then(Value::as_bool).ok_or_else(|| unreadable("draft", "scheduled flag is missing"))?;
    let draft = draft_from_value(env.get("draft").ok_or_else(|| unreadable("draft", "draft is missing"))?)?;
    // Rows written before the key existed take their own id as key: stable, and unique per entry.
    let idempotency_key = env.get("idempotency_key").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| row.id.clone());
    let threading = threading_from_value(env.get("threading"))?;
    let copy_pending = match env.get("copy_pending").filter(|v| !v.is_null()) {
        None => None,
        Some(c) => Some(CopyPending { words: text(c, "words")?, rfc822: unhex(&text(c, "rfc822_hex")?)? }),
    };
    let due = row.due_at_ms.ok_or_else(|| unreadable("state", "no due time"))?;
    let attempts = u32::try_from(row.attempts).map_err(|_| unreadable("attempts", row.attempts))?;
    let mut waiting = false;
    let mut retry_at_ms = None;
    let state = match row.state.as_str() {
        "undoable" => OutboxState::Undoable { until_ms: due },
        "scheduled" => OutboxState::Scheduled { at_ms: due },
        "sending" => OutboxState::Sending,
        "sent" => OutboxState::Sent { at_ms: row.sent_at_ms.ok_or_else(|| unreadable("state", "sent without a time"))? },
        "cancelled" => OutboxState::Cancelled,
        "failed" => {
            let f: Value = serde_json::from_str(row.failure_json.as_deref().ok_or_else(|| unreadable("failure", "missing"))?).map_err(|e| unreadable("failure", e))?;
            waiting = f.get("waiting").and_then(Value::as_bool).unwrap_or(false);
            // fix-realcore round 15; a row written before it has no retry time: it waits for the owner's Retry, as before
            retry_at_ms = f.get("retry_at_ms").and_then(Value::as_i64);
            OutboxState::Failed {
                failure: ActionFailed {
                    account_name: text(&f, "account_name")?,
                    reason_words: text(&f, "reason_words")?,
                    retryable: f.get("retryable").and_then(Value::as_bool).unwrap_or(false),
                    // delta 12; a row written before it has no code: None
                    code: f.get("code").and_then(Value::as_str).map(str::to_string),
                },
                attempts,
            }
        }
        other => return Err(unreadable("state", other)),
    };
    Ok(Record { entry: OutboxEntry { id: row.id, draft, state, created_at_ms: row.created_at_ms }, due_at_ms: due, scheduled, attempts, waiting, updated_at_ms: row.updated_at_ms, idempotency_key, threading, copy_pending, retry_at_ms })
}

/// The row as the other devices get it: without the pending Sent copy. That copy belongs to the device that
/// sent the message; two devices saving it would put two copies in Sent.
pub fn to_wire_row(r: &Record) -> RowData {
    let mut local = r.clone();
    local.copy_pending = None;
    to_row(&local)
}

/// The sync payload: the row as JSON bytes.
pub fn row_to_wire(row: &RowData) -> Vec<u8> {
    json!({
        "id": row.id, "draft_json": row.draft_json, "state": row.state, "due_at_ms": row.due_at_ms, "sent_at_ms": row.sent_at_ms,
        "failure_json": row.failure_json, "attempts": row.attempts, "created_at_ms": row.created_at_ms, "updated_at_ms": row.updated_at_ms,
    })
    .to_string()
    .into_bytes()
}

pub fn wire_to_row(bytes: &[u8]) -> Result<RowData, CoreError> {
    let v: Value = serde_json::from_slice(bytes).map_err(|e| unreadable("sync payload", e))?;
    let req_text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string).ok_or_else(|| unreadable("sync payload", format!("{k} is missing")));
    let req_int = |k: &str| v.get(k).and_then(Value::as_i64).ok_or_else(|| unreadable("sync payload", format!("{k} is missing")));
    Ok(RowData {
        id: req_text("id")?, draft_json: req_text("draft_json")?, state: req_text("state")?,
        due_at_ms: v.get("due_at_ms").and_then(Value::as_i64), sent_at_ms: v.get("sent_at_ms").and_then(Value::as_i64),
        failure_json: v.get("failure_json").and_then(Value::as_str).map(str::to_string),
        attempts: req_int("attempts")?, created_at_ms: req_int("created_at_ms")?, updated_at_ms: req_int("updated_at_ms")?,
    })
}
