//! Engine-actions: `Unsubscribe { message_id }` in the real core: leave the mailing list of a message by the header the store kept
//! (`list_unsubscribe_json`, `list_unsubscribe_one_click`). The method is the contract's `unsubscribe_method_from`; the refusals are
//! the contract's `check_unsubscribe` (no header: "This message is not from a mailing list."; done before: "You already left this
//! list."), the same two the fixture core raises.
//!   - one-click (RFC 8058): ONE POST of `List-Unsubscribe=One-Click`, `Content-Type: application/x-www-form-urlencoded`, to the
//!     header's https URL; no redirect is followed; a 2xx marks the list left, anything else is refused in the server's words and
//!     marks nothing (the owner may try again);
//!   - mail link: ONE outbox entry from the message's account to the address, with the subject the header gives (else "Unsubscribe"),
//!     queued with a zero undo window so the outbox's next tick sends it; the list is marked left once the entry is stored;
//!   - web page: the core sends nothing; the UI opens the page after the owner's confirm, and the list is marked left (as the fixture does).
//! Clear HTTP is used only for this Mac's own address and only by a core opened with `TestTransport::PlainLoopback` (the local fake
//! of the tests): the same rule as the sign-in (`sign_in::authority::allowed`).
//! State reads: `message/<id>/unsubscribe` (JSON string: "none", "one_click", "mail", "link", "done"), `message/<id>/unsubscribed`
//! ("true" or "false") and `message/<id>/unsubscribe_method` ("one_click", "mail", "link"; absent when the message has no header), and
//! (engine-act-r6 round 7) `message/<id>/unsubscribed_at` (when the list was left, milliseconds; absent while it was not).
//! engine-act-r6 round 7: a list left is left for the whole THREAD (and for any message of the account carrying the same header,
//! `Store::list_left_at`): the next issue of a newsletter in the same thread says "You already left this list." and posts nothing, and
//! every message of it reads `unsubscribed` true with the same date. Reads never send anything: only the confirmed `Unsubscribe` does
//! (the screen shows the method from `unsubscribe_method` and asks the owner first, ReaderListActions.swift).
use std::sync::Arc;
use std::time::Duration;

use contracts::{
    check_unsubscribe, unsubscribe_method_from, unsubscribe_state, unsubscribe_state_key, ActionFailed, CommandOutcome, CoreError, Draft, UnsubscribeMethod,
};
use store::Store;

use crate::consts::OAUTH_HTTP_TIMEOUT_MS;
use crate::reads::json_text;
use crate::reminders::store_failed;
use crate::sync::is_loopback;

/// engine-act-r6 round 7: one unsubscribe at a time. The check "already left", the POST and the mark are one step: two presses (or two windows)
/// reaching the core while the first POST is on the wire would both pass the check and post twice.
static UNSUBSCRIBE_ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

const ONE_CLICK_BODY: &str = "List-Unsubscribe=One-Click";
const DEFAULT_SUBJECT: &str = "Unsubscribe";

/// The host of a URL without its port ("127.0.0.1" for "https://127.0.0.1:8080/u"): rig-net-rust, the guard's one parser (the client's),
/// so the loopback test below and the POST agree on the host ("" when the client could not parse the URL).
fn host_name(url: &str) -> String { netguard::host_of_url(url) }

/// A plain-loopback core reads `http://127.0.0.1:<port>/..` in a header as the https URL the contract's rule needs: the one place a
/// test's local fake can be a one-click target. Any other URL, and any core but a plain-loopback one, is left exactly as the header has it.
fn as_header_url(url: &str, plain_loopback: bool) -> String {
    let clean = url.trim().trim_start_matches('<').trim_end_matches('>').trim();
    match clean.strip_prefix("http://") {
        Some(rest) if plain_loopback && is_loopback(&host_name(clean)) => format!("https://{rest}"),
        _ => url.to_string(),
    }
}

/// What the POST really goes to: `https` stays `https`, except toward this Mac's own address on a plain-loopback core.
fn wire_url(url: &str, plain_loopback: bool) -> String {
    match url.strip_prefix("https://") {
        Some(rest) if plain_loopback && is_loopback(&host_name(url)) => format!("http://{rest}"),
        _ => url.to_string(),
    }
}

/// How this message's list can be left, from the header the store kept. None = no header.
pub(crate) fn method(store: &Store, message_id: &str, plain_loopback: bool) -> Option<UnsubscribeMethod> {
    let (urls, one_click) = store.list_unsubscribe(message_id).ok()?;
    if urls.is_empty() { return None; }
    unsubscribe_method_from(urls.iter().map(|u| as_header_url(u, plain_loopback)).collect(), one_click)
}

fn failed(words: String, retryable: bool) -> CoreError {
    CoreError::Action { failure: ActionFailed { account_name: String::new(), reason_words: words, retryable, code: None } }
}

