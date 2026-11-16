//! real-compose: the compose commands the real core did not serve (it answered "The local engine does not do this yet."):
//! `ReplyAll` (a draft that answers everyone, threaded under the message) and `SetSignature` (the account's signature, kept on this Mac),
//! and the reads the app makes for them: the account's signature (`signature(address)`, `AccountSummary.signature`) and the draft's
//! header fields (`draft/<id>/to`, `cc`, `subject`, `from_account_id`, `reply_to_thread_id`, `draft/count`).
//! Nothing here touches the network: a reply-all draft leaves through the same `Send` path every draft takes (outbox queue, undo window,
//! In-Reply-To and References from the stored headers), and the send crate appends the saved signature once.
//! real-replyto (migration 0027): a reply goes to the mail's Reply-To addresses when it has them, else to the sender (`recipients_for_reply`,
//! `recipients_for_reply_all`); the stored Reply-To is read from the store, never from the server.
use contracts::{Command, CommandOutcome, CoreError, Signature};
use store::Store;

use crate::reminders::store_failed;

/// The commands this file serves.
pub(crate) fn is_command(c: &Command) -> bool { matches!(c, Command::ReplyAll { .. } | Command::SetSignature { .. }) }

pub(crate) fn run(store: &Store, c: Command, offset_s: i32) -> Result<CommandOutcome, CoreError> {
    let now = crate::rust_core::now_ms();
    match c {
        Command::ReplyAll { message_id } => reply_all(store, &message_id, now, offset_s),
        Command::SetSignature { account_id, text } => set_signature(store, &account_id, &text, now),
        _ => Err(CoreError::Unexpected { reason_words: "this is not a compose command".into() }),   // `is_command` lets no other through
    }
}

/// `SetSignature`: the text is trimmed and an empty one clears the signature; an unknown account is `NotFound` and nothing is written.
fn set_signature(store: &Store, account_id: &str, text: &str, now_ms: i64) -> Result<CommandOutcome, CoreError> {
    let found = store.set_account_signature(account_id, text, now_ms).map_err(|e| store_failed("the signature could not be saved", e))?;
    if !found { return Err(CoreError::NotFound { kind: "account".into(), id: account_id.to_string() }); }
    Ok(CommandOutcome { undo_entry_id: None, created_id: None })
}

/// `ReplyAll`: a draft in the message's thread, "Re: " subject, the original quoted, To and Cc from the contract's `reply_all_recipients`
/// (the sender, then the other To addresses; the Cc; none of the owner's own addresses; each once, the case ignored; a message the owner
/// wrote goes to its original recipients). From is the account that holds the message. `created_id` is the draft. One transaction.
fn reply_all(store: &Store, message_id: &str, now_ms: i64, offset_s: i32) -> Result<CommandOutcome, CoreError> {
    let m = store.message(message_id).map_err(|e| store_failed("the message could not be read", e))?
        .ok_or_else(|| CoreError::NotFound { kind: "message".into(), id: message_id.to_string() })?;
    let own = store.own_addresses().map_err(|e| store_failed("the accounts could not be read", e))?;
    let reply_to = store.reply_to(message_id).map_err(|e| store_failed("the message's Reply-To could not be read", e))?;
    let r = recipients_for_reply_all(&m.from_address, &reply_to, &m.to, &m.cc, own);
    let text = store.body_parts(message_id).ok().flatten().and_then(|p| p.body_text.or_else(|| p.body_html.as_deref().map(parse::safe_html_to_text))).unwrap_or_else(|| m.preview.clone());
    let body = contracts::quoted_reply_body(m.sender_name.clone(), when_words(m.received_at_ms, offset_s), text);
    let subject = contracts::prefixed_subject(m.subject.clone(), "Re".into());
    let draft_id = store::new_id("draft");
    let (id, account, thread) = (draft_id.clone(), m.account_id.clone(), m.thread_id.clone());
    let (to_json, cc_json) = (json_list(&r.to), json_list(&r.cc));
    store.write(move |ctx| {
        ctx.tx.execute(
            "INSERT INTO draft (id, from_account_id, to_json, cc_json, subject, body, reply_to_thread_id, updated_at_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![id, account, to_json, cc_json, subject, body, thread, now_ms],
        )?;
        Ok(())
    }).map_err(|e| store_failed("the reply draft was not saved", e))?;
    Ok(CommandOutcome { undo_entry_id: None, created_id: Some(draft_id) })
}

fn same(a: &str, b: &str) -> bool { a.trim().eq_ignore_ascii_case(b.trim()) }

/// Each address once (the case ignored), none of the owner's own, none blank, trimmed, in the order given.
fn unique_not_own(addresses: impl IntoIterator<Item = String>, own: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for a in addresses {
        if a.trim().is_empty() || own.iter().any(|o| same(o, &a)) || out.iter().any(|x| same(x, &a)) { continue; }
        out.push(a.trim().to_string());
    }
    out
}

