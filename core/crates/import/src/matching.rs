//! Which account a message belongs to: the address it was delivered to. Delivery headers come first (Delivered-To,
//! X-Original-To, Envelope-To), then To, Cc, Bcc, then From. In a sent mailbox From comes first, because the owner wrote it.
//! No match means the Imported account.
use crate::model::AccountAddress;
use crate::rawhdr::{address_of, header_values};

/// "Sent", "Sent Messages", "Sent Items", "[Gmail]/Sent Mail": a word of the last folder name is "sent".
pub fn is_sent_mailbox(mailbox: &str) -> bool {
    let last = mailbox.rsplit('/').next().unwrap_or(mailbox);
    last.split(|c: char| !c.is_alphanumeric()).any(|w| w.eq_ignore_ascii_case("sent"))
}

/// The addresses to try, lowercased, in order.
pub fn candidate_addresses(raw: &[u8], to: &[String], cc: &[String], bcc: &[String], from: Option<&str>, sent_mailbox: bool) -> Vec<String> {
    let lower = |s: &str| s.trim().to_ascii_lowercase();
    let mut out: Vec<String> = Vec::new();
    let from = from.map(lower).filter(|f| !f.is_empty());
    if sent_mailbox {
        out.extend(from.clone());
    }
    for name in ["delivered-to", "x-original-to", "envelope-to"] {
        out.extend(header_values(raw, name).iter().filter_map(|v| address_of(v)));
    }
    for list in [to, cc, bcc] {
        out.extend(list.iter().map(|a| lower(a.as_str())));
    }
    if !sent_mailbox {
        out.extend(from);
    }
    out
}

/// The id of the first account whose address is one of the candidates.
pub fn account_for<'a>(accounts: &'a [AccountAddress], candidates: &[String]) -> Option<&'a AccountAddress> {
    candidates.iter().find_map(|c| accounts.iter().find(|a| a.address.trim().to_ascii_lowercase() == *c))
}