/// The one POST of RFC 8058. Runs on its own thread: a blocking client never nests inside a caller's async runtime.
fn post_one_click(url: &str) -> Result<(), CoreError> {
    netguard::outbound_allowed_url(url)?;   // engine-act-r6 round 5
    let host = host_name(url);
    let url = url.to_string();
    let answer = std::thread::scope(|s| s.spawn(|| -> Result<(u16, String), String> {
        // ring is the TLS provider (Cargo.toml: rustls-no-provider); an Err means one is installed already, which is fine
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_millis(OAUTH_HTTP_TIMEOUT_MS))
            .build().map_err(|e| format!("{e}"))?;
        let r = client.post(&url).header(reqwest::header::CONTENT_TYPE, "application/x-www-form-urlencoded").body(ONE_CLICK_BODY).send().map_err(|e| format!("{}", e.without_url()))?;
        let status = r.status().as_u16();
        let body: String = r.text().unwrap_or_default().chars().take(200).collect();
        Ok((status, body))
    }).join());
    match answer {
        Ok(Ok((status, _))) if (200..300).contains(&status) => Ok(()),
        // the server's real status and the start of its body, never the generic words (R#20)
        Ok(Ok((status, body))) => Err(failed(format!("The list's server {host} answered {status} to the unsubscribe request.{}", if body.trim().is_empty() { String::new() } else { format!(" It said: {}", body.trim()) }), status >= 500)),
        Ok(Err(why)) => Err(failed(format!("The list's server {host} could not be reached: {why}"), true)),
        Err(_) => Err(failed("The unsubscribe request stopped unexpectedly. Try again.".into(), true)),
    }
}

/// The unsubscribe mail: one outbox entry, due at once, from the message's account. Saved under a key made from the message, so a
/// second save of the same message is the same entry and never a second mail.
fn queue_mail(store: &Arc<Store>, message_id: &str, account_id: &str, address: &str, subject: Option<String>, now_ms: i64) -> Result<(), CoreError> {
    use outbox::ports::{DraftEffect, SaveEntryPort, SaveRequest};
    let draft = Draft {
        id: format!("unsubscribe-{message_id}"), from_account_id: account_id.to_string(), to: vec![address.to_string()], cc: vec![], bcc: vec![],
        subject: subject.filter(|s| !s.trim().is_empty()).unwrap_or_else(|| DEFAULT_SUBJECT.to_string()), body: DEFAULT_SUBJECT.to_string(),
        attachment_ids: vec![], ai_marks: vec![], reply_to_thread_id: None,
    };
    let record = outbox::Record::undoable(format!("outbox-unsubscribe-{message_id}"), draft, now_ms, 0, format!("unsubscribe:{message_id}"), None);
    outbox::adapters::store::StoreOutbox::new(store.clone()).execute(&SaveRequest { record, draft: DraftEffect::Keep })
}

/// `Unsubscribe`. The order is the safe one for each method: the irreversible POST first, then the mark (a failed mark after a good POST
/// means a second POST on retry, which RFC 8058 makes harmless); the mail's entry is stored first, then the mark (a retry saves the same entry).
pub(crate) fn run(store: &Arc<Store>, message_id: &str, now_ms: i64, plain_loopback: bool) -> Result<CommandOutcome, CoreError> {
    let _once = UNSUBSCRIBE_ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let account = store.message_account(message_id).map_err(|e| store_failed("the message could not be read", e))?
        .ok_or_else(|| CoreError::NotFound { kind: "message".into(), id: message_id.to_string() })?;
    let done = store.list_left_at(message_id).map_err(|e| store_failed("the message could not be read", e))?.is_some();
    match check_unsubscribe(method(store, message_id, plain_loopback), done)? {
        UnsubscribeMethod::OneClick { url } => post_one_click(&wire_url(&url, plain_loopback))?,
        UnsubscribeMethod::Mail { address, subject } => queue_mail(store, message_id, &account, &address, subject, now_ms)?,
        UnsubscribeMethod::Link { .. } => {}
    }
    store.mark_unsubscribed(message_id.to_string(), now_ms).map_err(|e| store_failed("the list could not be marked as left", e))?;
    Ok(CommandOutcome { undo_entry_id: None, created_id: None })
}

/// The three state paths of a message's list; None for any other path.
pub(crate) fn state(store: &Store, plain_loopback: bool, path: &str) -> Option<String> {
    let p: Vec<&str> = path.split('/').collect();
    let ["message", id, what] = p.as_slice() else { return None };
    store.message_account(id).ok().flatten()?;
    let left_at = store.list_left_at(id).ok()?;
    let done = left_at.is_some();
    let method = method(store, id, plain_loopback);
    match *what {
        "unsubscribed_at" => left_at.map(|ms| ms.to_string()),
        "unsubscribe" => Some(json_text(&unsubscribe_state_key(unsubscribe_state(method, done)))),
        "unsubscribed" => Some(done.to_string()),
        "unsubscribe_method" => method.map(|m| match m { UnsubscribeMethod::OneClick { .. } => "one_click", UnsubscribeMethod::Mail { .. } => "mail", UnsubscribeMethod::Link { .. } => "link" }.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{as_header_url, wire_url};

    // rig-net-rust: the plain-loopback rewrite reads the host the client dials. Mutant "unsubscribe-hand-host": `host_name` back to the
    // split on '/', '?' and the last ':' (red: "127.0.0.1:9@evil.example" reads as 127.0.0.1, and clear HTTP goes to evil.example).
    #[test]
    fn the_loopback_rewrite_reads_the_host_the_client_dials() {
        assert_eq!(wire_url("https://127.0.0.1:9/u", true), "http://127.0.0.1:9/u");
        assert_eq!(wire_url("https://127.0.0.1:9@evil.example/u", true), "https://127.0.0.1:9@evil.example/u");
        assert_eq!(wire_url("https://evil.example\\@127.0.0.1:9/u", true), "https://evil.example\\@127.0.0.1:9/u");
        assert_eq!(as_header_url("http://127.0.0.1:9/u", true), "https://127.0.0.1:9/u");
        assert_eq!(as_header_url("http://127.0.0.1:9@evil.example/u", true), "http://127.0.0.1:9@evil.example/u");
        assert_eq!(wire_url("https://127.0.0.1:9/u", false), "https://127.0.0.1:9/u");
    }
}