/// Where a reply goes: the Reply-To addresses when the mail has them (the owner's own left out), else the sender. A Reply-To that holds only
/// the owner's own address says nothing, so the sender gets the reply; a mail the owner wrote (its sender is the owner, and no other Reply-To)
/// is answered to its original recipients.
pub(crate) fn recipients_for_reply(from: &str, reply_to: &[store::ReplyTo], to: &[String], own: &[String]) -> Vec<String> {
    let asked = unique_not_own(reply_to.iter().map(|r| r.address.clone()), own);
    if !asked.is_empty() { return asked; }
    let sender = unique_not_own([from.to_string()], own);
    if !sender.is_empty() { return sender; }
    unique_not_own(to.iter().cloned(), own)
}

/// Reply all: a mail with a Reply-To goes To the Reply-To addresses and copies every other To and Cc address of the original (none of the
/// owner's own, no address twice, none already in To). A mail with no Reply-To follows the contract's `reply_all_recipients` unchanged
/// (the sender, then the other To addresses; the Cc), which the test engine also uses.
pub(crate) fn recipients_for_reply_all(from: &str, reply_to: &[store::ReplyTo], to: &[String], cc: &[String], own: Vec<String>) -> contracts::ReplyRecipients {
    let asked = unique_not_own(reply_to.iter().map(|r| r.address.clone()), &own);
    if asked.is_empty() { return contracts::reply_all_recipients(from.to_string(), to.to_vec(), cc.to_vec(), own); }
    let others = unique_not_own(to.iter().chain(cc.iter()).cloned(), &own);
    let cc: Vec<String> = others.into_iter().filter(|a| !asked.iter().any(|x| same(x, a))).collect();
    contracts::ReplyRecipients { to: asked, cc }
}

/// A JSON list of strings (the engine crate links no JSON library outside its tests).
fn json_list(v: &[String]) -> String {
    fn quote(s: &String) -> String {
        let mut o = String::from("\"");
        for c in s.chars() {
            match c {
                '"' => o.push_str("\\\""),
                '\\' => o.push_str("\\\\"),
                '\n' => o.push_str("\\n"),
                '\r' => o.push_str("\\r"),
                '\t' => o.push_str("\\t"),
                c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
                c => o.push(c),
            }
        }
        o.push('"');
        o
    }
    format!("[{}]", v.iter().map(quote).collect::<Vec<_>>().join(","))
}

/// "Sat 7 Nov, 10:00" in this Mac's time, as the forward header writes it (draft_files.rs keeps the same function private).
fn when_words(ms: i64, offset_s: i32) -> String {
    let secs = ms.div_euclid(1000) + offset_s as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    const DAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    format!("{} {} {}, {:02}:{:02}", DAYS[days.rem_euclid(7) as usize], day, MONTHS[(month - 1) as usize], rem / 3600, (rem % 3600) / 60)
}

/// The trait's `signature(address)`: the saved text of that address (any case), None when there is none.
pub(crate) fn signature(store: &Store, address: &str) -> Option<Signature> { store.signature(address).ok().flatten() }

/// `draft/<id>/from_account_id`, `to`, `cc`, `subject`, `reply_to_thread_id` (JSON, as the test engine's state answers) and `draft/count`.
/// `draft/<id>/body` and `attachments` stay with draft_files.rs. Every other path: None.
pub(crate) fn state(store: &Store, path: &str) -> Option<String> {
    use crate::reads::json_text;
    match path.split('/').collect::<Vec<_>>().as_slice() {
        // real-replyto: the Reply-To the store kept (JSON [{"name","address"}], header order; "[]" when none) and where a plain Reply goes
        ["message", id, "reply_to"] => {
            store.message(id).ok().flatten()?;
            let items = store.reply_to(id).ok()?;
            let parts: Vec<String> = items.iter().map(|r| format!("{{\"name\":{},\"address\":{}}}", json_text(&r.name), json_text(&r.address))).collect();
            Some(format!("[{}]", parts.join(",")))
        }
        ["message", id, "reply_recipients"] => {
            let m = store.message(id).ok().flatten()?;
            let own = store.own_addresses().ok()?;
            let reply_to = store.reply_to(id).ok()?;
            Some(json_list(&recipients_for_reply(&m.from_address, &reply_to, &m.to, &own)))
        }
        ["draft", "count"] => store.draft_count().ok().map(|n| n.to_string()),
        ["draft", id, leaf @ ("from_account_id" | "to" | "cc" | "subject" | "reply_to_thread_id")] => {
            let (from, to, cc, subject, thread) = store.draft_fields(id).ok().flatten()?;
            match *leaf {
                "from_account_id" => Some(json_text(&from)),
                "to" => Some(json_list(&to)),
                "cc" => Some(json_list(&cc)),
                "subject" => Some(json_text(&subject)),
                _ => thread.map(|t| json_text(&t)),
            }
        }
        _ => None,
    }
}
