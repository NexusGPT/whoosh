//! Per-provider limits and the provider's name as the owner reads it.
use crate::consts;
use crate::port::Transport;
use crate::words;
use contracts::{Account, MailProvider};

/// Calls one account may have in flight at once. The cap follows how the account is REACHED, not who hosts it: a Graph
/// adapter takes four concurrent calls per mailbox, every IMAP account takes IMAP_CONN_PER_ACCOUNT, Microsoft 365 over
/// IMAP with XOAUTH2 included (it closes connections past its per-user limit: "Connection closed. 14").
pub fn worker_cap(_provider: MailProvider, transport: Transport) -> usize {
    match transport {
        Transport::Graph => consts::GRAPH_CONCURRENT,
        Transport::Imap => consts::IMAP_CONN_PER_ACCOUNT,
    }
}

/// The provider's brand, or the mail server's domain for a plain IMAP account.
pub fn provider_name(account: &Account) -> String {
    match account.provider {
        MailProvider::Google => "Google".to_string(),
        MailProvider::Microsoft => "Microsoft".to_string(),
        MailProvider::Icloud => "iCloud".to_string(),
        MailProvider::Fastmail => "Fastmail".to_string(),
        MailProvider::Imap => account.address.rsplit_once('@').map(|(_, d)| d.to_string()).unwrap_or_else(|| account.address.clone()),
    }
}

/// "slowed by <provider>", the engine's own English words.
pub fn slowed_words(account: &Account) -> String {
    format!("slowed by {}", provider_name(account))
}

/// The status of a slowed account: its words and the code `sync.slowed` with the provider.
pub fn slowed_status(account: &Account) -> String {
    words::status(&slowed_words(account), words::SLOWED, &[("provider", &provider_name(account))])
}
