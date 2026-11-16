//! Delta 10 (2026-11-08): the list across a SET of accounts, the built-in views and list filters, unsubscribe, and the
//! mailbox-count rule. Pure logic, shared by the store, the real core and the Swift FixtureCore.
use crate::{InboxQuery, Mailbox};

/// A filter on the current list (⇧U ⇧S ⇧I ⇧R). Several filters together keep a row only when it passes all of them.
#[derive(uniffi::Enum, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InboxFilter {
    Unread,
    Starred,
    /// the decision model's signal: the row was filed as needing a reply, or is from a VIP
    Important,
    /// inbound threads awaiting the owner's reply (the `waiting` definition)
    NoReply,
}

/// What a filter needs to know about one row.
#[derive(uniffi::Record, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListFilterFacts {
    pub unread: bool,
    pub starred: bool,
    pub important: bool,
    pub awaiting_reply: bool,
}

/// True when the row passes every filter (no filter: every row passes).
#[uniffi::export]
pub fn passes_filters(facts: ListFilterFacts, filters: Vec<InboxFilter>) -> bool {
    filters.iter().all(|f| match f {
        InboxFilter::Unread => facts.unread,
        InboxFilter::Starred => facts.starred,
        InboxFilter::Important => facts.important,
        InboxFilter::NoReply => facts.awaiting_reply,
    })
}

/// The accounts a query shows: None = every account. `account_ids` is the canonical field; the older single
/// `account_id` is read only when `account_ids` is empty (a caller written before the set).
#[uniffi::export]
pub fn query_accounts(query: InboxQuery) -> Option<Vec<String>> {
    if !query.account_ids.is_empty() {
        Some(query.account_ids)
    } else {
        query.account_id.map(|a| vec![a])
    }
}

/// Whether a row of this account is in the query's account set.
pub fn account_in(query: &InboxQuery, account_id: &str) -> bool {
    if !query.account_ids.is_empty() {
        query.account_ids.iter().any(|a| a == account_id)
    } else {
        query.account_id.as_deref().is_none_or(|a| a == account_id)
    }
}

/// The count a sidebar mailbox row shows: Junk, Trash and Archive show no count at zero (owner, v8), the others show theirs.
#[uniffi::export]
pub fn mailbox_count_shown(mailbox: Mailbox, count: u32) -> Option<u32> {
    match mailbox {
        Mailbox::Junk | Mailbox::Trash | Mailbox::Archive if count == 0 => None,
        _ => Some(count),
    }
}

/// How a mailing list can be left, from its List-Unsubscribe header (RFC 2369) and List-Unsubscribe-Post (RFC 8058).
#[derive(uniffi::Enum, Debug, Clone, PartialEq, Eq)]
pub enum UnsubscribeMethod {
    /// RFC 8058: an https URL and `List-Unsubscribe-Post: List-Unsubscribe=One-Click`; the core posts it, no page opens
    OneClick { url: String },
    /// a mailto: link; leaving sends a message from the account that received the list
    Mail { address: String, subject: Option<String> },
    /// an http(s) page the owner opens in the browser
    Link { url: String },
}

/// Picks the method from the header's URLs (in header order) and whether List-Unsubscribe-Post offered one-click.
/// One-click wins when offered with an https URL, then a mail link, then a web link. None: the message is not from a list.
#[uniffi::export]
pub fn unsubscribe_method_from(urls: Vec<String>, one_click_offered: bool) -> Option<UnsubscribeMethod> {
    let clean: Vec<String> = urls.iter().map(|u| u.trim().trim_start_matches('<').trim_end_matches('>').trim().to_string()).collect();
    if one_click_offered {
        if let Some(u) = clean.iter().find(|u| u.to_ascii_lowercase().starts_with("https://")) {
            return Some(UnsubscribeMethod::OneClick { url: u.clone() });
        }
    }
    if let Some(m) = clean.iter().find(|u| u.to_ascii_lowercase().starts_with("mailto:")) {
        let rest = &m[7..];
        let (address, query) = rest.split_once('?').map_or((rest, None), |(a, q)| (a, Some(q)));
        let subject = query.and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("subject=").map(|s| s.replace("%20", " "))));
        if !address.is_empty() {
            return Some(UnsubscribeMethod::Mail { address: address.to_string(), subject });
        }
    }
    clean
        .iter()
        .find(|u| { let l = u.to_ascii_lowercase(); l.starts_with("https://") || l.starts_with("http://") })
        .map(|u| UnsubscribeMethod::Link { url: u.clone() })
}

/// The first lines of a draft's body for its summary: the first `n` non-empty lines, trimmed, joined by a newline (delta 11).
#[uniffi::export]
pub fn draft_first_lines(body: String, n: u32) -> String {
    body.lines().map(str::trim).filter(|l| !l.is_empty()).take(n as usize).collect::<Vec<_>>().join("\n")
}

/// Whether a label may be filed on a thread (delta 11): once the owner took a label off a thread, a rule or the decision
/// model never files it there again; the owner can (and doing so lifts the removal).
#[uniffi::export]
pub fn may_file_label(decided_by: crate::DecidedBy, removed_by_user: bool) -> bool {
    !removed_by_user || matches!(decided_by, crate::DecidedBy::Owner)
}

/// The List-Unsubscribe header (RFC 2369) as `Message.list_unsubscribe` keeps it: each "<...>" entry in header order, folding
/// whitespace removed; and whether List-Unsubscribe-Post (RFC 8058) offered one-click (delta 11, shared by the sync crates).
pub fn list_unsubscribe_from_headers(list_unsubscribe: Option<&str>, post: Option<&str>) -> (Vec<String>, bool) {
    let mut urls = Vec::new();
    if let Some(v) = list_unsubscribe {
        let mut rest = v;
        while let Some(start) = rest.find('<') {
            let Some(len) = rest[start..].find('>') else { break };
            let inner: String = rest[start + 1..start + len].chars().filter(|c| !c.is_whitespace()).collect();
            if !inner.is_empty() {
                urls.push(format!("<{inner}>"));
            }
            rest = &rest[start + len + 1..];
        }
    }
    let one_click = !urls.is_empty()
        && post.is_some_and(|p| p.split(|c: char| c == ';' || c == ',').any(|kv| kv.trim().eq_ignore_ascii_case("List-Unsubscribe=One-Click")));
    (urls, one_click)
}

/// The code a failed send shows beside its words (delta 12, `ActionFailed.code`): the 3-digit SMTP reply code, followed
/// by the RFC 3463 enhanced status code when the server's text starts with one of the same class ("451 4.7.1").
/// None when the reply code is not a 4xx or 5xx failure. Rust-only: the send path fills the field, the UI reads it.
pub fn smtp_failure_code(reply_code: &str, server_text: &str) -> Option<String> {
    let reply = reply_code.trim();
    let b = reply.as_bytes();
    if b.len() != 3 || !b.iter().all(u8::is_ascii_digit) || !(b[0] == b'4' || b[0] == b'5') {
        return None;
    }
    // the text may repeat the reply code ("451-4.7.1 ..." or "451 4.7.1 ..."): skip it
    let mut text = server_text.trim_start();
    if let Some(rest) = text.strip_prefix(reply) {
        text = rest.trim_start_matches(['-', ' ']);
    }
    let first = text.split_whitespace().next().unwrap_or("");
    let parts: Vec<&str> = first.split('.').collect();
    let enhanced = parts.len() == 3
        && parts[0].len() == 1 && parts[0].as_bytes()[0] == b[0]
        && parts[1..].iter().all(|p| (1..=3).contains(&p.len()) && p.bytes().all(|c| c.is_ascii_digit()));
    Some(if enhanced { format!("{reply} {first}") } else { reply.to_string() })
}
